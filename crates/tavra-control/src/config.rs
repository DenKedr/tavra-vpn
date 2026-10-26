use std::net::SocketAddr;
/// Deliberately no Debug: configuration contains credentials.
pub struct Config {
    pub bind: SocketAddr,
    pub database_url: String,
    pub bootstrap_secret: String,
}
impl Config {
    pub fn from_env() -> Result<Self, &'static str> {
        let database_url =
            std::env::var("TAVRA_DATABASE_URL").map_err(|_| "TAVRA_DATABASE_URL required")?;
        let bootstrap_secret = std::env::var("TAVRA_BOOTSTRAP_SECRET").unwrap_or_default();
        if !bootstrap_secret.is_empty() && bootstrap_secret.len() < 32 {
            return Err("Bootstrap secret must have at least 32 bytes");
        }
        let bind = std::env::var("TAVRA_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8787".into())
            .parse()
            .map_err(|_| "Invalid TAVRA_BIND")?;
        Ok(Self {
            bind,
            database_url,
            bootstrap_secret,
        })
    }
}
