#[tokio::main]
async fn main() -> anyhow::Result<()> {
    denki::app::run().await
}
