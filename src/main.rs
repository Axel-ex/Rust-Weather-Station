#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use esp_hal::clock::CpuClock;
use esp_hal::i2c::master::I2c;
use esp_hal::rtc_cntl::wakeup_cause;
use esp_hal::system::software_reset;
use esp_hal::system::SleepSource;
use esp_hal::Async;
use log::info;

#[macro_use]
pub mod utils;
pub mod config;
pub mod network;
pub mod platform;
pub mod rtc_manager;
pub mod tasks;

use crate::config::CONFIG;
use crate::tasks::anemo_task::anemo_task;
use crate::tasks::as5600_task::as5600_task;
use crate::tasks::ina219_task::ina210_task;
use crate::tasks::mqtt_task::{mqtt_task, MQTT_CHANNEL};
use crate::tasks::ota_task::{init_ota, ota_task};
use crate::tasks::wifi_task::{runner_task, wifi_task};
use rtc_manager::RtcManager;
use tasks::dht_task::dht_task;
use utils::wait_for_stack;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    software_reset();
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

    let mut platform = platform::Platform::new(peripherals);
    esp_rtos::start(platform.timg0.timer0);

    let mut rtc_manager = RtcManager::new(platform.rain_pin, platform.lpwr);
    rtc_manager.init_next_full_measurement();

    if let SleepSource::Ext0 = wakeup_cause() {
        rtc_manager.handle_external_wakeup().await;
    }

    // Woke up from deep sleep timer
    let network_manager = network::NetworkManager::new(platform.wifi);

    let receiver = MQTT_CHANNEL.receiver();
    spawner.spawn(runner_task(network_manager.runner)).ok();
    spawner.spawn(wifi_task(network_manager.controller)).ok();
    wait_for_stack(&network_manager.stack)
        .await
        .expect("The network stack failed to get up");

    //check for OTA
    let ota_handle = init_ota(platform.flash);
    ota_task(network_manager.stack, ota_handle, &mut platform.watchdog).await;

    spawner
        .spawn(mqtt_task(network_manager.stack, receiver))
        .unwrap();

    // I2C bus, configure sda and scl with pull ups
    let i2c_bus = mk_static!(Mutex<CriticalSectionRawMutex, I2c<'static, Async>>, Mutex::new(platform.i2c_dev));
    let ina_i2c = mk_static!(
        I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Async>>,
        I2cDevice::new(i2c_bus)
    );
    let as_i2c = mk_static!(
        I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Async>>,
        I2cDevice::new(i2c_bus)
    );

    // Finally the tasks for each sensors
    let sender_dht = MQTT_CHANNEL.sender();
    let sender_anemo = MQTT_CHANNEL.sender();
    let sender_as5600 = MQTT_CHANNEL.sender();
    let sender_ina219 = MQTT_CHANNEL.sender();

    spawner.spawn(dht_task(platform.dht_pin, sender_dht)).ok();
    spawner
        .spawn(anemo_task(platform.anemo_pin, sender_anemo))
        .ok();
    spawner.spawn(as5600_task(as_i2c, sender_as5600)).ok();
    spawner.spawn(ina210_task(ina_i2c, sender_ina219)).ok();

    publish!(
        MQTT_CHANNEL.sender(),
        "rain",
        rtc_manager.load_rain_tips() as f32 * 0.231
    );
    rtc_manager.store_rain_tips(0);

    platform.watchdog.feed();
    Timer::after_secs(CONFIG.main_task_dur_secs).await;
    info!("Going to sleep...");

    platform.transistor_pin.set_low();
    platform.watchdog.disable();
    rtc_manager.set_next_full_measurement_s(CONFIG.deep_sleep_dur_secs);
    Timer::after_secs(1).await;

    rtc_manager.set_deep_sleep_timer(core::time::Duration::from_secs(CONFIG.deep_sleep_dur_secs));
    rtc_manager.sleep();
    panic!();
}
