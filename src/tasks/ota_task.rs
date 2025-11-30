use crate::config::CONFIG;
use embassy_net::{
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
};
use embassy_time::{Duration, with_timeout};
use embedded_io_async::Read as _;
use esp_hal::peripherals::FLASH;
use esp_hal::peripherals::TIMG1;
use esp_hal::timer::timg::Wdt;
use esp_hal_ota::Ota;
use esp_storage::FlashStorage;
use log::{error, info};
use reqwless::{
    client::HttpClient,
    request::Method,
    response::{HeaderIterator, Response},
};
use static_cell::StaticCell;

const NB_CON: usize = 1;
const RX_SIZE: usize = 4096;
const TX_SIZE: usize = 1024;
const TIMEOUT_SECS: u64 = 1;

type OtaType = Ota<FlashStorage<'static>>;

static OTA_CELL: StaticCell<OtaType> = StaticCell::new();

pub async fn ota_task(
    stack: embassy_net::Stack<'static>,
    ota_handle: &'static mut OtaType,
    watchdog: &mut Wdt<TIMG1<'_>>,
) {
    let state = mk_static!(
        TcpClientState<NB_CON, TX_SIZE, RX_SIZE>,
        TcpClientState::new()
    );

    let tcp = TcpClient::new(stack, state);
    let dns = DnsSocket::new(stack);
    let mut client = HttpClient::new(&tcp, &dns);

    info!("checking updates..");

    if let Ok(res) = with_timeout(
        Duration::from_secs(TIMEOUT_SECS),
        client.request(Method::GET, CONFIG.ota_url),
    )
    .await
    {
        match res {
            Ok(mut req) => {
                let mut rx_buff = [0u8; RX_SIZE];
                match req.send(&mut rx_buff).await {
                    Ok(response) => {
                        do_update(response, ota_handle, watchdog).await;
                    }
                    Err(e) => {
                        error!("Sending the request: {:?}", e);
                    }
                }
            }
            Err(e) => error!("Error: {:?}", e),
        }
    }
    info!("No update found! continuing...");
}

pub fn init_ota(flash: FLASH<'static>) -> &'static mut OtaType {
    OTA_CELL.init_with(|| {
        let storage = FlashStorage::new(flash);
        Ota::new(storage).expect("Cannot create OTA")
    })
}

pub async fn do_update<'resp, 'buf, C>(
    response: Response<'resp, 'buf, C>,
    ota_handle: &mut OtaType,
    watchdog: &mut Wdt<TIMG1<'_>>,
) where
    C: embedded_io_async::Read,
{
    let flash_size = response.content_length.unwrap_or_default() as u32;
    let target_crc = get_crc(response.headers());

    info!(
        "OTA: flash_size = {}, target_crc = {}",
        flash_size, target_crc
    );

    ota_handle
        .ota_begin(flash_size, target_crc)
        .expect("Fail starting the OTA!");

    let mut reader = response.body().reader();
    let mut chunk = [0u8; RX_SIZE];
    let mut bytes_sent: u32 = 0;

    loop {
        // Only read up to the remaining bytes
        let remaining = flash_size - bytes_sent;
        if remaining == 0 {
            break;
        }

        let to_read = core::cmp::min(chunk.len(), remaining as usize);

        watchdog.feed();
        let n = reader.read(&mut chunk[..to_read]).await.unwrap();
        info!("OTA: read {} bytes", n);

        if n == 0 {
            error!(
                "OTA: unexpected EOF after {} bytes (expected {})",
                bytes_sent, flash_size
            );
            break;
        }

        bytes_sent += n as u32;

        let res = ota_handle.ota_write_chunk(&chunk[..n]);
        info!("OTA: ota_write_chunk -> {:?}", res);

        // led.set_low();
        if res == Ok(true) {
            info!("OTA: write_chunk reports completion, flushing...");
            match ota_handle.ota_flush(true, true) {
                Ok(_) => {
                    info!("Valid image received, restarting!");

                    esp_hal::system::software_reset();
                }
                Err(e) => {
                    error!("OTA: flush error: {:?}", e);
                }
            }
            break;
        }
    }

    info!("OTA: total bytes sent to OTA: {}", bytes_sent);
}

pub fn get_crc(headers: HeaderIterator) -> u32 {
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("target_crc") {
            info!("got crc: {:?}", value);
            let s = core::str::from_utf8(value).unwrap_or("0");
            if let Ok(crc) = s.trim().parse::<u32>() {
                return crc;
            }
        }
    }
    0
}
