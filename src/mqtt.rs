use std::sync::mpsc::Sender;

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::modem::Modem,
    mqtt::client::{
        EspMqttClient, InitialChunkData, Message, MessageImpl, MqttClientConfiguration,
        SubsequentChunkData,
    },
    nvs::EspDefaultNvsPartition,
    sntp::EspSntp,
    sys::id_t,
    timer::EspTaskTimerService,
    wifi::{AsyncWifi, ClientConfiguration, EspWifi},
};

use crate::state::{Metro, StateEvent, Tram};

const WIFI_SSID: &str = env!("ESP_WIFI_SSID");
const WIFI_PASSWORD: &str = env!("ESP_WIFI_PASS");

const MQTT_ENDPOINT: &str = env!("ESP_MQTT_ENDPOINT");
const MQTT_CLIENT_ID: &str = env!("ESP_MQTT_CLIENT_ID");

pub async fn mqtt_thread(
    tx: Sender<StateEvent>,
    modem: Modem,
    sys_loop: EspSystemEventLoop,
    timer: EspTaskTimerService,
    nvs: EspDefaultNvsPartition,
) -> ! {
    let mut wifi = AsyncWifi::wrap(
        EspWifi::new(modem, sys_loop.clone(), Some(nvs)).unwrap(),
        sys_loop,
        timer,
    )
    .unwrap();
    wifi.set_configuration(&esp_idf_svc::wifi::Configuration::Client(
        ClientConfiguration {
            ssid: WIFI_SSID.into(),
            password: WIFI_PASSWORD.into(),
            ..Default::default()
        },
    ))
    .unwrap();
    loop {
        if !wifi.is_connected().unwrap() {
            tx.send(StateEvent::WifiConnected(false)).unwrap();
            wifi.start().await.unwrap();
            wifi.connect().await.unwrap();
            wifi.wait_netif_up().await.unwrap();
            tx.send(StateEvent::WifiConnected(true)).unwrap();
        }

        let ntp = EspSntp::new_default().unwrap();
        while ntp.get_sync_status() != esp_idf_svc::sntp::SyncStatus::Completed {
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
        tx.send(StateEvent::TimeSynced(true)).unwrap();

        let config = MqttClientConfiguration {
            client_id: MQTT_CLIENT_ID.into(),
            ..Default::default()
        };
        let (mut client, mut connection) =
            EspMqttClient::new_with_conn(MQTT_ENDPOINT, &config).unwrap();

        let mut ota: Option<esp_ota::OtaUpdate> = None;

        while let Some(msg) = connection.next() {
            match msg {
                Err(e) => log::error!("MQTT Error: {:?}", e),
                Ok(msg) => {
                    let event: esp_idf_svc::mqtt::client::Event<MessageImpl> = msg;

                    match event {
                        esp_idf_svc::mqtt::client::Event::Received(msg) => match msg.topic() {
                            Some("villamos") => {
                                let payload_raw = String::from_utf8(msg.data().to_vec()).unwrap();
                                let payload = serde_json::from_str::<Tram>(&payload_raw).unwrap();
                                log::info!("Payload: {:?}", payload);
                                tx.send(StateEvent::TramStateChanged(payload)).unwrap();
                            }
                            Some("metro") => {
                                let payload_raw = String::from_utf8(msg.data().to_vec()).unwrap();
                                let payload = serde_json::from_str::<Metro>(&payload_raw).unwrap();
                                log::info!("Payload: {:?}", payload);
                                tx.send(StateEvent::MetroStateChanged(payload)).unwrap();
                            }
                            Some("tramcast/ota/data") | None => {
                                if msg.topic().is_none() && ota.is_none() {
                                    log::info!(
                                        "Received unexpected message: id: {:?}, len: {}",
                                        msg.id(),
                                        msg.data().len()
                                    );
                                    continue;
                                }

                                let data = msg.data();
                                if let Some(mut in_progress_ota) = ota.take() {
                                    match msg.details() {
                                        esp_idf_svc::mqtt::client::Details::InitialChunk(_) => {
                                            panic!("Received initial OTA message in middle of OTA");
                                        }
                                        esp_idf_svc::mqtt::client::Details::SubsequentChunk(
                                            SubsequentChunkData {
                                                current_data_offset,
                                                total_data_size,
                                            },
                                        ) => {
                                            let current = current_data_offset + data.len();
                                            log::info!(
                                                "OTA message {}/{}",
                                                current,
                                                total_data_size
                                            );
                                            in_progress_ota.write(data).unwrap();

                                            if current == *total_data_size {
                                                log::info!("OTA message complete, applying...");
                                                let mut completed_ota =
                                                    in_progress_ota.finalize().unwrap();
                                                completed_ota.set_as_boot_partition().unwrap();
                                                completed_ota.restart();
                                                log::info!("OTA restart failed");
                                            } else {
                                                ota = Some(in_progress_ota);
                                            }
                                        }
                                        esp_idf_svc::mqtt::client::Details::Complete => {
                                            log::info!("OTA message complete, applying...");
                                            let mut completed_ota =
                                                in_progress_ota.finalize().unwrap();
                                            completed_ota.set_as_boot_partition().unwrap();
                                            completed_ota.restart();
                                            log::info!("OTA restart failed");
                                        }
                                    }
                                } else {
                                    log::info!("Starting new OTA update");
                                    match msg.details() {
                                        esp_idf_svc::mqtt::client::Details::InitialChunk(
                                            InitialChunkData { total_data_size },
                                        ) => {
                                            log::info!(
                                                "OTA message (initial) {}/{}",
                                                data.len(),
                                                total_data_size
                                            );
                                            let mut new_ota = esp_ota::OtaUpdate::begin().unwrap();
                                            new_ota.write(data).unwrap();
                                            ota = Some(new_ota);
                                        }
                                        _ => {
                                            panic!("Received OTA message without initial chunk");
                                        }
                                    }
                                }
                            }
                            Some("tramcast/ota/confirm") => {
                                let msg = String::from_utf8(msg.data().to_vec()).unwrap();
                                if msg != "success" {
                                    log::info!(
                                        "Received OTA confirm message with invalid content: {:?}",
                                        msg
                                    );
                                    continue;
                                }
                                log::info!("Received OTA confirm message");
                                esp_ota::mark_app_valid();
                            }
                            Some("tramcast/rollback") => {
                                log::info!("Received rollback message");
                                esp_ota::rollback_and_reboot().expect("Failed to rollback");
                            }
                            _ => log::info!("Received unknown message: {:?}", msg),
                        },
                        esp_idf_svc::mqtt::client::Event::Connected(_) => {
                            log::info!("Connected to MQTT broker");

                            let topics = vec![
                                "villamos",
                                "metro",
                                "tramcast/ota/data",
                                "tramcast/ota/confirm",
                                "tramcast/rollback",
                            ];
                            for topic in topics {
                                client
                                    .subscribe(topic, esp_idf_svc::mqtt::client::QoS::ExactlyOnce)
                                    .unwrap();
                            }
                            client
                                .publish(
                                    "tramcast/ota/result",
                                    esp_idf_svc::mqtt::client::QoS::AtMostOnce,
                                    false,
                                    "success".as_bytes(),
                                )
                                .unwrap();
                            tx.send(StateEvent::MqttConnected(true)).unwrap();
                        }
                        esp_idf_svc::mqtt::client::Event::Disconnected => {
                            log::info!("Disconnected from MQTT broker");
                            tx.send(StateEvent::MqttConnected(false)).unwrap();
                        }
                        esp_idf_svc::mqtt::client::Event::Subscribed(topic) => {
                            log::info!("Subscribed to topic: {:?}", topic);
                        }
                        esp_idf_svc::mqtt::client::Event::Unsubscribed(topic) => {
                            log::info!("Unsubscribed from topic: {:?}", topic);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
