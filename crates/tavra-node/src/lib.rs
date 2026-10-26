#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_real_proc_shapes_and_missing_values() {
        let m = parse_memory("MemTotal:       1024 kB\nMemAvailable:    512 kB\n");
        assert_eq!(m.total_bytes, Some(1048576));
        assert_eq!(m.available_bytes, Some(524288));
        assert_eq!(parse_memory("broken").total_bytes, None);
        assert_eq!(
            parse_load("0.20 1.25 2.00 1/100 42"),
            Some([0.2, 1.25, 2.0])
        );
        assert_eq!(parse_load("NaN 0 0"), None);
        let n = parse_network("Inter-| Receive\n eth0: 123 0 0 0 0 0 0 0 456 0 0 0 0 0 0 0\n");
        assert_eq!(n[0].received_bytes, 123);
        assert_eq!(n[0].transmitted_bytes, 456);
        assert_eq!(
            parse_ports("header\n 0: 0100007F:1F90 00000000:0000 0A rest", "tcp"),
            vec![ListeningPort {
                transport: "tcp".into(),
                port: 8080
            }]
        );
    }
    #[test]
    fn hostname_parser_limits_output_to_safe_hostname() {
        assert_eq!(parse_hostname("node-01\n"), Some("node-01".into()));
        assert_eq!(parse_hostname("node token=secret"), None);
        assert_eq!(parse_hostname(""), None);
    }
    #[test]
    fn unknown_config_fields_and_plaintext_are_rejected() {
        assert!(
            serde_json::from_str::<Config>(
                r#"{"server_cert":"a","server_key":"b","management_client_ca":"c","insecure":true}"#
            )
            .is_err()
        );
        let c: Config = serde_json::from_str(
            r#"{"server_cert":"a","server_key":"b","management_client_ca":"c"}"#,
        )
        .unwrap();
        assert_eq!(c.bind, "127.0.0.1:9443".parse().unwrap());
    }
}

use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, BufReader},
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Semaphore,
    time::timeout,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,
    pub server_cert: PathBuf,
    pub server_key: PathBuf,
    pub management_client_ca: PathBuf,
}
fn default_bind() -> SocketAddr {
    "127.0.0.1:9443".parse().unwrap()
}
#[derive(Serialize, Deserialize, Default)]
pub struct Memory {
    pub total_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
}
#[derive(Serialize, Deserialize)]
pub struct Network {
    pub interface: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
}
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct ListeningPort {
    pub transport: String,
    pub port: u16,
}
#[derive(Serialize, Deserialize)]
pub struct Disk {
    pub mount: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}
