use std::thread;

use esp_idf_svc::hal::{peripherals::Peripherals, task::thread::ThreadSpawnConfiguration};

mod draw;

fn main() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();

    ThreadSpawnConfiguration {
        name: Some("draw_thread\0".as_bytes()),
        ..Default::default()
    }
    .set()
    .unwrap();

    let draw_thread = thread::Builder::new()
        .stack_size(4096)
        .spawn(move || draw::draw_thread(peripherals))
        .unwrap();

    draw_thread.join().unwrap();
}
