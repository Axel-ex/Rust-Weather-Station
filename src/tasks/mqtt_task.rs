use crate::config::CONFIG;
use embassy_net::dns::DnsQueryType;
use embassy_net::{tcp::TcpSocket, Stack};
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

pub const BUFFER_SIZE: usize = 2048;
pub const DEFAULT_STRING_SIZE: usize = 30;
pub const CHANNEL_SIZE: usize = 5;
pub static MQTT_CHANNEL: Channel<
    CriticalSectionRawMutex,
    String<DEFAULT_STRING_SIZE>,
    CHANNEL_SIZE,
> = Channel::new();

async fn try_connect(
    stack: embassy_net::Stack<'static>,
    addr: embassy_net::IpAddress,
    port: u16,

    label: &str,
) {
    let mut rx = [0; 1024];
    let mut tx = [0; 1024];
    let mut s = TcpSocket::new(stack, &mut rx, &mut tx);
    s.set_timeout(Some(Duration::from_secs(5)));

    match s.connect((addr, port)).await {
        Ok(()) => log::info!("{}: CONNECT OK", label),
        Err(e) => log::error!("{}: CONNECT ERR: {:?}", label, e),
    }
}

async fn tcp_probe(stack: embassy_net::Stack<'static>) {
    use embassy_net::{IpAddress, Ipv4Address};

    try_connect(
        stack,
        IpAddress::Ipv4(Ipv4Address::new(192, 168, 1, 1)),
        443,
        "gateway:443",
    )
    .await;
    try_connect(
        stack,
        IpAddress::Ipv4(Ipv4Address::new(93, 184, 215, 14)),
        80,
        "example.com:80",
    )
    .await;
    try_connect(
        stack,
        IpAddress::Ipv4(Ipv4Address::new(1, 1, 1, 1)),
        53,
        "1.1.1.1:53 (TCP)",
    )
    .await;
    try_connect(
        stack,
        IpAddress::Ipv4(Ipv4Address::new(34, 223, 228, 31)),
        1883,
        "emqx:1883",
    )
    .await;
}

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
    info!("waiting for config");
    match stack
        .wait_config_up()
        .with_timeout(Duration::from_secs(30))
        .await
    {
        Ok(()) => info!("got config: {:?}", stack.config_v4()),
        Err(_) => error!("wait_config_up() errored"),
    }
    if !stack.is_config_up() {
        error!("No IP. Bailing.");
        return;
    }

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
            info!("testing tcp connections");
            tcp_probe(stack).await;
            return;
        }
    };

    let broker_ip = addrs.first().copied().unwrap(); // pick the first A record
                                                     // let broker_ip = IpAddress::Ipv4(Ipv4Addr::new(54, 36, 178, 49));
    let broker = (broker_ip, 1883u16);
    info!("Broker address: {:#?}", broker);

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
