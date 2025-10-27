use alloc::format;
use core::net::IpAddr;
use core::result::Result;
use core::str::FromStr;
use embassy_net::IpAddress;
use rust_mqtt::packet::v5::publish_packet::QualityOfService;

use crate::config::CONFIG;
use embassy_net::dns::DnsQueryType;
use embassy_net::{tcp::TcpSocket, Stack};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};
use embassy_time::{Duration, TimeoutError, WithTimeout};
use esp_hal::rng::Rng;
use heapless::String;
use log::{debug, error, info};
use rust_mqtt::client::{client::MqttClient, client_config::ClientConfig};

pub const SOCKET_TIMEOUT: u64 = 120;
pub const BUFFER_SIZE: usize = 2048;
pub const DEFAULT_STRING_SIZE: usize = 70;
pub const PAYLOAD_SIZE: usize = 20;
pub const TOPIC_SIZE: usize = 70;
pub const CHANNEL_SIZE: usize = 5;
pub static MQTT_CHANNEL: Channel<CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE> =
    Channel::new();

#[derive(Debug)]
pub struct MqttPacket {
    topic: String<TOPIC_SIZE>,
    payload: String<PAYLOAD_SIZE>,
}

impl MqttPacket {
    pub fn new(topic: &str, payload: &str) -> Self {
        let topic_string = String::from_str(topic).unwrap_or_default();
        let payload_string = String::from_str(payload).unwrap_or_default();
        MqttPacket {
            topic: topic_string,
            payload: payload_string,
        }
    }
}

#[embassy_executor::task]
pub async fn mqtt_task(
    stack: Stack<'static>,
    mqtt_receiver: Receiver<'static, CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE>,
) {
    wait_for_stack(&stack)
        .await
        .inspect(|_| info!("Got config: {:?}", stack.config_v4()))
        .unwrap(); // crash if the stack never gets up

    let broker = (
        IpAddress::from_str(CONFIG.broker_ip).unwrap(),
        CONFIG.broker_port,
    );
    debug!("Broker address: {:#?}", broker);

    // Create a TCP socket
    let mut tcp_rx = [0; BUFFER_SIZE];
    let mut tcp_tx = [0; BUFFER_SIZE];
    let mut socket = TcpSocket::new(stack, &mut tcp_rx, &mut tcp_tx);
    socket.set_timeout(Some(Duration::from_secs(SOCKET_TIMEOUT)));
    socket.connect(broker).await.unwrap();

    // Create mqtt client
    let rng = Rng::new();
    let mut config: ClientConfig<'static, 16, Rng> =
        rust_mqtt::client::client_config::ClientConfig::new(
            rust_mqtt::client::client_config::MqttVersion::MQTTv5,
            rng,
        );
    config.add_max_subscribe_qos(rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS1);
    config.add_client_id("esp_client");
    config.add_username(CONFIG.mqtt_user);
    config.add_password(CONFIG.mqtt_pass);

    let mut mqtt_rx = [0; BUFFER_SIZE];
    let mut mqtt_tx = [0; BUFFER_SIZE];

    let mut client = MqttClient::new(
        socket,
        &mut mqtt_tx,
        BUFFER_SIZE,
        &mut mqtt_rx,
        BUFFER_SIZE,
        config,
    );

    client
        .connect_to_broker()
        .await
        .expect("Couldnt connect to broker!");

    loop {
        let received = mqtt_receiver.receive().await;
        info!("topic: {}, payload: {}", received.topic, received.payload);

        client
            .send_message(
                received.topic.as_str(),
                received.payload.as_bytes(),
                QualityOfService::QoS1,
                true,
            )
            .await
            .map_err(|e| error!("Error sending mqtt packet: {:?}", e))
            .ok();
    }
}

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
