mod log_entry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("ALS_ENGINE_READY port=0");
    Ok(())
}
