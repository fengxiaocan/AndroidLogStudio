mod adb;
mod device;
mod device_manager;
mod filter;
mod log_entry;
mod parser;
mod pid_cache;
mod recorder;
mod ring_buffer;
mod statistics;
mod websocket;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let token = std::env::var("ALS_ENGINE_TOKEN")?;
    let server = websocket::run_server(token).await?;
    // Flush is required: when Electron pipes stdout, println! buffers and the
    // parent never sees ALS_ENGINE_READY until the process exits.
    use std::io::Write;
    println!("ALS_ENGINE_READY port={}", server.port);
    std::io::stdout().flush()?;
    tokio::signal::ctrl_c().await?;
    Ok(())
}
