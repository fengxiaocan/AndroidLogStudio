mod adb;
mod device;
mod filter;
mod log_entry;
mod parser;
mod recorder;
mod ring_buffer;
mod statistics;
mod websocket;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let port = websocket::run_server().await?;
    println!("ALS_ENGINE_READY port={port}");
    tokio::signal::ctrl_c().await?;
    Ok(())
}