#[derive(Serialize, Deserialize)]
pub struct Capability {
    pub protocol: String,
    pub supported: bool,
    pub reason: String,
}
#[derive(Serialize, Deserialize)]
pub struct Metrics {
    pub schema_version: u32,
    pub sampled_at_unix_seconds: u64,
    pub os: String,
    pub architecture: String,
    pub hostname: Option<String>,
    pub os_release: Option<String>,
    pub load_average: Option<[f64; 3]>,
    pub memory: Memory,
    pub network: Vec<Network>,
    pub listening_ports: Vec<ListeningPort>,
    pub disks: Vec<Disk>,
    pub tun_device_present: bool,
    pub capabilities: Vec<Capability>,
    pub unavailable: Vec<String>,
}
pub fn parse_hostname(s: &str) -> Option<String> {
    let s = s.trim();
    if !s.is_empty()
        && s.len() <= 253
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
    {
        Some(s.into())
    } else {
        None
    }
}
pub fn parse_memory(s: &str) -> Memory {
    let value = |key: &str| {
        s.lines().find_map(|l| {
            let mut p = l.split_whitespace();
            if p.next()? != key {
                return None;
            }
            p.next()?.parse::<u64>().ok()?.checked_mul(1024)
        })
    };
    Memory {
        total_bytes: value("MemTotal:"),
        available_bytes: value("MemAvailable:"),
    }
}
pub fn parse_load(s: &str) -> Option<[f64; 3]> {
    let mut p = s.split_whitespace();
    let a = [
        p.next()?.parse().ok()?,
        p.next()?.parse().ok()?,
        p.next()?.parse().ok()?,
    ];
    if a.iter().all(|x: &f64| x.is_finite() && *x >= 0.0) {
        Some(a)
    } else {
        None
    }
}
pub fn parse_network(s: &str) -> Vec<Network> {
    s.lines()
        .filter_map(|l| {
            let (name, values) = l.split_once(':')?;
            let p: Vec<_> = values.split_whitespace().collect();
            Some(Network {
                interface: name.trim().into(),
                received_bytes: p.first()?.parse().ok()?,
                transmitted_bytes: p.get(8)?.parse().ok()?,
            })
        })
        .collect()
}
pub fn parse_ports(s: &str, transport: &str) -> Vec<ListeningPort> {
    s.lines()
        .skip(1)
        .filter_map(|l| {
            let p: Vec<_> = l.split_whitespace().collect();
            if transport == "tcp" && *p.get(3)? != "0A" {
                return None;
            }
            if transport == "udp" && *p.get(3)? != "07" {
                return None;
            }
            let port = u16::from_str_radix(p.get(1)?.rsplit_once(':')?.1, 16).ok()?;
            Some(ListeningPort {
                transport: transport.into(),
                port,
            })
        })
        .collect()
}
fn read_metric(path: &str, unavailable: &mut Vec<String>) -> String {
    fs::read_to_string(path).unwrap_or_else(|_| {
        unavailable.push(path.into());
        String::new()
    })
}
pub fn inspect() -> Metrics {
    let mut unavailable = Vec::new();
    let memory = parse_memory(&read_metric("/proc/meminfo", &mut unavailable));
    let load_average = parse_load(&read_metric("/proc/loadavg", &mut unavailable));
    let network = parse_network(&read_metric("/proc/net/dev", &mut unavailable));
    let mut listening_ports = Vec::new();
    for (path, proto) in [
        ("/proc/net/tcp", "tcp"),
        ("/proc/net/tcp6", "tcp"),
        ("/proc/net/udp", "udp"),
        ("/proc/net/udp6", "udp"),
    ] {
        listening_ports.extend(parse_ports(&read_metric(path, &mut unavailable), proto));
    }
    listening_ports.sort();
    listening_ports.dedup();
    let os_release = fs::read_to_string("/etc/os-release").ok().and_then(|s| {
        s.lines().find_map(|l| {
            l.strip_prefix("PRETTY_NAME=")
                .map(|s| s.trim_matches('"').to_owned())
        })
    });
    let mut disks = Vec::new();
    #[cfg(unix)]
    {
        let mut st = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: static NUL-terminated path and valid output pointer; initialized only on success.
        if unsafe { libc::statvfs(c"/".as_ptr(), st.as_mut_ptr()) } == 0 {
            let st = unsafe { st.assume_init() };
            // libc statvfs counters/block sizes use target-dependent integer widths:
            // macOS and Linux require explicit normalization to the JSON u64 byte contract.
            #[allow(clippy::unnecessary_cast)]
            disks.push(Disk {
                mount: "/".into(),
                total_bytes: (st.f_blocks as u64).saturating_mul(st.f_frsize as u64),
                available_bytes: (st.f_bavail as u64).saturating_mul(st.f_frsize as u64),
            });
        } else {
            unavailable.push("root filesystem statvfs".into());
        }
    }
    Metrics {
        schema_version: 1,
        sampled_at_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        os: std::env::consts::OS.into(),
        architecture: std::env::consts::ARCH.into(),
        hostname: parse_hostname(&read_metric("/proc/sys/kernel/hostname", &mut unavailable)),
        os_release,
        load_average,
        memory,
        network,
        listening_ports,
        disks,
        tun_device_present: std::path::Path::new("/dev/net/tun").exists(),
        capabilities: [
            "tavra-link",
            "wireguard",
            "amneziawg",
            "openvpn",
            "xray",
            "hysteria2",
            "trusttunnel",
            "naiveproxy",
            "dnstt",
            "ikev2",
        ]
        .into_iter()
        .map(|p| Capability {
            protocol: p.into(),
            supported: false,
            reason: "No managed runtime installed or verified by this agent".into(),
        })
        .collect(),
        unavailable,
    }
}
pub fn tls_config(c: &Config) -> Result<Arc<rustls::ServerConfig>, String> {
    let certs = |p: &PathBuf| -> Result<Vec<rustls::pki_types::CertificateDer<'static>>, String> {
        let f = fs::File::open(p).map_err(|_| "certificate file unavailable")?;
        rustls_pemfile::certs(&mut BufReader::new(f))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "invalid certificate PEM".into())
    };
    let mut roots = rustls::RootCertStore::empty();
    for ca in certs(&c.management_client_ca)? {
        roots.add(ca).map_err(|_| "invalid management CA")?;
    }
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(roots),
        Arc::new(rustls::crypto::ring::default_provider()),
    )
    .build()
    .map_err(|_| "management CA required")?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(
        fs::File::open(&c.server_key).map_err(|_| "server key unavailable")?,
    ))
    .map_err(|_| "invalid server key")?
    .ok_or("server private key required")?;
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| "TLS versions unavailable")?
    .with_client_cert_verifier(verifier)
    .with_single_cert(certs(&c.server_cert)?, key)
    .map_err(|_| "invalid server identity")?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}
pub async fn serve(listener: TcpListener, tls: Arc<rustls::ServerConfig>) -> io::Result<()> {
    let acceptor = tokio_rustls::TlsAcceptor::from(tls);
    let permits = Arc::new(Semaphore::new(32));
    loop {
        let (socket, _) = listener.accept().await?;
        let Ok(permit) = permits.clone().try_acquire_owned() else {
            drop(socket);
            continue;
        };
        let acceptor = acceptor.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let _=timeout(Duration::from_secs(10),async move {
        let mut stream=acceptor.accept(socket).await.map_err(io::Error::other)?;
        let mut header=Vec::new();let mut byte=[0u8;1];
        while header.len()<8192 && !header.ends_with(b"\r\n\r\n") {if stream.read(&mut byte).await?==0{return Ok::<_,io::Error>(())} header.push(byte[0]);}
        let request=std::str::from_utf8(&header).unwrap_or("");
        let line=request.split("\r\n").next().unwrap_or("");
        let (status,body)=if !header.ends_with(b"\r\n\r\n") {("431 Request Header Fields Too Large", "{}".into())} else {match line {
            "GET /v1/metrics HTTP/1.1" => ("200 OK",serde_json::to_string(&inspect()).map_err(io::Error::other)?),
            "GET /healthz HTTP/1.1" => ("200 OK",r#"{"status":"ok","scope":"management-agent","vpn_ready":false}"#.into()),
            l if !l.starts_with("GET ") => ("405 Method Not Allowed","{}".into()),
            _ => ("404 Not Found","{}".into()),
        }};
        let response=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",body.len());
        stream.write_all(response.as_bytes()).await?;stream.shutdown().await
    }).await;
        });
    }
}
