use std::sync::mpsc::Sender;

use esp_idf_svc::{
    eventloop::EspSystemEventLoop, hal::modem::Modem, nvs::EspDefaultNvsPartition,
    timer::EspTaskTimerService,
};

use crate::state::{StateEvent, Tram};

pub async fn mqtt_thread(
    tx: Sender<StateEvent>,
    _modem: Modem,
    _sys_loop: EspSystemEventLoop,
    _timer: EspTaskTimerService,
    _nvs: EspDefaultNvsPartition,
) -> ! {
    std::thread::sleep(std::time::Duration::from_secs(2));
    tx.send(StateEvent::WifiConnected(true)).unwrap();
    std::thread::sleep(std::time::Duration::from_secs(2));
    tx.send(StateEvent::TimeSynced(true)).unwrap();
    std::thread::sleep(std::time::Duration::from_secs(2));
    tx.send(StateEvent::MqttConnected(true)).unwrap();
    std::thread::sleep(std::time::Duration::from_secs(2));

    loop {
        tx.send(StateEvent::TramStateChanged(Tram {
            depart_at: Some(chrono::Utc::now() + chrono::TimeDelta::try_minutes(5).unwrap()),
            time_left_ms: Some(5 * 60 * 1000),
        }))
        .unwrap();
        std::thread::sleep(std::time::Duration::from_secs(15));
    }
}
