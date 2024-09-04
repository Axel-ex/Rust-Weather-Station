use anyhow::Result;
use core::sync::atomic::Ordering;
use esp_idf_svc::{
    mqtt::client::*,
    wifi::{BlockingWifi, EspWifi},
};
use std::time::Duration;
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
            connection_refresh_interval: Duration::from_secs(70),
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
        .map_err(|e| log::error!("fail publishing bme data: {e}"))
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
    let wind_speed = (ROTATION_COUNT.load(Ordering::Relaxed) as f32) * 0.0173833 * 3.6;
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
