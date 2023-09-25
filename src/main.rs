use kpower_server::app::Application;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Application::build().await;
    Ok(())
}
