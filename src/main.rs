#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use core::fmt::Write as _;

use as5600::asynch::As5600;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_net::StackResources;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Flex, Input, InputConfig, Output, OutputConfig};
use esp_hal::i2c::master::I2c;
use esp_hal::rng::Rng;
use esp_hal::rtc_cntl::sleep::TimerWakeupSource;
use esp_hal::rtc_cntl::sleep::{Ext0WakeupSource, WakeupLevel};
use esp_hal::rtc_cntl::{wakeup_cause, Rtc};
use esp_hal::system::SleepSource;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{i2c, Async};
use esp_radio::Controller;

use heapless::String;
use ina219::address::Address;
use ina219::calibration::{IntCalibration, MicroAmpere};
use ina219::AsyncIna219;

pub mod config;
pub mod tasks;

use crate::config::CONFIG;
use crate::tasks::anemo_task::anemo_task;
use crate::tasks::as5600_task::as5600_task;
use crate::tasks::ina219_task::ina210_task;
use crate::tasks::mqtt_task::{mqtt_task, MqttPacket, DEFAULT_STRING_SIZE, MQTT_CHANNEL};
use crate::tasks::pluvio_task::pluvio_window;
use crate::tasks::wifi_task::{runner_task, wifi_task};
use tasks::dht_task::dht_task;

//TODO: call the reset from esp_idf_sys in the panic handler
// #[panic_handler]
// fn panic(_: &core::panic::PanicInfo) -> ! {
//     loop {}
// }

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

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 98767);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

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

    //PERIPHERALS
    // PINS
    let dht_pin = Flex::new(peripherals.GPIO32);
    let _transistor_pin = Output::new(
        peripherals.GPIO17,
        esp_hal::gpio::Level::High,
        OutputConfig::default(),
    );
    let anemo_pin = Input::new(peripherals.GPIO27, InputConfig::default());
    let mut pluvio_p = peripherals.GPIO25;
    let pluvio_pin = Input::new(pluvio_p.reborrow(), InputConfig::default());

    // I2C peripherals
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

    let encoder = As5600::new(as_i2c);
    let calib = IntCalibration::new(MicroAmpere(1_000_000), 1_000).unwrap();
    let ina = AsyncIna219::new_calibrated(ina_i2c, Address::from_byte(0x40).unwrap(), calib)
        .await
        .unwrap();

    let sender_dht = MQTT_CHANNEL.sender();
    let sender_anemo = MQTT_CHANNEL.sender();
    let sender_pluvio = MQTT_CHANNEL.sender();
    let sender_as5600 = MQTT_CHANNEL.sender();
    let sender_ina219 = MQTT_CHANNEL.sender();
    let receiver = MQTT_CHANNEL.receiver();

    //TODO: register gpio wake up for pluvio

    let mut rtc = Rtc::new(peripherals.LPWR);
    let deep_sleep_timer =
        TimerWakeupSource::new(core::time::Duration::from_secs(CONFIG.deep_sleep_dur_secs));

    spawner.spawn(runner_task(runner)).ok();
    spawner.spawn(wifi_task(controller)).ok();
    spawner.spawn(mqtt_task(stack, receiver)).unwrap();
    spawner.spawn(dht_task(dht_pin, sender_dht)).ok();

    match wakeup_cause() {
        SleepSource::Ext0 => {
            let mut topic = String::<DEFAULT_STRING_SIZE>::new();
            let mut payload = String::<DEFAULT_STRING_SIZE>::new();
            write!(&mut topic, "{}/rain", CONFIG.topic).unwrap();
            write!(&mut payload, "0.231").unwrap();

            let packet = MqttPacket::new(&topic, &payload);
            MQTT_CHANNEL.send(packet).await;
            Timer::after_secs(2).await;
        }
        _ => {
            spawner.spawn(anemo_task(anemo_pin, sender_anemo)).ok();
            spawner.spawn(as5600_task(encoder, sender_as5600)).ok();
            spawner.spawn(ina210_task(ina, sender_ina219)).ok();
            pluvio_window(
                &mut pluvio_p,
                sender_pluvio,
                Duration::from_secs(CONFIG.main_task_dur_secs),
            )
            .await;
            Timer::after(Duration::from_secs(CONFIG.main_task_dur_secs)).await;
        }
    }

    let ext0 = Ext0WakeupSource::new(pluvio_p, WakeupLevel::Low);
    rtc.sleep_deep(&[&deep_sleep_timer, &ext0]);
}
