//! Offline fixture helper; no network access. Private seed is read only from a file.
use std::path::Path;
fn work() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("expected authority-seed-file claims-file".into());
    }
    let key = std::fs::read(Path::new(&args[0]))?;
    if key.len() > 128 {
        return Err("invalid key".into());
    }
    let seed: [u8; 32] = hex::decode(std::str::from_utf8(&key)?.trim())?
        .try_into()
        .map_err(|_| "invalid key")?;
    let bytes = std::fs::read(Path::new(&args[1]))?;
    if bytes.len() > 4096 {
        return Err("claims too large".into());
    }
    let claims: tavra_link::admission::LeaseClaims = serde_json::from_slice(&bytes)?;
    let lease =
        tavra_link::admission::issue_lease(&ed25519_dalek::SigningKey::from_bytes(&seed), claims)?;
    println!("{}", serde_json::to_string(&lease)?);
    Ok(())
}
fn main() {
    if work().is_err() {
        eprintln!("test lease issuance failed");
        std::process::exit(1)
    }
}
