use std::{
    sync::mpsc::Receiver,
    thread,
    time::{Duration, Instant},
};

use chrono::SubsecRound;
use chrono::TimeZone;
use chrono_tz::Europe::Budapest;
use embedded_graphics::{
    geometry::Point,
    image::{Image, ImageRaw},
    mono_font::{
        self, ascii::FONT_10X20, ascii::FONT_6X10, MonoFont, MonoTextStyle, MonoTextStyleBuilder,
    },
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Baseline, Text},
};
use esp_idf_svc::hal::gpio::{Gpio16, Gpio17, Gpio21, Gpio22, Gpio23, Gpio25, Gpio26};
use esp_idf_svc::hal::i2c::I2C0;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::hal::spi::SPI2;
use ssd1306::{prelude::*, Ssd1306};

use crate::state::{Metro, StateEvent, Tram};

const NO_WIFI: &[u8] = include_bytes!("../assets/no_wifi.raw");
const TRAM: &[u8] = include_bytes!("../assets/tram.raw");

type DisplayDevice<DI> =
    Ssd1306<DI, DisplaySize128x64, ssd1306::mode::BufferedGraphicsMode<DisplaySize128x64>>;

const STYLE: MonoTextStyle<'static, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_6X10)
    .text_color(BinaryColor::On)
    .background_color(BinaryColor::Off)
    .build();

const FONT_20X40: MonoFont<'static> = MonoFont {
    image: ImageRaw::new(include_bytes!("../assets/font_20x40.raw"), 320),
    glyph_mapping: &mono_font::mapping::ASCII,
    character_size: Size::new(20, 40),
    character_spacing: 0,
    baseline: 30,
    underline: mono_font::DecorationDimensions::new(30 + 4, 2),
    strikethrough: mono_font::DecorationDimensions::new(40 / 2, 2),
};

const BIG_STYLE: MonoTextStyle<'static, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_20X40)
    .text_color(BinaryColor::On)
    .background_color(BinaryColor::Off)
    .build();

const MEDIUM_STYLE: MonoTextStyle<'static, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_10X20)
    .text_color(BinaryColor::On)
    .background_color(BinaryColor::Off)
    .build();

enum Screen {
    Tram,
    DataNotAvailable,
    Metro,
    Weather,
}

struct Display<DI> {
    tram: Option<Tram>,
    metro: Option<Metro>,
    wifi_connected: bool,
    mqtt_connected: bool,
    time_synced: bool,
    dev: Option<DisplayDevice<DI>>,
    screen: Screen,
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
            screen: Screen::DataNotAvailable,
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

