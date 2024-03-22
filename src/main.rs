use std::{thread, time::Duration};

use embedded_graphics::{
    geometry::Point,
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};
use esp_idf_svc::hal::{
    gpio::PinDriver,
    peripherals::Peripherals,
    prelude::*,
    spi::{
        config::{Config, DriverConfig},
        SpiDeviceDriver, SpiDriver, SPI2,
    },
};
use ssd1306::{mode::DisplayConfig, size::DisplaySize128x64, Ssd1306};

fn main() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();

    let d0 = peripherals.pins.gpio22;
    let d1 = peripherals.pins.gpio21;
    let res = peripherals.pins.gpio17;
    let sdi = peripherals.pins.gpio23;
    let dc = peripherals.pins.gpio16;
    let cs = peripherals.pins.gpio25;
    let cs2 = peripherals.pins.gpio26;

    let mut res = PinDriver::output(res).unwrap();
    res.set_high().unwrap();

    let mut dc = PinDriver::output(dc).unwrap();
    dc.set_low().unwrap();

    let spi_driver = SpiDriver::new::<SPI2>(
        peripherals.spi2,
        d0,
        d1,
        Some(sdi),
        &DriverConfig::default(),
    )
    .unwrap();

    let config = Config::new().baudrate(100.kHz().into()).write_only(true);
    let spi_device = SpiDeviceDriver::new(&spi_driver, Some(cs2), &config).unwrap();

    let cs = PinDriver::output(cs).unwrap();
    let interface = ssd1306::prelude::SPIInterface::new(spi_device, dc, cs);
    let mut display = Ssd1306::new(
        interface,
        DisplaySize128x64,
        ssd1306::rotation::DisplayRotation::Rotate0,
    )
    .into_buffered_graphics_mode();
    display.init().unwrap();

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    Text::with_baseline(
        "Hello, World!",
        Point::zero(),
        text_style,
        embedded_graphics::text::Baseline::Top,
    )
    .draw(&mut display)
    .unwrap();

    display.flush().unwrap();

    log::info!("Hello, world!");

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
