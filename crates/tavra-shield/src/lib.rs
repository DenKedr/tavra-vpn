//! Bounded domain filtering. No OS DNS settings or network activity.
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    net::IpAddr,
    sync::atomic::{AtomicU64, Ordering},
};

pub const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_RULES: usize = 50_000;
const MAX_LINE_BYTES: usize = 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Ads,
    Trackers,
    Malware,
    Annoyances,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub ads: bool,
    pub trackers: bool,
    pub malware: bool,
    pub annoyances: bool,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            ads: true,
            trackers: true,
            malware: true,
            annoyances: true,
        }
    }
}
impl Policy {
    fn enabled(self, c: Category) -> bool {
        match c {
            Category::Ads => self.ads,
            Category::Trackers => self.trackers,
            Category::Malware => self.malware,
            Category::Annoyances => self.annoyances,
        }
    }
}
pub struct Source<'a> {
    pub category: Category,
    pub text: &'a str,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub &'static str);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Error {}
#[derive(Debug, Clone)]
pub struct RejectedRule {
    pub source: usize,
    pub line: usize,
    pub reason: &'static str,
}
#[derive(Debug, Clone, Default)]
pub struct CompileReport {
    pub accepted: usize,
    pub rejected: Vec<RejectedRule>,
}
#[derive(Default, Debug)]
struct Rules {
    exact: HashSet<String>,
    suffix: HashSet<String>,
}
impl Rules {
    fn matches(&self, host: &str) -> bool {
        if self.exact.contains(host) {
            return true;
        }
        let mut current = host;
        loop {
            if self.suffix.contains(current) {
                return true;
            }
            match current.split_once('.') {
                Some((_, rest)) => current = rest,
                None => return false,
            }
        }
    }
}
#[derive(Debug)]
pub struct CompiledRules {
    allow: Rules,
    blocks: [(Category, Rules); 4],
    report: CompileReport,
}
impl CompiledRules {
    pub fn compile(sources: &[Source<'_>]) -> Result<Self, Error> {
        if sources
            .iter()
            .try_fold(0usize, |n, s| n.checked_add(s.text.len()))
            .is_none_or(|n| n > MAX_SOURCE_BYTES)
        {
            return Err(Error("source size limit exceeded"));
        }
        let mut compiled = Self {
            allow: Rules::default(),
            blocks: [
                Category::Malware,
                Category::Trackers,
                Category::Ads,
                Category::Annoyances,
            ]
            .map(|c| (c, Rules::default())),
            report: CompileReport::default(),
        };
        let mut lines = 0;
        for (source_index, source) in sources.iter().enumerate() {
            for (line_index, raw) in source.text.lines().enumerate() {
                lines += 1;
                if lines > MAX_RULES {
                    return Err(Error("rule line limit exceeded"));
                }
                let line = raw.trim();
                if line.is_empty() || line.starts_with('!') || line.starts_with('#') {
                    continue;
                }
                let parsed = parse_rule(line);
                match parsed {
                    Ok((allow, suffix, host)) => {
                        let rules = if allow {
                            &mut compiled.allow
                        } else {
                            &mut compiled
                                .blocks
                                .iter_mut()
                                .find(|(c, _)| *c == source.category)
                                .expect("fixed category set")
                                .1
                        };
                        if suffix {
                            rules.suffix.insert(host);
                        } else {
                            rules.exact.insert(host);
                        }
                        compiled.report.accepted += 1;
                    }
                    Err(error) => compiled.report.rejected.push(RejectedRule {
                        source: source_index,
                        line: line_index + 1,
                        reason: error.0,
                    }),
                }
            }
        }
        Ok(compiled)
    }
    pub fn report(&self) -> &CompileReport {
        &self.report
    }
    fn decide(&self, host: &str, policy: Policy) -> Decision {
        if self.allow.matches(host) {
            return Decision::Allow;
        }
        for (category, rules) in &self.blocks {
            if policy.enabled(*category) && rules.matches(host) {
                return Decision::Block(*category);
            }
        }
        Decision::Allow
    }
}
fn parse_rule(line: &str) -> Result<(bool, bool, String), Error> {
    if line.len() > MAX_LINE_BYTES {
        return Err(Error("line size limit exceeded"));
    }
    if line.contains("##") || line.contains("#@#") {
        return Err(Error("cosmetic rules unsupported"));
    }
    if line.contains('$') {
        return Err(Error("rule modifiers unsupported"));
    }
    if line.starts_with('/') {
        return Err(Error("regular expressions unsupported"));
    }
    let (allow, suffix, domain) =
        if let Some(h) = line.strip_prefix("@@||").and_then(|s| s.strip_suffix('^')) {
            (true, true, h)
        } else if let Some(h) = line.strip_prefix("||").and_then(|s| s.strip_suffix('^')) {
            (false, true, h)
        } else if let Some(h) = line.strip_prefix("exact:") {
            (false, false, h)
        } else if let Some(h) = line.strip_prefix("allow:") {
            (true, false, h)
        } else {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() == 2 && ["0.0.0.0", "127.0.0.1", "::", "::1"].contains(&fields[0]) {
                (false, false, fields[1])
            } else {
                return Err(Error("unsupported rule syntax"));
            }
        };
    Ok((allow, suffix, canonical_host(domain)?))
}
pub fn canonical_host(host: &str) -> Result<String, Error> {
    if host.len() > 1024 || host.is_empty() || host.trim() != host {
        return Err(Error("invalid hostname"));
    }
    let host = host.strip_suffix('.').unwrap_or(host);
    let ascii = idna::domain_to_ascii_strict(host)
        .map_err(|_| Error("invalid IDNA hostname"))?
        .to_ascii_lowercase();
    if ascii.len() > 253 || !ascii.contains('.') || ascii.parse::<IpAddr>().is_ok() {
        return Err(Error(
            "domain must contain at least two labels and cannot be IP",
        ));
    }
    for label in ascii.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(Error("invalid domain label"));
        }
    }
    Ok(ascii)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Block(Category),
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counters {
    pub allowed: u64,
    pub blocked: u64,
    pub ads: u64,
    pub trackers: u64,
    pub malware: u64,
    pub annoyances: u64,
}
pub struct Engine {
    rules: CompiledRules,
    policy: Policy,
    counts: [AtomicU64; 5],
}
impl Engine {
    pub fn new(rules: CompiledRules, policy: Policy) -> Self {
        Self {
            rules,
            policy,
            counts: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
    pub fn check(&self, host: &str) -> Result<Decision, Error> {
        let host = canonical_host(host)?;
        let decision = self.rules.decide(&host, self.policy);
        let index = match decision {
            Decision::Allow => 0,
            Decision::Block(Category::Ads) => 1,
            Decision::Block(Category::Trackers) => 2,
            Decision::Block(Category::Malware) => 3,
            Decision::Block(Category::Annoyances) => 4,
        };
        // Saturation prevents a long-running service from wrapping counters.
        let _ = self.counts[index].try_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            Some(n.saturating_add(1))
        });
        Ok(decision)
    }
    /// Independent atomic snapshots: aggregate may advance during concurrent checks.
    pub fn counters(&self) -> Counters {
        let [allowed, ads, trackers, malware, annoyances] =
            self.counts.each_ref().map(|n| n.load(Ordering::Relaxed));
        Counters {
            allowed,
            blocked: ads
                .saturating_add(trackers)
                .saturating_add(malware)
                .saturating_add(annoyances),
            ads,
            trackers,
            malware,
            annoyances,
        }
    }
}

