use crate::CONFIG;

use esp_hal::{
    gpio::{Flex, Input, InputConfig, Output, OutputConfig, Pull},
    i2c::{self, master::I2c},
    peripherals::{self, FLASH, GPIO25, LPWR, TIMG0, TIMG1, WIFI},
    time::Duration,
    timer::timg::{TimerGroup, Wdt},
    Async,
};

pub struct Platform {
    pub lpwr: LPWR<'static>,
    pub timg0: TimerGroup<'static, TIMG0<'static>>,
    pub watchdog: Wdt<TIMG1<'static>>,
    pub flash: FLASH<'static>,
    pub i2c_dev: I2c<'static, Async>,
    pub wifi: WIFI<'static>,
    pub transistor_pin: Output<'static>,
    pub dht_pin: Flex<'static>,
    pub anemo_pin: Input<'static>,
    pub rain_pin: GPIO25<'static>,
}

impl Platform {
    pub fn new(peripherals: peripherals::Peripherals) -> Self {
        // watchdog
        let mut watchdog = TimerGroup::new(peripherals.TIMG1).wdt;
        let watchdog_timeout = Duration::from_secs(CONFIG.main_task_dur_secs + 10);
        watchdog.set_timeout(esp_hal::timer::timg::MwdtStage::Stage0, watchdog_timeout);
        watchdog.enable();

        //i2c
        let i2c_dev = I2c::new(peripherals.I2C0, i2c::master::Config::default())
            .unwrap()
            .into_async();

        //peripherals
        let transistor_pin = Output::new(
            peripherals.GPIO17,
            esp_hal::gpio::Level::High,
            OutputConfig::default(),
        );
        let dht_pin = Flex::new(peripherals.GPIO32);
        let anemo_pin = Input::new(
            peripherals.GPIO27,
            InputConfig::default().with_pull(Pull::Up),
        );
        let mut rain_gpio = peripherals.GPIO25;
        let _rain_pin = Input::new(
            rain_gpio.reborrow(),
            InputConfig::default().with_pull(Pull::Up),
        );

        Platform {
            lpwr: peripherals.LPWR,
            timg0: TimerGroup::new(peripherals.TIMG0),
            flash: peripherals.FLASH,
            wifi: peripherals.WIFI,
            watchdog,
            i2c_dev,
            transistor_pin,
            dht_pin,
            anemo_pin,
            rain_pin: rain_gpio,
        }
    }
}
