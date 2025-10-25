use core::net::Ipv4Addr;

use crate::config::CONFIG;
use embassy_net::{tcp::TcpSocket, IpAddress, Stack};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};
use embassy_time::{Duration, Timer, WithTimeout};
use esp_hal::rng::Rng;
use heapless::String;
use log::{error, info};
use rust_mqtt::{
    client::{client::MqttClient, client_config::ClientConfig},
    packet::v5::publish_packet::QualityOfService,
};
use smoltcp::wire::DnsQueryType;

pub const BUFFER_SIZE: usize = 2048;
pub const DEFAULT_STRING_SIZE: usize = 30;
pub const CHANNEL_SIZE: usize = 5;
pub static MQTT_CHANNEL: Channel<
    CriticalSectionRawMutex,
    String<DEFAULT_STRING_SIZE>,
    CHANNEL_SIZE,
> = Channel::new();

#[embassy_executor::task]
pub async fn mqtt_task(
    stack: Stack<'static>,
    _mqtt_receiver: Receiver<
        'static,
        CriticalSectionRawMutex,
        String<DEFAULT_STRING_SIZE>,
        CHANNEL_SIZE,
    >,
) {
    // info!("waiting for config");
    // match stack
    //     .wait_config_up()
    //     .with_timeout(Duration::from_secs(30))
    //     .await
    // {
    //     Ok(()) => info!("got config: {:?}", stack.config_v4()),
    //     Err(_) => error!("wait_config_up() errored"),
    // }
    // if !stack.is_config_up() {
    //     error!("No IP. Bailing.");
    //     return;
    // }

    info!("Waiting for link");
    match stack
        .wait_link_up()
        .with_timeout(Duration::from_secs(30))
        .await
    {
        Ok(()) => info!("link is up"),
        Err(_) => {
            error!("wait_link_up() timed out");
            return;
        }
    }

    if !stack.is_link_up() {
        error!("Link is not up. Bailing.");
        return;
    }

    // 2) Resolve broker (or skip DNS and hardcode IP)
    info!("Resolving: {}", CONFIG.broker_url);
    let addrs = match stack.dns_query(CONFIG.broker_url, DnsQueryType::A).await {
        Ok(a) if !a.is_empty() => a,
        Ok(_) => {
            error!("DNS: no A records");
            return;
        }
        Err(e) => {
            error!("DNS failed: {e:?}");
            return;
        }
    };

    let broker_ip = addrs.first().copied().unwrap(); // pick the first A record
                                                     // let broker_ip = IpAddress::Ipv4(Ipv4Addr::new(54, 36, 178, 49));
    let broker = (broker_ip, 1883u16);

    // 3) Create a TCP socket
    let mut tcp_rx = [0; BUFFER_SIZE];
    let mut tcp_tx = [0; BUFFER_SIZE];

    let mut socket = TcpSocket::new(stack, &mut tcp_rx, &mut tcp_tx);
    socket.set_timeout(Some(Duration::from_secs(10)));

    // 4) Connect TCP
    socket.connect(broker).await.unwrap();

    let rng = Rng::new();
    let mut config: ClientConfig<'static, 16, Rng> =
        rust_mqtt::client::client_config::ClientConfig::new(
            rust_mqtt::client::client_config::MqttVersion::MQTTv5,
            rng,
        );
    config.add_max_subscribe_qos(rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS1);
    config.add_client_id("weather_station");

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

    if let Err(e) = client
        .send_message(
            "weather_station/test",
            "Hello guyzzzzzzz".as_bytes(),
            QualityOfService::QoS1,
            true,
        )
        .await
    {
        error!("An error occured sending the message: {e}");
    }
    info!("Message sent");
    Timer::after_secs(2).await;
}
