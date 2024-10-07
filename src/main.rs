use std::error::Error;
use network_listener::listener::{
    capture,
    logger,
};
use capture::capture_packets;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    logger::setup_logging()?;

    let handle = tokio::spawn(async move {
        let _res = capture_packets().await;
    });
    handle.await?;

    Ok(())
}
