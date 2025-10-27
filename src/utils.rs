use crate::config::{CONFIG, PAYLOAD_SIZE, TOPIC_SIZE};
use crate::tasks::mqtt_task::{MqttPacket, MQTT_CHANNEL};
use core::fmt::Write;
use embassy_net::Stack;
use embassy_time::{Duration, TimeoutError, Timer, WithTimeout};
use heapless::String;

pub async fn wait_for_stack(stack: &Stack<'static>) -> Result<(), TimeoutError> {
    stack
        .wait_config_up()
        .with_timeout(Duration::from_secs(30))
        .await?;

    stack
        .wait_link_up()
        .with_timeout(Duration::from_secs(30))
        .await?;

    Ok(())
}

pub async fn publish_rain() {
    Timer::after_secs(2).await;
    let mut topic = String::<TOPIC_SIZE>::new();
    let mut payload = String::<PAYLOAD_SIZE>::new();
    write!(&mut topic, "{}/rain", CONFIG.topic).unwrap();
    write!(&mut payload, "0.231").unwrap();

    let packet = MqttPacket::new(&topic, &payload);
    MQTT_CHANNEL.send(packet).await;
}
