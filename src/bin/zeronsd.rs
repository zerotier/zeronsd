use zeronsd::cli::init;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    Ok(init().await?)
}
