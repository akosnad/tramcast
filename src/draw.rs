use std::{sync::mpsc::Receiver, thread, time::Duration};

use chrono::SubsecRound;
use embedded_graphics::{
    geometry::Point,
    image::{Image, ImageRaw},
    mono_font::{ascii::FONT_6X10, MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use esp_idf_svc::hal::gpio::{Gpio16, Gpio17, Gpio21, Gpio22, Gpio23, Gpio25, Gpio26};
use esp_idf_svc::hal::i2c::I2C0;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::hal::spi::SPI2;
use ssd1306::{prelude::*, Ssd1306};

use crate::state::{Metro, StateEvent, Tram};

const NO_WIFI: &[u8] = include_bytes!("../assets/no_wifi.raw");

type DisplayDevice<DI> =
    Ssd1306<DI, DisplaySize128x64, ssd1306::mode::BufferedGraphicsMode<DisplaySize128x64>>;

const STYLE: MonoTextStyle<'static, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_6X10)
    .text_color(BinaryColor::On)
    .background_color(BinaryColor::Off)
    .build();

struct Display<DI> {
    tram: Option<Tram>,
    metro: Option<Metro>,
    wifi_connected: bool,
    mqtt_connected: bool,
    time_synced: bool,
    dev: Option<DisplayDevice<DI>>,
}

impl<DI> Display<DI>
where
    DI: WriteOnlyDataCommand,
{
    fn new(dev: DisplayDevice<DI>) -> Self {
        let mut this = Self {
            tram: None,
            metro: None,
            wifi_connected: false,
            mqtt_connected: false,
            time_synced: false,
            dev: Some(dev),
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
            StateEvent::TimeSynced(b) => {
                self.time_synced = b;
            }
        }
    }

    fn event_loop(&mut self, rx: Receiver<StateEvent>) -> ! {
        loop {
            while let Ok(event) = rx.try_recv() {
                self.update_state(event);
            }
            self.redraw();
            thread::sleep(Duration::from_millis(100));
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
        if self.wifi_connected {
            return;
        }
        let dev = self.dev.as_mut().unwrap();

        Text::with_baseline("WIFI: disconnected", Point::new(0, 0), STYLE, Baseline::Top)
            .draw(dev)
            .unwrap();

        let image_raw: ImageRaw<BinaryColor> = ImageRaw::new(NO_WIFI, 50);
        let image = Image::with_center(&image_raw, dev.bounding_box().center());
        image.draw(dev).unwrap();
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
        if !self.time_synced {
            return;
        }

        let dev = self.dev.as_mut().unwrap();

        if let Some(tram) = &self.tram {
            if let Some(depart_at) = tram.depart_at {
                let time_left_seconds = depart_at
                    .round_subsecs(0)
                    .signed_duration_since(chrono::Utc::now())
                    .num_seconds();

                if time_left_seconds <= 0 {
                    Text::with_baseline("Tram: now", Point::new(0, 20), STYLE, Baseline::Top)
                        .draw(dev)
                        .unwrap();
                    return;
                }

                let time_left_human =
                    humantime::format_duration(Duration::from_secs(time_left_seconds as u64));

                Text::with_baseline(
                    &format!("Tram: {}", time_left_human),
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
        if !self.time_synced {
            return;
        }

        let dev = self.dev.as_mut().unwrap();

        let now = chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(3600).unwrap());

        let time = now.format("%Y-%m-%d %H:%M:%S").to_string();

        Text::with_baseline(&time, Point::new(0, 30), STYLE, Baseline::Top)
            .draw(dev)
            .unwrap();
    }
}

#[cfg(not(feature = "simulated"))]
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
    _i2c: I2C0,
) {
    use esp_idf_svc::hal::{
        gpio::PinDriver,
        spi::{
            config::{Config, DriverConfig},
            SpiDeviceDriver, SpiDriver,
        },
    };

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

#[cfg(feature = "simulated")]
pub fn draw_thread(
    rx: Receiver<StateEvent>,
    d0: Gpio22,
    d1: Gpio21,
    _res: Gpio17,
    _sdi: Gpio23,
    _dc: Gpio16,
    _cs: Gpio25,
    _cs2: Gpio26,
    _spi: SPI2,
    i2c: I2C0,
) {
    let config = esp_idf_svc::hal::i2c::I2cConfig::new().baudrate(10.kHz().into());
    let i2c = esp_idf_svc::hal::i2c::I2cDriver::new(i2c, d1, d0, &config).unwrap();
    let i2c = ssd1306::I2CDisplayInterface::new(i2c);
    let mut display_device = Ssd1306::new(
        i2c,
        DisplaySize128x64,
        ssd1306::rotation::DisplayRotation::Rotate0,
    )
    .into_buffered_graphics_mode();
    display_device.init().unwrap();

    let mut display = Display::new(display_device);
    display.event_loop(rx);
}
