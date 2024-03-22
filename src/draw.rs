use std::{thread, time::Duration};

use embedded_graphics::{
    geometry::Point,
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use esp_idf_svc::hal::spi::{config::DriverConfig, SpiDeviceDriver, SpiDriver, SPI2};
use esp_idf_svc::hal::{gpio::PinDriver, prelude::*, spi::config::Config};
use ssd1306::{prelude::*, Ssd1306};

pub fn draw_thread(peripherals: esp_idf_svc::hal::peripherals::Peripherals) {
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

    let spi_driver = Box::leak(Box::new(
        SpiDriver::new::<SPI2>(
            peripherals.spi2,
            d0,
            d1,
            Some(sdi),
            &DriverConfig::default(),
        )
        .unwrap(),
    ));

    let config = Config::new().baudrate(100.kHz().into()).write_only(true);

    let cs = PinDriver::output(cs).unwrap();

    let spi_device = SpiDeviceDriver::new(spi_driver, Some(cs2), &config).unwrap();
    let interface = ssd1306::prelude::SPIInterface::new(spi_device, dc, cs);
    let mut display = Ssd1306::new(
        interface,
        DisplaySize128x64,
        ssd1306::rotation::DisplayRotation::Rotate0,
    )
    .into_buffered_graphics_mode();
    display.init().unwrap();

    let style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .background_color(BinaryColor::Off)
        .build();

    Text::with_baseline("Hello, world!", Point::zero(), style, Baseline::Top)
        .draw(&mut display)
        .unwrap();

    display.flush().unwrap();

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
