use core::str::FromStr;
use embassy_net::IpAddress;
use rust_mqtt::packet::v5::publish_packet::QualityOfService;

use crate::config::BUFFER_SIZE;
use crate::config::{CHANNEL_SIZE, CONFIG, PAYLOAD_SIZE, SOCKET_TIMEOUT, TOPIC_SIZE};
use embassy_net::{tcp::TcpSocket, Stack};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};
use embassy_time::{Duration, Timer};
use esp_hal::rng::Rng;
use heapless::String;
use log::{debug, error, info};
use rust_mqtt::client::{client::MqttClient, client_config::ClientConfig};

pub static MQTT_CHANNEL: Channel<CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE> =
    Channel::new();

#[derive(Debug)]
pub struct MqttPacket {
    topic: String<TOPIC_SIZE>,
    payload: String<PAYLOAD_SIZE>,
}

impl MqttPacket {
    pub fn new(topic: String<TOPIC_SIZE>, payload: String<PAYLOAD_SIZE>) -> Self {
        MqttPacket { topic, payload }
    }
}

#[embassy_executor::task]
pub async fn mqtt_task(
    stack: Stack<'static>,
    mqtt_receiver: Receiver<'static, CriticalSectionRawMutex, MqttPacket, CHANNEL_SIZE>,
) {
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
        Timer::after_millis(500).await;
    }
}
