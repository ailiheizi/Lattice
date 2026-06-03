use anyhow::Result;
#[tokio::main]
async fn main() -> Result<()> {
    nextim_store::run().await
}
