use core::fmt::Write as _;
use core::str::FromStr;

use as5600::asynch::As5600;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_futures::select::{select, Either};
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Ticker, Timer};
use esp_hal::i2c::master::I2c;
use esp_hal::Async;

use crate::{
    config::CONFIG,
    tasks::mqtt_task::{MqttPacket, CHANNEL_SIZE, DEFAULT_STRING_SIZE},
};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

use heapless::String;
use log::{debug, error, info};

const MEASUREMENT_FREQ: u64 = 2;
const INVALID_ANGLE: f32 = 361.0;

#[embassy_executor::task]
pub async fn as5600_task(
    mut encoder: As5600<
        &'static mut I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Async>>,
    >,
    mqtt_sender: Sender<'static, CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE>,
) {
    info!("Starting as5600 task");
    let mut ticker = Ticker::every(Duration::from_secs(CONFIG.task_dur_secs));
    let mut nb_measurements: f32 = 1.0;
    let mut avg_angle: f32 = 0.0;

    loop {
        let tick = ticker.next();
        match select(Timer::after_secs(MEASUREMENT_FREQ), tick).await {
            Either::First(()) => {
                let current_angle = get_wind_direction(&mut encoder).await;
                if current_angle == INVALID_ANGLE {
                    continue;
                }

                avg_angle = (avg_angle + current_angle) / nb_measurements;
                nb_measurements += 1.0;
            }
            Either::Second(()) => {
                break;
            }
        }
    }

    info!("measured angle {}", avg_angle);
    let mut payload = String::<DEFAULT_STRING_SIZE>::new();
    write!(&mut payload, "{}", match_direction(avg_angle)).unwrap();
    let mut topic = String::<DEFAULT_STRING_SIZE>::new();
    write!(&mut topic, "{}/anemo/wind_direction", CONFIG.topic).unwrap();
    let packet = MqttPacket::new(&topic, &payload);

    mqtt_sender.send(packet).await;
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
