mod log_entry;
mod parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("ALS_ENGINE_READY port=0");
    Ok(())
}
