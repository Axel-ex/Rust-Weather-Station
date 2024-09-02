use anyhow::Result;
use core::sync::atomic::Ordering;
use esp_idf_svc::{
    hal::gpio::*,
    mqtt::client::*,
    wifi::{BlockingWifi, EspWifi},
};
use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use weather_station::*;

//MQTT
pub fn mqtt_create(
    url: &str,
    client_id: &str,
) -> Result<(EspMqttClient<'static>, EspMqttConnection)> {
    let (mqtt_client, mqtt_connection) = EspMqttClient::new(
        url,
        &MqttClientConfiguration {
            client_id: Some(client_id),
            username: Some(CONFIG.mqtt_user),
            password: Some(CONFIG.mqtt_pass),
            ..Default::default()
        },
    )?;
    Ok((mqtt_client, mqtt_connection))
}

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
        .expect("fail publishing bme data");
}

pub fn publish_anemo_data(mqtt_cli: &mut EspMqttClient, wind_direction: String) {
    let anemo_topic = format!("{}/wind_direction", CONFIG.topic);

    mqtt_cli
        .publish(
            anemo_topic.as_str(),
            QoS::AtLeastOnce,
            true,
            wind_direction.as_bytes(),
        )
        .expect("fail publishing anemo data");

    let rotations = ROTATION_COUNT.load(Ordering::Relaxed);
    ROTATION_COUNT.store(0, Ordering::Relaxed);
    let topic = format!("{}/wind_speed", CONFIG.topic);

    mqtt_cli
        .publish(
            &topic,
            QoS::AtLeastOnce,
            true,
            rotations.to_string().as_bytes(),
        )
        .map_err(|e| {
            log::error!("Couldn't publish wind speed: {e}");
        })
        .ok();
}

pub fn publish_rain_data(mqtt_cli: &mut EspMqttClient) {
    let topic = format!("{}/rain", CONFIG.topic);
    let rain_count = RAIN_COUNT.load(Ordering::Relaxed);
    RAIN_COUNT.store(0, Ordering::Relaxed);

    mqtt_cli
        .publish(
            &topic,
            QoS::AtLeastOnce,
            true,
            rain_count.to_string().as_bytes(),
        )
        .map_err(|e| {
            log::error!("Error publishing rain data: {e}");
        })
        .ok();
}

pub fn publish_wifi_data(mqtt_cli: &mut EspMqttClient, wifi: &mut BlockingWifi<EspWifi>) {
    let scan_result = wifi.wifi_mut().scan();
    let topic = format!("{}/wifi", CONFIG.topic);

    match scan_result {
        Ok(access_points) => {
            // Filter to find the access point with SSID "MEO-BD8310"
            if let Some(net) = access_points.iter().find(|ap| ap.ssid == CONFIG.wifi_ssid) {
                mqtt_cli
                    .publish(
                        &topic,
                        QoS::AtLeastOnce,
                        true,
                        net.signal_strength.to_string().as_bytes(),
                    )
                    .map_err(|e| {
                        log::error!("Fail publishing wifi data: {e}");
                    })
                    .ok();
            } else {
                log::warn!("{} not found.", CONFIG.wifi_ssid);
            }
        }
        Err(e) => {
            log::warn!("Failed to scan WiFi networks: {:?}", e);
        }
    }
}
