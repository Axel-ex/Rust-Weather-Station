#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_net::StackResources;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Flex, Input, InputConfig, Output, OutputConfig};
use esp_hal::i2c::master::I2c;
use esp_hal::rng::Rng;
use esp_hal::rtc_cntl::sleep::TimerWakeupSource;
use esp_hal::rtc_cntl::sleep::{Ext0WakeupSource, WakeupLevel};
use esp_hal::rtc_cntl::{wakeup_cause, Rtc};
use esp_hal::system::{software_reset, SleepSource};
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{i2c, Async};
use esp_radio::Controller;
use log::info;

#[macro_use]
pub mod utils;
pub mod config;
pub mod tasks;

use crate::config::CONFIG;
use crate::tasks::anemo_task::anemo_task;
use crate::tasks::as5600_task::as5600_task;
use crate::tasks::ina219_task::ina210_task;
use crate::tasks::mqtt_task::{mqtt_task, MQTT_CHANNEL};
use crate::tasks::wifi_task::{runner_task, wifi_task};
use crate::utils::wait_for_stack;
use tasks::dht_task::dht_task;

// use esp_backtrace as _;
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    software_reset();
}

//I2c
type BusI2C = I2c<'static, Async>;

macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        STATIC_CELL.init_with(|| $val)
    }};
}
extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

//TODO: move const in config
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 98767);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    let timg1 = TimerGroup::new(peripherals.TIMG1);
    let mut watchdog = timg1.wdt;
    let watchdog_timeout = esp_hal::time::Duration::from_secs(CONFIG.main_task_dur_secs + 10);
    let _ = watchdog.set_timeout(esp_hal::timer::timg::MwdtStage::Stage0, watchdog_timeout);
    info!(
        "Main watchdog configured for {} seconds",
        CONFIG.main_task_dur_secs
    );

    // Init wifi
    let radio_init = mk_static!(
        Controller<'static>,
        esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller")
    );

    let (controller, interfaces) =
        esp_radio::wifi::new(radio_init, peripherals.WIFI, Default::default())
            .expect("Failed to initialize Wi-Fi controller");

    // Net stack
    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    let (stack, runner) = embassy_net::new(
        interfaces.sta,
        embassy_net::Config::dhcpv4(Default::default()),
        mk_static!(StackResources<6>, StackResources::<6>::new()),
        seed,
    );
    let receiver = MQTT_CHANNEL.receiver();
    spawner.spawn(runner_task(runner)).ok();
    spawner.spawn(wifi_task(controller)).ok();
    wait_for_stack(&stack)
        .await
        .inspect(|_| info!("Got config: {:?}", stack.config_v4()))
        .unwrap(); // crash if the stack never gets up
    spawner.spawn(mqtt_task(stack, receiver)).unwrap();

    //PERIPHERALS
    // PINS
    let dht_pin = Flex::new(peripherals.GPIO32);
    let mut transistor_pin = Output::new(
        peripherals.GPIO17,
        esp_hal::gpio::Level::High,
        OutputConfig::default(),
    );
    let anemo_pin = Input::new(
        peripherals.GPIO27,
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
    );

    // I2C bus
    let i2c_dev = I2c::new(peripherals.I2C0, i2c::master::Config::default())
        .unwrap()
        .with_sda(peripherals.GPIO21)
        .with_scl(peripherals.GPIO22)
        .into_async();
    let i2c_bus = mk_static!(Mutex<CriticalSectionRawMutex, BusI2C>, Mutex::new(i2c_dev));
    let ina_i2c = mk_static!(
        I2cDevice<'static, CriticalSectionRawMutex, BusI2C>,
        I2cDevice::new(i2c_bus)
    );
    let as_i2c = mk_static!(
        I2cDevice<'static, CriticalSectionRawMutex, BusI2C>,
        I2cDevice::new(i2c_bus)
    );

    let sender_dht = MQTT_CHANNEL.sender();
    let sender_anemo = MQTT_CHANNEL.sender();
    let sender_as5600 = MQTT_CHANNEL.sender();
    let sender_ina219 = MQTT_CHANNEL.sender();

    let mut rtc = Rtc::new(peripherals.LPWR);
    let deep_sleep_timer =
        TimerWakeupSource::new(core::time::Duration::from_secs(CONFIG.deep_sleep_dur_secs));
    let ext0 = Ext0WakeupSource::new(peripherals.GPIO25, WakeupLevel::Low);

    if let SleepSource::Ext0 = wakeup_cause() {
        publish!(&MQTT_CHANNEL.sender(), "rain", "0.231");
    }
    spawner.spawn(dht_task(dht_pin, sender_dht)).ok();
    spawner.spawn(anemo_task(anemo_pin, sender_anemo)).ok();
    spawner.spawn(as5600_task(as_i2c, sender_as5600)).ok();
    spawner.spawn(ina210_task(ina_i2c, sender_ina219)).ok();

    watchdog.enable();
    Timer::after_secs(CONFIG.main_task_dur_secs).await;
    info!("Going to sleep...");
    transistor_pin.set_low();
    Timer::after_secs(1).await;
    rtc.sleep_deep(&[&deep_sleep_timer, &ext0]);
}
