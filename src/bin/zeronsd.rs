use std::time::Duration;

use zeronsd::cli::init;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    init().await?;

    loop {
        tokio::time::sleep(Duration::MAX).await
    }
}
