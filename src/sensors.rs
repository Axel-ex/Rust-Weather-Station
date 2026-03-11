//! Sensor hardware initialization and ownership.
//!
//! This module groups together the peripherals required by the physical
//! sensors used in the weather station. It performs the one-time hardware
//! configuration required before the measurement tasks are spawned.
//!
//! - `Sensors` acts as a *hardware bundle* created during boot.
//! - The struct simply holds configured peripherals that will later be moved
//!   into their respective tasks.
//! - No sensor logic or polling happens here.
//!
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use esp_hal::{
    gpio::{Flex, Input, InputConfig, Output, OutputConfig, Pull},
    i2c::{self, master::I2c},
    peripherals::{GPIO17, GPIO21, GPIO22, GPIO27, GPIO32, I2C0},
    Async,
};

pub struct Sensors {
    pub i2c_bus: &'static Mutex<CriticalSectionRawMutex, I2c<'static, Async>>,
    pub transistor_pin: Output<'static>,
    pub dht_pin: Flex<'static>,
    pub anemo_pin: Input<'static>,
}

impl Sensors {
    pub fn new(
        transistor_gpio: GPIO17<'static>,
        dht_gpio: GPIO32<'static>,
        anemo_gpio: GPIO27<'static>,
        sda_pin: GPIO21<'static>,
        scl_pin: GPIO22<'static>,
        i2c: I2C0<'static>,
    ) -> Self {
        //i2c
        let i2c_dev = I2c::new(i2c, i2c::master::Config::default())
            .unwrap()
            .with_sda(sda_pin)
            .with_scl(scl_pin)
            .into_async();
        let i2c_bus =
            mk_static!(Mutex<CriticalSectionRawMutex, I2c<'static, Async>>, Mutex::new(i2c_dev));

        //peripherals
        let transistor_pin = Output::new(
            transistor_gpio,
            esp_hal::gpio::Level::High,
            OutputConfig::default(),
        );
        let dht_pin = Flex::new(dht_gpio);
        let anemo_pin = Input::new(anemo_gpio, InputConfig::default().with_pull(Pull::Up));

        Sensors {
            i2c_bus,
            transistor_pin,
            dht_pin,
            anemo_pin,
        }
    }
}
