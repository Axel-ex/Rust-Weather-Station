//publish a mqtt packet
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

        let _ = ::core::fmt::Write::write_fmt(&mut payload, ::core::format_args!("{}", $val));

        $sender
            .send($crate::tasks::mqtt_task::MqttPacket::new(topic, payload))
            .await;
    }};
}

//create a static variable with static lifetime
#[macro_export]
macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        STATIC_CELL.init_with(|| $val)
    }};
}
