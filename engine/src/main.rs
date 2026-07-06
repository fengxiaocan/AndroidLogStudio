mod adb;
mod device;
mod device_manager;
mod filter;
mod log_entry;
mod parser;
mod recorder;
mod ring_buffer;
mod statistics;
mod websocket;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let token = std::env::var("ALS_ENGINE_TOKEN")?;
    let server = websocket::run_server(token).await?;
    println!("ALS_ENGINE_READY port={}", server.port);
    tokio::signal::ctrl_c().await?;
    Ok(())
}
