use anyhow::Result;
#[tokio::main]
async fn main() -> Result<()> {
    lattice_store::run().await
}
