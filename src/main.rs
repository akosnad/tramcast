use std::{sync::mpsc, thread};

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        peripherals::Peripherals,
        task::{block_on, thread::ThreadSpawnConfiguration},
    },
    nvs::EspDefaultNvsPartition,
    timer::EspTaskTimerService,
};

mod draw;
#[cfg(not(feature = "simulated"))]
mod mqtt;
#[cfg(feature = "simulated")]
mod simulated_mqtt;
mod state;

fn main() {
    #[cfg(feature = "simulated")]
    use simulated_mqtt as mqtt;

    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();
    let timer = EspTaskTimerService::new().unwrap();

    let d0 = peripherals.pins.gpio22;
    let d1 = peripherals.pins.gpio21;
    let res = peripherals.pins.gpio17;
    let sdi = peripherals.pins.gpio23;
    let dc = peripherals.pins.gpio16;
    let cs = peripherals.pins.gpio25;
    let cs2 = peripherals.pins.gpio26;
    let spi2 = peripherals.spi2;
    let i2c0 = peripherals.i2c0;

    let (tx, rx) = mpsc::channel::<state::StateEvent>();

    ThreadSpawnConfiguration {
        name: Some("draw_thread\0".as_bytes()),
        ..Default::default()
    }
    .set()
    .unwrap();

    let draw_thread = thread::Builder::new()
        .stack_size(8192)
        .spawn(move || draw::draw_thread(rx, d0, d1, res, sdi, dc, cs, cs2, spi2, i2c0))
        .unwrap();

    ThreadSpawnConfiguration {
        name: Some("mqtt_thread\0".as_bytes()),
        ..Default::default()
    }
    .set()
    .unwrap();

    let mqtt_thread = thread::Builder::new()
        .stack_size(8192)
        .spawn(move || {
            block_on(mqtt::mqtt_thread(
                tx,
                peripherals.modem,
                sys_loop,
                timer,
                nvs,
            ));
        })
        .unwrap();

    draw_thread.join().unwrap();
    mqtt_thread.join().unwrap();
}
