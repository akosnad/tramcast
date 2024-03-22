use std::sync::mpsc::Sender;

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::modem::Modem,
    mqtt::client::{EspMqttClient, Message, MessageImpl, MqttClientConfiguration},
    nvs::EspDefaultNvsPartition,
    sntp::EspSntp,
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
) {
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
                            _ => log::info!("Received unknown message: {:?}", msg),
                        },
                        esp_idf_svc::mqtt::client::Event::Connected(_) => {
                            log::info!("Connected to MQTT broker");
                            client
                                .subscribe("villamos", esp_idf_svc::mqtt::client::QoS::AtMostOnce)
                                .unwrap();
                            client
                                .subscribe("metro", esp_idf_svc::mqtt::client::QoS::AtMostOnce)
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
