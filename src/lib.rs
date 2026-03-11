#![no_std]

#[macro_use]
pub mod utils;
pub mod config;
pub mod network;
pub mod rtc_manager;
pub mod sensors;
pub mod tasks;

use crate::{
    config::CONFIG,
    network::{init_network, wait_for_stack},
    rtc_manager::RtcManager,
    sensors::Sensors,
    tasks::{
        anemo_task::anemo_task,
        as5600_task::as5600_task,
        dht_task::dht_task,
        ina219_task::ina210_task,
        mqtt_task::{mqtt_task, MQTT_CHANNEL},
        ota_task::check_for_ota,
        wifi_task::{runner_task, wifi_task},
    },
};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::Timer;
use esp_hal::{
    i2c::master::I2c,
    peripherals::{TIMG1, WIFI},
    time::Duration,
    timer::timg::{TimerGroup, Wdt},
    Async,
};
use esp_hal_ota::Ota;
use esp_storage::FlashStorage;
use log::info;

type ShareI2cBus = &'static mut I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Async>>;

pub fn init_watchdog(timer_group1: TIMG1) -> Wdt<TIMG1> {
    let mut watchdog = TimerGroup::new(timer_group1).wdt;

    let watchdog_timeout = Duration::from_secs(CONFIG.main_task_dur_secs + 10);
    watchdog.set_timeout(esp_hal::timer::timg::MwdtStage::Stage0, watchdog_timeout);
    watchdog.enable();

    watchdog
}

pub async fn measuring_window(
    spawner: &Spawner,
    rtc_manager: &mut RtcManager,
    watchdog: &mut Wdt<TIMG1<'static>>,
    mut sensors: Sensors,
    wifi: WIFI<'static>,
    ota_handle: &'static mut Ota<FlashStorage<'static>>,
) {
    let (controller, stack, runner) = init_network(wifi);
    spawner.spawn(runner_task(runner)).ok();
    spawner.spawn(wifi_task(controller)).ok();
    wait_for_stack(&stack)
        .await
        .expect("The network stack failed to get up");

    check_for_ota(stack, ota_handle, watchdog).await;

    // Create communication channels
    let receiver = MQTT_CHANNEL.receiver();
    let sender_dht = MQTT_CHANNEL.sender();
    let sender_anemo = MQTT_CHANNEL.sender();
    let sender_as5600 = MQTT_CHANNEL.sender();
    let sender_ina219 = MQTT_CHANNEL.sender();

    // spawn the tasks
    let (ina_i2c, as_i2c) = share_i2c_bus(sensors.i2c_bus);
    spawner.spawn(mqtt_task(stack, receiver)).unwrap();
    spawner
        .spawn(dht_task(sensors.dht_pin, sender_dht))
        .unwrap();
    spawner
        .spawn(anemo_task(sensors.anemo_pin, sender_anemo))
        .unwrap();
    spawner.spawn(as5600_task(as_i2c, sender_as5600)).unwrap();
    spawner.spawn(ina210_task(ina_i2c, sender_ina219)).unwrap();

    //publish accumulated rain and reset RTC memory
    publish!(
        MQTT_CHANNEL.sender(),
        "rain",
        rtc_manager.load_rain_tips() as f32 * 0.231
    );
    rtc_manager.store_rain_tips(0);

    // wait for the task to perform their jobs
    watchdog.feed();
    Timer::after_secs(CONFIG.main_task_dur_secs).await;

    sensors.transistor_pin.set_low(); //turn off peripherals
    watchdog.disable();
    rtc_manager.set_next_full_measurement_s(CONFIG.deep_sleep_dur_secs);
    rtc_manager.set_deep_sleep_timer(core::time::Duration::from_secs(CONFIG.deep_sleep_dur_secs));

    info!("Going to sleep...");
    Timer::after_secs(1).await;
}

fn share_i2c_bus(
    i2c_bus: &'static Mutex<CriticalSectionRawMutex, I2c<'static, Async>>,
) -> (ShareI2cBus, ShareI2cBus) {
    let ina_i2c = mk_static!(
        I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Async>>,
        I2cDevice::new(i2c_bus)
    );
    let as_i2c = mk_static!(
        I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Async>>,
        I2cDevice::new(i2c_bus)
    );

    (ina_i2c, as_i2c)
}
