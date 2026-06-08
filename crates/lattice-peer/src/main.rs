#[tokio::main]
async fn main() -> anyhow::Result<()> {
    lattice_peer::run().await
}
