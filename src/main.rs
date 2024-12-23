use ssri_server::run_server_main;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    run_server_main().await
}

