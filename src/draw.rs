use std::{sync::mpsc::Receiver, thread, time::Duration};

use embedded_graphics::{
    geometry::Point,
    mono_font::{ascii::FONT_6X10, MonoTextStyle, MonoTextStyleBuilder},
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

use crate::state::{Metro, StateEvent, Tram};

type DisplayDevice<'a> = Ssd1306<
    SPIInterface<
        SpiDeviceDriver<'a, &'a mut SpiDriver<'a>>,
        PinDriver<'a, Gpio16, esp_idf_svc::hal::gpio::Output>,
        PinDriver<'a, Gpio25, esp_idf_svc::hal::gpio::Output>,
    >,
    DisplaySize128x64,
    ssd1306::mode::BufferedGraphicsMode<DisplaySize128x64>,
>;

const STYLE: MonoTextStyle<'static, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_6X10)
    .text_color(BinaryColor::On)
    .background_color(BinaryColor::Off)
    .build();

#[derive(Default)]
struct Display<'a> {
    tram: Option<Tram>,
    metro: Option<Metro>,
    wifi_connected: bool,
    mqtt_connected: bool,
    dev: Option<DisplayDevice<'a>>,
}

impl<'a> Display<'a> {
    fn new(dev: DisplayDevice<'a>) -> Self {
        let mut this = Self {
            dev: Some(dev),
            ..Default::default()
        };
        this.redraw();
        this
    }

    fn update_state(&mut self, event: StateEvent) {
        match event {
            StateEvent::TramStateChanged(tram) => {
                self.tram = Some(tram);
            }
            StateEvent::MetroStateChanged(metro) => {
                self.metro = Some(metro);
            }
            StateEvent::WifiConnected(b) => {
                self.wifi_connected = b;
            }
            StateEvent::MqttConnected(b) => {
                self.mqtt_connected = b;
            }
        }
    }

    fn event_loop(&mut self, rx: Receiver<StateEvent>) -> ! {
        loop {
            while let Ok(event) = rx.try_recv() {
                self.update_state(event);
                self.redraw();
            }
            thread::sleep(Duration::from_secs(1));
        }
    }

    fn redraw(&mut self) {
        self.dev.as_mut().unwrap().clear(BinaryColor::Off).unwrap();

        self.draw_wifi();
        self.draw_mqtt();
        self.draw_tram();
        self.draw_time();

        self.dev.as_mut().unwrap().flush().unwrap();
    }

    fn draw_wifi(&mut self) {
        let dev = self.dev.as_mut().unwrap();

        if self.wifi_connected {
            Text::with_baseline("WIFI: connected", Point::new(0, 0), STYLE, Baseline::Top)
                .draw(dev)
                .unwrap();
        } else {
            Text::with_baseline("WIFI: disconnected", Point::new(0, 0), STYLE, Baseline::Top)
                .draw(dev)
                .unwrap();
        }
    }

    fn draw_mqtt(&mut self) {
        let dev = self.dev.as_mut().unwrap();

        if self.mqtt_connected {
            Text::with_baseline("MQTT: connected", Point::new(0, 10), STYLE, Baseline::Top)
                .draw(dev)
                .unwrap();
        } else {
            Text::with_baseline(
                "MQTT: disconnected",
                Point::new(0, 10),
                STYLE,
                Baseline::Top,
            )
            .draw(dev)
            .unwrap();
        }
    }

    fn draw_tram(&mut self) {
        let dev = self.dev.as_mut().unwrap();

        if let Some(tram) = &self.tram {
            if let Some(time_left_ms) = tram.time_left_ms {
                let time_left = chrono::Duration::from_std(Duration::from_millis(time_left_ms))
                    .unwrap()
                    .num_minutes();

                Text::with_baseline(
                    &format!("Tram: {}", time_left),
                    Point::new(0, 20),
                    STYLE,
                    Baseline::Top,
                )
                .draw(dev)
                .unwrap();
                return;
            }
        }

        Text::with_baseline("Tram: N/A", Point::new(0, 20), STYLE, Baseline::Top)
            .draw(dev)
            .unwrap();
    }

    fn draw_time(&mut self) {
        let dev = self.dev.as_mut().unwrap();

        let now = chrono::Utc::now();
        let time = now.format("%Y-%m-%d %H:%M:%S").to_string();

        Text::with_baseline(&time, Point::new(0, 30), STYLE, Baseline::Top)
            .draw(dev)
            .unwrap();
    }
}

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
    let mut display_device = Ssd1306::new(
        interface,
        DisplaySize128x64,
        ssd1306::rotation::DisplayRotation::Rotate0,
    )
    .into_buffered_graphics_mode();
    display_device.init().unwrap();

    let mut display = Display::new(display_device);
    display.event_loop(rx);
}
