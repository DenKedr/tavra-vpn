use serde::Deserialize;
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    path::PathBuf,
};
#[cfg(target_os = "linux")]
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
#[derive(Deserialize)]
struct Config {
    interface: String,
    own_ip: Ipv4Addr,
    peer_ip: Ipv4Addr,
    mtu: usize,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    bind: SocketAddrV4,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    #[serde(flatten)]
    mode: Mode,
}
#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
enum Mode {
    Server {
        cert_file: PathBuf,
        key_file: PathBuf,
        authority_key: String,
        node_id: String,
        epoch: u64,
    },
    Client {
        server_addr: SocketAddrV4,
        server_name: String,
        root_cert_file: PathBuf,
        device_key_file: PathBuf,
        signed_lease_file: PathBuf,
    },
}
fn valid(c: &Config) -> bool {
    !c.interface.is_empty()
        && c.interface.len() < 16
        && c.interface
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        && (576..=1100).contains(&c.mtu)
        && c.own_ip != c.peer_ip
        && [c.own_ip, c.peer_ip]
            .iter()
            .all(|ip| ip.is_private() && !ip.is_unspecified() && !ip.is_multicast())
}
#[cfg(any(target_os = "linux", test))]
fn packet_allowed(p: &[u8], src: Ipv4Addr, dst: Ipv4Addr, mtu: usize) -> bool {
    p.len() >= 20
        && p.len() <= mtu
        && p[0] >> 4 == 4
        && (p[0] & 15) >= 5
        && usize::from(p[0] & 15) * 4 <= p.len()
        && usize::from(u16::from_be_bytes([p[2], p[3]])) == p.len()
        && p[12..16] == src.octets()
        && p[16..20] == dst.octets()
}
#[cfg(any(target_os = "linux", test))]
fn certs(path: &std::path::Path) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
    let bytes = std::fs::read(path)?;
    let cs = rustls_pemfile::certs(&mut &bytes[..]).collect::<std::result::Result<Vec<_>, _>>()?;
    if cs.is_empty() {
        return Err("certificate required".into());
    }
    Ok(cs)
}
#[cfg(any(target_os = "linux", test))]
fn roots(path: &std::path::Path) -> Result<rustls::RootCertStore> {
    let mut roots = rustls::RootCertStore::empty();
    for c in certs(path)? {
        roots.add(c)?;
    }
    Ok(roots)
}
#[cfg(target_os = "linux")]
fn transport() -> Arc<quinn::TransportConfig> {
    let mut t = quinn::TransportConfig::default();
    t.max_concurrent_bidi_streams(1u32.into())
        .max_concurrent_uni_streams(0u32.into());
    t.datagram_receive_buffer_size(Some(64 * 1024))
        .datagram_send_buffer_size(64 * 1024);
    t.receive_window(65536u32.into())
        .stream_receive_window(8192u32.into());
    t.max_idle_timeout(Some(Duration::from_secs(15).try_into().unwrap()));
    Arc::new(t)
}
#[cfg(target_os = "linux")]
fn now() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs()
        .try_into()?)
}
#[cfg(target_os = "linux")]
mod tun {
    use super::*;
    use std::{
        io,
        os::fd::{AsRawFd, FromRawFd, OwnedFd},
    };
    pub struct Tun(tokio::io::unix::AsyncFd<OwnedFd>);
    impl Tun {
        pub fn create(name: &str) -> Result<Self> {
            // Nonpersistent TUN disappears on fd close; TUN_EXCL forbids attaching an existing interface.
            let fd = unsafe {
                libc::open(
                    c"/dev/net/tun".as_ptr(),
                    libc::O_RDWR | libc::O_NONBLOCK | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error().into());
            }
            let fd = unsafe { OwnedFd::from_raw_fd(fd) };
            let mut req: libc::ifreq = unsafe { std::mem::zeroed() };
            for (a, b) in req.ifr_name.iter_mut().zip(name.bytes()) {
                *a = b as libc::c_char;
            }
            req.ifr_ifru.ifru_flags =
                (libc::IFF_TUN | libc::IFF_NO_PI | libc::IFF_TUN_EXCL) as libc::c_short;
            if unsafe { libc::ioctl(fd.as_raw_fd(), 0x400454ca as libc::c_ulong, &mut req) } < 0 {
                return Err(io::Error::last_os_error().into());
            }
            Ok(Self(tokio::io::unix::AsyncFd::new(fd)?))
        }
        pub async fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
            loop {
                let mut ready = self.0.readable().await?;
                match ready.try_io(|fd| {
                    let n =
                        unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
                    if n < 0 {
                        Err(io::Error::last_os_error())
                    } else {
                        Ok(n as usize)
                    }
                }) {
                    Ok(v) => return v,
                    Err(_) => continue,
                }
            }
        }
        pub async fn write(&self, buf: &[u8]) -> io::Result<()> {
            loop {
                let mut ready = self.0.writable().await?;
                match ready.try_io(|fd| {
                    let n = unsafe { libc::write(fd.as_raw_fd(), buf.as_ptr().cast(), buf.len()) };
                    if n < 0 {
                        Err(io::Error::last_os_error())
                    } else if n as usize != buf.len() {
                        Err(io::Error::other("short packet write"))
                    } else {
                        Ok(())
                    }
                }) {
                    Ok(v) => return v,
                    Err(_) => continue,
                }
            }
        }
    }
}
#[cfg(target_os = "linux")]
async fn run(c: Config) -> Result<()> {
    use tavra_link::{
        admission::SignedLease,
        transport::{accept, connect},
    };
    let (endpoint, session) = match &c.mode {
        Mode::Server {
            cert_file,
            key_file,
            authority_key,
            node_id,
            epoch,
        } => {
            if node_id.is_empty() || node_id.len() > 128 || *epoch == 0 {
                return Err("invalid node".into());
            }
            let public: [u8; 32] = hex::decode(authority_key)?
                .try_into()
                .map_err(|_| "invalid authority")?;
            let authority = ed25519_dalek::VerifyingKey::from_bytes(&public)?;
            let keybytes = std::fs::read(key_file)?;
            let key = rustls_pemfile::private_key(&mut &keybytes[..])?.ok_or("key required")?;
            let mut tls = rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs(cert_file)?, key)?;
            tls.alpn_protocols = vec![b"tavra-link/0".to_vec()];
            tls.max_early_data_size = 0;
            let crypto = quinn::crypto::rustls::QuicServerConfig::try_from(tls)?;
            let mut server = quinn::ServerConfig::with_crypto(Arc::new(crypto));
            server.transport_config(transport());
            let endpoint = quinn::Endpoint::server(server, c.bind.into())?;
            let incoming = tokio::time::timeout(Duration::from_secs(60), endpoint.accept())
                .await?
                .ok_or("endpoint closed")?;
            endpoint.set_server_config(None);
            let conn = tokio::time::timeout(Duration::from_secs(5), incoming).await??;
            let session = accept(conn, &authority, node_id, *epoch, now()?).await?;
            (endpoint, session)
        }
        Mode::Client {
            server_addr,
            server_name,
            root_cert_file,
            device_key_file,
            signed_lease_file,
        } => {
            let bytes = std::fs::read(device_key_file)?;
            if bytes.len() > 128 {
                return Err("invalid key".into());
            }
            let seed: [u8; 32] = hex::decode(std::str::from_utf8(&bytes)?.trim())?
                .try_into()
                .map_err(|_| "invalid key")?;
            let device = ed25519_dalek::SigningKey::from_bytes(&seed);
            let leasebytes = std::fs::read(signed_lease_file)?;
            if leasebytes.len() > 4096 {
                return Err("invalid lease".into());
            }
            let lease: SignedLease = serde_json::from_slice(&leasebytes)?;
            let mut tls = rustls::ClientConfig::builder()
                .with_root_certificates(roots(root_cert_file)?)
                .with_no_client_auth();
            tls.alpn_protocols = vec![b"tavra-link/0".to_vec()];
            tls.enable_early_data = false;
            let mut client = quinn::ClientConfig::new(Arc::new(
                quinn::crypto::rustls::QuicClientConfig::try_from(tls)?,
            ));
            client.transport_config(transport());
            let mut endpoint = quinn::Endpoint::client(c.bind.into())?;
            endpoint.set_default_client_config(client);
            let conn = tokio::time::timeout(
                Duration::from_secs(5),
                endpoint.connect((*server_addr).into(), server_name)?,
            )
            .await??;
            let session = connect(conn, lease, &device, now()?).await?;
            (endpoint, session)
        }
    };
    let tun = tun::Tun::create(&c.interface)?;
    eprintln!("admitted; test TUN ready (configure private addresses and MTU externally)");
    let mut buf = vec![0u8; 65536];
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let result:Result<()>=async{loop{tokio::select!{
        _=tokio::signal::ctrl_c()=>break,
        _=term.recv()=>break,
        n=tun.read(&mut buf)=>{let n=n?;if packet_allowed(&buf[..n],c.own_ip,c.peer_ip,c.mtu){session.send_packet(&buf[..n])?;}},
        p=session.receive_packet()=>{let p=p?;if packet_allowed(&p,c.peer_ip,c.own_ip,c.mtu){tokio::time::timeout(Duration::from_secs(1),tun.write(&p)).await??;}}
    }}Ok(())}.await;
    session.close();
    drop(tun);
    endpoint.close(0u32.into(), b"closed");
    result
}
#[tokio::main]
async fn main() {
    if entry().await.is_err() {
        #[cfg(target_os = "linux")]
        eprintln!(
            "tavra-link-cli: failed (configuration, privilege, trust, admission or transport); no forwarding remains"
        );
        #[cfg(not(target_os = "linux"))]
        eprintln!("tavra-link-cli: Unsupported; Linux required; no network changes made");
        std::process::exit(1)
    }
}
async fn entry() -> Result<()> {
    let path = std::env::args_os().nth(1).ok_or("config required")?;
    let bytes = std::fs::read(path)?;
    if bytes.len() > 16384 {
        return Err("config too large".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let map = value.as_object().ok_or("config object required")?;
    let common = ["interface", "own_ip", "peer_ip", "mtu", "bind", "mode"];
    let extra: &[&str] = match map.get("mode").and_then(|v| v.as_str()) {
        Some("server") => &["cert_file", "key_file", "authority_key", "node_id", "epoch"],
        Some("client") => &[
            "server_addr",
            "server_name",
            "root_cert_file",
            "device_key_file",
            "signed_lease_file",
        ],
        _ => return Err("invalid mode".into()),
    };
    if map
        .keys()
        .any(|k| !common.contains(&k.as_str()) && !extra.contains(&k.as_str()))
    {
        return Err("unknown config field".into());
    }
    let c: Config = serde_json::from_value(value)?;
    if !valid(&c) {
        return Err("invalid config".into());
    }
    #[cfg(target_os = "linux")]
    {
        run(c).await
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = c;
        Err("Unsupported: Linux required".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        serde_json::from_str(r#"{"interface":"tavra0","own_ip":"10.250.0.1","peer_ip":"10.250.0.2","mtu":1100,"bind":"0.0.0.0:0","mode":"server","cert_file":"cert.pem","key_file":"key.pem","authority_key":"00","node_id":"test","epoch":1}"#).unwrap()
    }
    #[test]
    fn config_bounds() {
        let mut c = config();
        assert!(valid(&c));
        c.mtu = 1101;
        assert!(!valid(&c));
        c.mtu = 1100;
        c.interface = "bad/name".into();
        assert!(!valid(&c));
        c.interface = "x".repeat(16);
        assert!(!valid(&c));
    }
    #[test]
    fn ipv4_exact_policy() {
        let a = Ipv4Addr::new(10, 250, 0, 1);
        let b = Ipv4Addr::new(10, 250, 0, 2);
        let mut p = vec![0u8; 20];
        p[0] = 0x45;
        p[3] = 20;
        p[12..16].copy_from_slice(&a.octets());
        p[16..20].copy_from_slice(&b.octets());
        assert!(packet_allowed(&p, a, b, 1100));
        assert!(!packet_allowed(&p, b, a, 1100));
        p[0] = 0x65;
        assert!(!packet_allowed(&p, a, b, 1100));
        p[0] = 0x45;
        p[3] = 21;
        assert!(!packet_allowed(&p, a, b, 1100));
    }
    #[test]
    fn invalid_trust_rejected() {
        let p = std::env::temp_dir().join(format!("tavra-invalid-root-{}", std::process::id()));
        std::fs::write(&p, b"not a certificate").unwrap();
        assert!(roots(&p).is_err());
        std::fs::remove_file(p).unwrap();
    }
}
