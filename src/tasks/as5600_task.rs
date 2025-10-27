use as5600::asynch::As5600;
use core::str::FromStr;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_futures::select::{select, Either};
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Ticker, Timer};
use esp_hal::i2c::master::I2c;
use esp_hal::Async;

use crate::{
    config::{CHANNEL_SIZE, CONFIG, DEFAULT_STRING_SIZE},
    tasks::mqtt_task::MqttPacket,
};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

use heapless::String;
use log::{error, info};

const MEASUREMENT_FREQ: u64 = 2;
const INVALID_ANGLE: f32 = 361.0;

#[embassy_executor::task]
pub async fn as5600_task(
    i2c: &'static mut I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Async>>,
    mqtt_sender: Sender<'static, CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE>,
) {
    let mut encoder = As5600::new(i2c);

    info!("Starting as5600 task");
    let mut ticker = Ticker::every(Duration::from_secs(CONFIG.task_dur_secs));
    let mut nb_measurements: f32 = 0.0;
    let mut sum_angle: f32 = 0.0;

    loop {
        let tick = ticker.next();
        match select(Timer::after_secs(MEASUREMENT_FREQ), tick).await {
            Either::First(()) => {
                let current_angle = get_wind_direction(&mut encoder).await;
                if current_angle == INVALID_ANGLE {
                    continue;
                }

                sum_angle = sum_angle + current_angle;
                nb_measurements += 1.0;
            }
            Either::Second(()) => {
                break;
            }
        }
    }
    info!("avg angle: {}", sum_angle / nb_measurements);
    publish!(
        &mqtt_sender,
        "anemo/wind_direction",
        match_direction(sum_angle / nb_measurements)
    );
}

async fn get_wind_direction(
    encoder: &mut As5600<
        &'static mut I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Async>>,
    >,
) -> f32 {
    let reading = match encoder.angle().await {
        Ok(value) => value,
        Err(_) => {
            error!("Couldn't read wind direction");
            return INVALID_ANGLE;
        }
    };

    (reading as f32) * (360.0 / 4096.0)
}

fn match_direction(avg_angle: f32) -> String<DEFAULT_STRING_SIZE> {
    let direction = match avg_angle {
        0.0..45.0 => "N",
        45.0..90.0 => "NE",
        90.0..135.0 => "E",
        135.0..180.0 => "SE",
        180.0..225.0 => "S",
        225.0..270.0 => "SW",
        270.0..315.0 => "W",
        315.0..360.0 => "NW",
        _ => "Invalid Angle",
    };

    String::from_str(direction).unwrap()
}
