mod filter;
mod log_entry;
mod parser;
mod recorder;
mod ring_buffer;
mod statistics;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("ALS_ENGINE_READY port=0");
    Ok(())
}
