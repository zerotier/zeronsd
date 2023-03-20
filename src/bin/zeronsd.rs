use zeronsd::cli::init;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    init().await
}
