#[tokio::main(worker_threads = 2)]
async fn main() {
    if let Err(message) = run().await {
        eprintln!("tavra-node: {message}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), String> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--config") {
        args.insert(0, "serve".into());
    }
    match args.as_slice() {
        [command] if command == "inspect" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&tavra_node::inspect())
                    .map_err(|_| "cannot serialize metrics")?
            );
            Ok(())
        }
        [command, flag, path]
            if (command == "serve" || command == "check-config") && flag == "--config" =>
        {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(path)
                .map_err(|_| "cannot read configuration")?
                .take(65537)
                .read_to_end(&mut bytes)
                .map_err(|_| "cannot read configuration")?;
            if bytes.len() > 65536 {
                return Err("configuration too large".into());
            }
            let config: tavra_node::Config =
                serde_json::from_slice(&bytes).map_err(|_| "invalid configuration")?;
            let tls = tavra_node::tls_config(&config)?;
            if command == "check-config" {
                println!("configuration valid");
                return Ok(());
            }
            let listener = tokio::net::TcpListener::bind(config.bind)
                .await
                .map_err(|_| "cannot bind management listener")?;
            tavra_node::serve(listener, tls)
                .await
                .map_err(|_| "management listener failed".into())
        }
        _ => Err(
            "usage: tavra-node inspect | check-config --config FILE | serve --config FILE".into(),
        ),
    }
}
