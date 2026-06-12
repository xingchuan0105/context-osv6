#[tokio::main]
async fn main() -> anyhow::Result<()> {
    avrag_worker::run().await
}
