use kpower_server::{app::Application, config::get_config};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().expect("Unable to load environment variables from .env file");
    let config = get_config().expect("Failed to read configuration.");
    Application::build(config).await?;
    Ok(())
}
