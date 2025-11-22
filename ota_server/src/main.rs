use anyhow::Result;
use anyhow::anyhow;
use clap::Parser;
use crc32fast::Hasher;
use log::{error, info};
use std::fs;
use std::io::Cursor;
use tiny_http::{Header, Request, Response};
use tiny_http::{Server, StatusCode};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    ip_addr: String,

    #[arg(short, long)]
    port: u16,

    #[arg(short, long)]
    file: String,
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    let address = format!("{}:{}", args.ip_addr, args.port);
    let server = Server::http(&address).unwrap();
    info!("Serving {} runnning at {}", args.file, address);

    for request in server.incoming_requests() {
        if let Some(addr) = request.remote_addr() {
            info!("Received request from {addr}");
        }
        match handle_ota_update(request, &args.file) {
            Ok(_) => {
                info!("Firmware succefully updated, shuting down !");
                break;
            }
            Err(e) => {
                error!("{}", e);
                break;
            }
        }
    }
}

pub fn handle_ota_update(request: Request, file: &str) -> Result<()> {
    let firmware = match fs::read(file) {
        Ok(data) => data,
        Err(e) => {
            error!("Fail to read {}: {}", file, e);
            return Err(e.into());
        }
    };

    let mut hasher = Hasher::new();
    hasher.update(&firmware);
    let crc = hasher.finalize();

    let crc_str = crc.to_string();
    let header_crc = Header::from_bytes(&b"target_crc"[..], crc_str.as_bytes())
        .map_err(|_| anyhow!("Invalid header: target_crc"))?;
    let header_ct = Header::from_bytes(&b"Content-Type"[..], &b"application/octet-stream"[..])
        .map_err(|_| anyhow!("Invalid header: content-type"))?;

    let flash_size = firmware.len();
    let body = Cursor::new(firmware);
    let response = Response::new(
        StatusCode(200),
        vec![header_ct, header_crc],
        body,
        Some(flash_size),
        None,
    )
    .with_chunked_threshold(usize::MAX);

    request.respond(response)?;

    Ok(())
}
