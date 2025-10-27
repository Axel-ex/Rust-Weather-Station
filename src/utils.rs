use embassy_net::Stack;
use embassy_time::{Duration, TimeoutError, WithTimeout};

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

#[macro_export]
macro_rules! publish {
    ($sender:expr, $suffix:expr, $val:expr) => {{
        // Absolute paths + $crate to avoid hygiene issues.
        let mut topic: ::heapless::String<{ $crate::config::TOPIC_SIZE }> =
            ::heapless::String::new();
        let mut payload: ::heapless::String<{ $crate::config::PAYLOAD_SIZE }> =
            ::heapless::String::new();

        let _ = ::core::fmt::Write::write_fmt(
            &mut topic,
            ::core::format_args!("{}/{}", $crate::config::CONFIG.topic, $suffix),
        );
        // This is effectively "{}"
        let _ = ::core::fmt::Write::write_fmt(&mut payload, ::core::format_args!("{}", $val));

        $sender
            .send($crate::tasks::mqtt_task::MqttPacket::new(topic, payload))
            .await;
    }};
}
