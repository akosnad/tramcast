use std::{sync::mpsc::Receiver, thread, time::Duration};

use embedded_graphics::{
    geometry::Point,
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use esp_idf_svc::hal::{gpio::PinDriver, prelude::*, spi::config::Config};
use esp_idf_svc::hal::{
    gpio::{Gpio16, Gpio17, Gpio21, Gpio22, Gpio23, Gpio25, Gpio26},
    spi::{config::DriverConfig, SpiDeviceDriver, SpiDriver, SPI2},
};
use ssd1306::{prelude::*, Ssd1306};

use crate::state::StateEvent;

pub fn draw_thread(
    rx: Receiver<StateEvent>,
    d0: Gpio22,
    d1: Gpio21,
    res: Gpio17,
    sdi: Gpio23,
    dc: Gpio16,
    cs: Gpio25,
    cs2: Gpio26,
    spi: SPI2,
) {
    let mut res = PinDriver::output(res).unwrap();
    res.set_high().unwrap();

    let mut dc = PinDriver::output(dc).unwrap();
    dc.set_low().unwrap();

    let spi_driver = Box::leak(Box::new(
        SpiDriver::new(spi, d0, d1, Some(sdi), &DriverConfig::default()).unwrap(),
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
        while let Ok(event) = rx.try_recv() {
            match event {
                StateEvent::TramStateChanged(tram) => {}
                _ => {}
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
}