    fn event_loop(mut self, rx: Receiver<StateEvent>) -> ! {
        let mut last_screen_cycle = Instant::now();
        loop {
            while let Ok(event) = rx.try_recv() {
                self.update_state(event);
            }
            if last_screen_cycle.elapsed() > Duration::from_secs(4) {
                self.cycle_screen();
                last_screen_cycle = Instant::now();
            }
            self.redraw();
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn cycle_screen(&mut self) {
        if !self.wifi_connected || !self.mqtt_connected || !self.time_synced {
            self.screen = Screen::DataNotAvailable;
            return;
        }

        match self.screen {
            Screen::Tram => {
                // TODO: implement other screens
                //self.screen = Screen::Metro;
            }
            Screen::Metro => {
                self.screen = Screen::Weather;
            }
            Screen::Weather => {
                self.screen = Screen::Tram;
            }
            Screen::DataNotAvailable => {
                // If all data becomes available, start with the tram screen
                self.screen = Screen::Tram;
            }
        }
    }

    fn redraw(&mut self) {
        self.dev.as_mut().unwrap().clear(BinaryColor::Off).unwrap();

        self.draw_screen();
        self.draw_time();

        self.dev.as_mut().unwrap().flush().unwrap();
    }

    fn draw_screen(&mut self) {
        match self.screen {
            Screen::Tram => {
                self.draw_tram();
            }
            Screen::Metro => {
                self.draw_metro();
            }
            Screen::Weather => {
                self.draw_weather();
            }
            Screen::DataNotAvailable => {
                self.draw_data_not_available();
            }
        }
    }

    fn draw_time(&mut self) {
        if !self.time_synced {
            return;
        }

        let dev = self.dev.as_mut().unwrap();

        let now_utc = chrono::Utc::now();
        let now = Budapest.from_utc_datetime(&now_utc.naive_utc());

        let time = now.format("%Y-%m-%d %H:%M:%S").to_string();

        let center = dev.bounding_box().center();
        let top_center =
            Point::new(center.x, 0) + FONT_6X10.character_size.y_axis() - Point::new(0, 4).y_axis();

        Text::with_alignment(&time, top_center, STYLE, Alignment::Center)
            .draw(dev)
            .unwrap();
    }

    fn draw_tram(&mut self) {
        if !self.time_synced {
            return;
        }

        let dev = self.dev.as_mut().unwrap();
        let pos = Point::new(4 + 27 / 2, 0) + dev.bounding_box().center().y_axis();

        if let Some(tram) = &self.tram {
            if let Some(depart_at) = tram.depart_at {
                let time_left_seconds = depart_at
                    .round_subsecs(0)
                    .signed_duration_since(chrono::Utc::now())
                    .num_seconds();

                if time_left_seconds <= 0 {
                    Text::with_baseline("now", Point::new(0, 20), STYLE, Baseline::Top)
                        .draw(dev)
                        .unwrap();
                    return;
                }

                let image_raw: ImageRaw<BinaryColor> = ImageRaw::new(TRAM, 27);
                let image = Image::with_center(&image_raw, pos);
                image.draw(dev).unwrap();

                Text::with_baseline(
                    &format!("{:02}", time_left_seconds / 60),
                    Point::new(38, 55),
                    BIG_STYLE,
                    Baseline::Bottom,
                )
                .draw(dev)
                .unwrap();

                Text::with_alignment(
                    &format!(": {:02}", time_left_seconds % 60),
                    Point::new(128, 47),
                    MEDIUM_STYLE,
                    Alignment::Right,
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

    fn draw_metro(&mut self) {
        let dev = self.dev.as_mut().unwrap();

        if let Some(metro) = &self.metro {
            if let Some(depart_at) = metro.depart_at {
                let time_left_seconds = depart_at
                    .round_subsecs(0)
                    .signed_duration_since(chrono::Utc::now())
                    .num_seconds();

                if time_left_seconds <= 0 {
                    Text::with_baseline("Metro: now", Point::new(0, 20), STYLE, Baseline::Top)
                        .draw(dev)
                        .unwrap();
                    return;
                }

                let time_left_human =
                    humantime::format_duration(Duration::from_secs(time_left_seconds as u64));

                Text::with_baseline(
                    &format!("Metro: {}", time_left_human),
                    Point::new(0, 20),
                    STYLE,
                    Baseline::Top,
                )
                .draw(dev)
                .unwrap();
            }
        } else {
            Text::with_baseline("Metro: N/A", Point::new(0, 20), STYLE, Baseline::Top)
                .draw(dev)
                .unwrap();
        }
    }

    fn draw_weather(&mut self) {
        let dev = self.dev.as_mut().unwrap();

        Text::with_baseline("Weather: N/A", Point::new(0, 20), STYLE, Baseline::Top)
            .draw(dev)
            .unwrap();
    }

    fn draw_data_not_available(&mut self) {
        let dev = self.dev.as_mut().unwrap();

        let center = dev.bounding_box().center();
        let bottom_center = Point::new(center.x, 64) - FONT_6X10.character_size.y_axis()
            + Point::new(0, FONT_6X10.baseline as i32).y_axis();

        if !self.wifi_connected {
            Text::with_alignment(
                "Connecting WiFi...",
                bottom_center,
                STYLE,
                Alignment::Center,
            )
            .draw(dev)
            .unwrap();

            let image_raw: ImageRaw<BinaryColor> = ImageRaw::new(NO_WIFI, 50);
            let image = Image::with_center(&image_raw, center);
            image.draw(dev).unwrap();
        } else if !self.mqtt_connected {
            Text::with_alignment(
                "Connecting MQTT...",
                bottom_center,
                STYLE,
                Alignment::Center,
            )
            .draw(dev)
            .unwrap();
        } else if !self.time_synced {
            Text::with_alignment("Syncing time...", bottom_center, STYLE, Alignment::Center)
                .draw(dev)
                .unwrap();
        } else {
            Text::with_alignment(
                "Waiting for data...",
                bottom_center,
                STYLE,
                Alignment::Center,
            )
            .draw(dev)
            .unwrap();
        }
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

    let display = Display::new(display_device);
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

    let display = Display::new(display_device);
    display.event_loop(rx);
}
