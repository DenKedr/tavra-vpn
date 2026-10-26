#[tokio::main]
async fn main() {
    if run().await.is_err() {
        eprintln!(
            "TAVRA control startup or server failed; check configuration and database availability"
        );
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = tavra_control::config::Config::from_env()?;
    let state =
        tavra_control::AppState::connect(&config.database_url, config.bootstrap_secret).await?;
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    axum::serve(listener, tavra_control::router(state)).await?;
    Ok(())
}