/// Opt-in local UDP harness; never changes system DNS. Caller owns the upstream.
pub struct LoopbackDns {
    socket: tokio::net::UdpSocket,
    engine: std::sync::Arc<Engine>,
    upstream_timeout: std::time::Duration,
}
impl LoopbackDns {
    pub async fn bind(
        engine: std::sync::Arc<Engine>,
        upstream_timeout: std::time::Duration,
    ) -> std::io::Result<Self> {
        if upstream_timeout.is_zero() || upstream_timeout > std::time::Duration::from_secs(5) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "upstream timeout must be in (0, 5s]",
            ));
        }
        Ok(Self {
            socket: tokio::net::UdpSocket::bind("127.0.0.1:0").await?,
            engine,
            upstream_timeout,
        })
    }
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.socket.local_addr()
    }
    /// Handles at most one datagram, sequentially. Cancellation drops the pending request.
    /// Async upstream must yield; a blocking callback cannot be preempted by Tokio.
    pub async fn serve_once<F, Fut>(&self, upstream: F) -> std::io::Result<()>
    where
        F: FnOnce(Vec<u8>) -> Fut,
        Fut: std::future::Future<Output = std::io::Result<Vec<u8>>>,
    {
        use hickory_proto::op::{Message, MessageType, OpCode, ResponseCode};
        const LIMIT: usize = 4096;
        let mut bytes = [0u8; LIMIT + 1];
        let (len, peer) = self.socket.recv_from(&mut bytes).await?;
        if len > LIMIT {
            return Ok(());
        }
        let Ok(request) = Message::from_vec(&bytes[..len]) else {
            return Ok(());
        };
        // Never forward malformed/multi-question or response packets.
        if request.message_type() != MessageType::Query
            || request.op_code() != OpCode::Query
            || request.queries().len() != 1
            || !request.answers().is_empty()
            || !request.name_servers().is_empty()
        {
            return Ok(());
        }
        let mut response = Message::new();
        response
            .set_id(request.id())
            .set_message_type(MessageType::Response)
            .set_op_code(request.op_code())
            .set_recursion_desired(request.recursion_desired())
            .add_queries(request.queries().iter().cloned());
        let name = request.queries()[0].name().to_utf8();
        match self.engine.check(&name) {
            Err(_) => {
                response.set_response_code(ResponseCode::Refused);
            }
            Ok(Decision::Block(_)) => {
                response.set_response_code(ResponseCode::NXDomain);
            }
            Ok(Decision::Allow) => {
                let result =
                    tokio::time::timeout(self.upstream_timeout, upstream(bytes[..len].to_vec()))
                        .await;
                if let Ok(Ok(answer)) = result
                    && answer.len() <= LIMIT
                    && let Ok(parsed) = Message::from_vec(&answer)
                    && parsed.id() == request.id()
                    && parsed.message_type() == MessageType::Response
                    && parsed.op_code() == request.op_code()
                    && parsed.queries() == request.queries()
                {
                    self.socket.send_to(&answer, peer).await?;
                    return Ok(());
                }
                response.set_response_code(ResponseCode::ServFail);
            }
        }
        let answer = response
            .to_vec()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.socket.send_to(&answer, peer).await?;
        Ok(())
    }
}
