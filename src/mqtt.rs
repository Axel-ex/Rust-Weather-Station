use crate::{global::*, CONFIG};
use anyhow::Result;
use core::sync::atomic::Ordering;
use embedded_dht_rs::SensorReading;
use esp_idf_svc::mqtt::client::*;
use ina219::measurements::BusVoltage;
use std::time::Duration;

//MQTT
pub fn mqtt_create() -> Result<(EspMqttClient<'static>, EspMqttConnection)> {
    let (mqtt_client, mqtt_connection) = EspMqttClient::new(
        CONFIG.broker_url,
        &MqttClientConfiguration {
            client_id: Some(CONFIG.client_id),
            username: Some(CONFIG.mqtt_user),
            password: Some(CONFIG.mqtt_pass),
            keep_alive_interval: Some(Duration::from_secs(100)),
            ..Default::default()
        },
    )?;
    Ok((mqtt_client, mqtt_connection))
}

#[allow(unused)]
pub fn publish_bme_data(mqtt_cli: &mut EspMqttClient, bme_readings: bosch_bme680::MeasurmentData) {
    let payload = format!(
        "{{\"temperature\": {}, \"humidity\": {}, \"pressure\": {}}}",
        bme_readings.temperature, bme_readings.humidity, bme_readings.pressure
    );
    let bme_topic = format!("{}/bme680", CONFIG.topic);

    mqtt_cli
        .publish(
            bme_topic.as_str(),
            QoS::AtLeastOnce,
            true,
            payload.as_bytes(),
        )
        .map_err(|e| log::error!("fail publishing bme data: {e}"))
        .ok();
}

pub fn publish_dht_data(mqtt_cli: &mut EspMqttClient, dht_readings: SensorReading<f32>) {
    if dht_readings.humidity == 0.0 && dht_readings.temperature == 0.0 {
        return;
    }
    let payload = format!(
        "{{\"temperature\": {}, \"humidity\": {}, \"pressure\": {}}}",
        dht_readings.temperature, dht_readings.humidity, 0
    );
    let bme_topic = format!("{}/bme680", CONFIG.topic);

    mqtt_cli
        .publish(
            bme_topic.as_str(),
            QoS::AtLeastOnce,
            true,
            payload.as_bytes(),
        )
        .map_err(|e| log::error!("fail publishing dht data: {e}"))
        .ok();
}

pub fn publish_anemo_data(mqtt_cli: &mut EspMqttClient, wind_direction: String) {
    let anemo_topic = format!("{}/anemo/wind_direction", CONFIG.topic);

    mqtt_cli
        .publish(
            anemo_topic.as_str(),
            QoS::AtLeastOnce,
            true,
            wind_direction.as_bytes(),
        )
        .map_err(|e| log::error!("fail publishing anemo data: {e}"))
        .ok();
    //calculation with anemo dimension * 3.6 to have km/h
    let wind_speed = (ROTATION_COUNT.load(Ordering::Relaxed) as f32)
        * (1.05 / CONFIG.active_duration_s as f32)
        * 3.6;
    ROTATION_COUNT.store(0, Ordering::Relaxed);
    let topic = format!("{}/anemo/wind_speed", CONFIG.topic);

    mqtt_cli
        .publish(
            &topic,
            QoS::AtLeastOnce,
            true,
            wind_speed.to_string().as_bytes(),
        )
        .map_err(|e| {
            log::error!("Couldn't publish wind speed: {e}");
        })
        .ok();
}

pub fn publish_rain_data(mqtt_cli: &mut EspMqttClient) {
    let topic = format!("{}/rain", CONFIG.topic);
    let rain_quantity = (RAIN_COUNT.load(Ordering::Relaxed) as f32) * 0.233;
    RAIN_COUNT.store(0, Ordering::Relaxed);

    mqtt_cli
        .publish(
            &topic,
            QoS::AtLeastOnce,
            true,
            rain_quantity.to_string().as_bytes(),
        )
        .map_err(|e| {
            log::error!("Error publishing rain data: {e}");
        })
        .ok();
}

pub fn publish_battery_readings(mqtt_cli: &mut EspMqttClient, battery_reading: BusVoltage) {
    let topic_voltage = format!("{}/battery/voltage", CONFIG.topic);
    let topic_percentage = format!("{}/battery/percentage", CONFIG.topic);
    let payload_mv = format!("{}", battery_reading.voltage_mv());
    let payload_percentage = format!(
        "{}",
        (battery_reading.voltage_mv() as f32 / 1000 as f32 - 3.6) / (4.1 - 3.6) * 100 as f32
    );
    log::info!("Battery readings: {}", payload_mv);

    if let Err(e) = mqtt_cli.publish(
        &topic_voltage,
        QoS::AtLeastOnce,
        true,
        payload_mv.as_bytes(),
    ) {
        log::error!("Fail publishing bus voltage {e}");
    }
    if let Err(e) = mqtt_cli.publish(
        &topic_percentage,
        QoS::AtLeastOnce,
        true,
        payload_percentage.as_bytes(),
    ) {
        log::error!("Fail publishing battery percentage {e}");
    }
}
