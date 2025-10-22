use std::{sync::Arc, thread};

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{delay::FreeRtos, task::{queue::Queue, TickType}},
    log::EspLogger,
    mqtt::client::{EspMqttClient, MqttClientConfiguration},
    nvs::EspDefaultNvs,
};
use embedded_svc::mqtt::client::QoS;
use log::{error, info};

#[derive(Copy, Clone, Debug, Default)]
struct DhtReading {
    temperature_c: f32,
    humidity_rh: f32,
}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    // Grab shared ESP-IDF services the rest of the program needs so they stay alive.
    let _sysloop = EspSystemEventLoop::take()?;
    let _nvs = EspDefaultNvs::new()?;

    // MQTT client is owned by the publishing task.
    let mqtt_config = MqttClientConfiguration {
        client_id: Some("weather-station"),
        ..Default::default()
    };
    let (mut mqtt_client, mut connection) = EspMqttClient::new_with_conn(
        "mqtt://broker.local",
        &mqtt_config,
    )?;

    // Pump the MQTT event loop in the background so the connection stays alive.
    thread::spawn(move || {
        while let Some(event) = connection.next() {
            match event {
                Ok(event) => info!("MQTT event: {:?}", event),
                Err(err) => error!("MQTT error: {:?}", err),
            }
        }
    });

    // FreeRTOS queue shared between the DHT sampling task and the MQTT publisher.
    let queue: Arc<Queue<DhtReading>> = Arc::new(Queue::new(4)?);

    let dht_queue = queue.clone();
    thread::spawn(move || dht_task(dht_queue));

    let mqtt_queue = queue.clone();
    thread::spawn(move || mqtt_task(mqtt_queue, mqtt_client));

    // Keep the main task alive forever.
    loop {
        FreeRtos::delay_ms(1_000);
    }
}

fn dht_task(queue: Arc<Queue<DhtReading>>) {
    let mut fake_temperature = 20.0_f32;
    loop {
        // Replace this section with your real DHT sensor read.
        fake_temperature += 0.1;
        let reading = DhtReading {
            temperature_c: fake_temperature,
            humidity_rh: 60.0,
        };

        if let Err(err) = queue.send(&reading, TickType::from_millis(10)) {
            error!("dropping reading: {:?}", err);
        }

        FreeRtos::delay_ms(2_000);
    }
}

fn mqtt_task(queue: Arc<Queue<DhtReading>>, mut client: EspMqttClient) {
    let mut slot = DhtReading::default();

    loop {
        match queue.receive(&mut slot, TickType::from_secs(5)) {
            Ok(()) => {
                let payload = format!(
                    "{{\"temperature_c\":{:.2},\"humidity_rh\":{:.2}}}",
                    slot.temperature_c, slot.humidity_rh
                );

                if let Err(err) = client.publish(
                    "weather/readings/dht22",
                    QoS::AtLeastOnce,
                    false,
                    payload.as_bytes(),
                ) {
                    error!("failed to publish reading: {:?}", err);
                }
            }
            Err(err) => error!("MQTT task timed out waiting for new sample: {:?}", err),
        }
    }
}
