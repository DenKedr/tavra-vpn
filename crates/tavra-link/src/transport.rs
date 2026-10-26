use crate::{
    admission::{AuthMessage, SignedLease, make_auth, verify_auth},
    codec::Frame,
};
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::time::{Duration, Instant};
#[derive(Debug, thiserror::Error)]
#[error("TAVRA Link transport failed")]
pub struct LinkError;
pub struct Admitted {
    connection: quinn::Connection,
    deadline: Instant,
}
impl Admitted {
    pub fn send_packet(&self, packet: &[u8]) -> Result<(), LinkError> {
        self.forwarding_allowance()?;
        let data = Frame::Packet(packet.to_vec())
            .encode()
            .map_err(|_| LinkError)?;
        if self
            .connection
            .max_datagram_size()
            .is_none_or(|n| data.len() > n)
        {
            return Err(LinkError);
        }
        self.connection
            .send_datagram(data.into())
            .map_err(|_| LinkError)
    }
    pub async fn receive_packet(&self) -> Result<Vec<u8>, LinkError> {
        let bytes = match tokio::time::timeout(
            self.forwarding_allowance()?,
            self.connection.read_datagram(),
        )
        .await
        {
            Ok(Ok(bytes)) => bytes,
            _ => {
                self.close();
                return Err(LinkError);
            }
        };
        self.forwarding_allowance()?;
        match Frame::decode(&bytes).map_err(|_| LinkError)? {
            Frame::Packet(data) => Ok(data),
            _ => Err(LinkError),
        }
    }
    /// Remaining monotonic lifetime of this authenticated finite lease.
    /// Call immediately before each forwarding side effect, including after waiting
    /// for device readiness. Expiration closes QUIC and permanently denies forwarding.
    pub fn forwarding_allowance(&self) -> Result<Duration, LinkError> {
        let d = self.deadline.saturating_duration_since(Instant::now());
        if d.is_zero() {
            self.close();
            Err(LinkError)
        } else {
            Ok(d)
        }
    }
    pub fn close(&self) {
        self.connection.close(0u32.into(), b"closed");
    }
}
fn binding(c: &quinn::Connection) -> Result<[u8; 32], LinkError> {
    let mut b = [0; 32];
    c.export_keying_material(&mut b, b"tavra-link-admission/0", b"")
        .map_err(|_| LinkError)?;
    Ok(b)
}
async fn read_control(recv: &mut quinn::RecvStream) -> Result<Frame, LinkError> {
    let mut header = [0; 8];
    recv.read_exact(&mut header).await.map_err(|_| LinkError)?;
    let size = u32::from_be_bytes(header[4..8].try_into().map_err(|_| LinkError)?) as usize;
    if size > 4096 {
        return Err(LinkError);
    }
    let mut bytes = header.to_vec();
    bytes.resize(8 + size, 0);
    recv.read_exact(&mut bytes[8..])
        .await
        .map_err(|_| LinkError)?;
    Frame::decode(&bytes).map_err(|_| LinkError)
}
async fn write_control(send: &mut quinn::SendStream, frame: Frame) -> Result<(), LinkError> {
    send.write_all(&frame.encode().map_err(|_| LinkError)?)
        .await
        .map_err(|_| LinkError)
}
fn lease_deadline(started: Instant, expires: i64, now: i64) -> Result<Instant, LinkError> {
    let ttl = expires
        .checked_sub(now)
        .filter(|v| (1..=900).contains(v))
        .ok_or(LinkError)?;
    let deadline = started
        .checked_add(Duration::from_secs(ttl as u64))
        .ok_or(LinkError)?;
    if deadline <= Instant::now() {
        return Err(LinkError);
    }
    Ok(deadline)
}
fn admitted(
    c: quinn::Connection,
    expires: i64,
    now: i64,
    started: Instant,
) -> Result<Admitted, LinkError> {
    Ok(Admitted {
        connection: c,
        deadline: lease_deadline(started, expires, now)?,
    })
}
pub async fn accept(
    connection: quinn::Connection,
    authority: &VerifyingKey,
    node: &str,
    epoch: u64,
    now: i64,
) -> Result<Admitted, LinkError> {
    let started = Instant::now();
    let work = async {
        let (mut send, mut recv) = connection.open_bi().await.map_err(|_| LinkError)?;
        let mut nonce = [0; 32];
        getrandom::fill(&mut nonce).map_err(|_| LinkError)?;
        write_control(&mut send, Frame::Hello(nonce.to_vec())).await?;
        let Frame::Auth(bytes) = read_control(&mut recv).await? else {
            return Err(LinkError);
        };
        let auth: AuthMessage = serde_json::from_slice(&bytes).map_err(|_| LinkError)?;
        let clock = now
            .checked_add(started.elapsed().as_secs() as i64)
            .ok_or(LinkError)?;
        let claims = verify_auth(
            &auth,
            authority,
            &binding(&connection)?,
            &nonce,
            node,
            epoch,
            clock,
        )
        .map_err(|_| LinkError)?;
        let config = serde_json::to_vec(&claims).map_err(|_| LinkError)?;
        write_control(&mut send, Frame::Config(config)).await?;
        send.finish().map_err(|_| LinkError)?;
        admitted(connection.clone(), claims.expires_at, now, started)
    };
    match tokio::time::timeout(Duration::from_secs(5), work).await {
        Ok(Ok(a)) => Ok(a),
        _ => {
            connection.close(1u32.into(), b"admission denied");
            Err(LinkError)
        }
    }
}
pub async fn connect(
    connection: quinn::Connection,
    lease: SignedLease,
    device: &SigningKey,
    now: i64,
) -> Result<Admitted, LinkError> {
    let expected = lease.claims.clone();
    let started = Instant::now();
    let work = async {
        let (mut send, mut recv) = connection.accept_bi().await.map_err(|_| LinkError)?;
        let Frame::Hello(bytes) = read_control(&mut recv).await? else {
            return Err(LinkError);
        };
        let nonce: [u8; 32] = bytes.try_into().map_err(|_| LinkError)?;
        let auth =
            make_auth(lease, device, &binding(&connection)?, &nonce).map_err(|_| LinkError)?;
        write_control(
            &mut send,
            Frame::Auth(serde_json::to_vec(&auth).map_err(|_| LinkError)?),
        )
        .await?;
        send.finish().map_err(|_| LinkError)?;
        let Frame::Config(bytes) = read_control(&mut recv).await? else {
            return Err(LinkError);
        };
        let actual: crate::admission::LeaseClaims =
            serde_json::from_slice(&bytes).map_err(|_| LinkError)?;
        if actual != expected {
            return Err(LinkError);
        }
        admitted(connection.clone(), actual.expires_at, now, started)
    };
    match tokio::time::timeout(Duration::from_secs(5), work).await {
        Ok(Ok(a)) => Ok(a),
        _ => {
            connection.close(1u32.into(), b"admission denied");
            Err(LinkError)
        }
    }
}

#[cfg(test)]
mod expiry_tests {
    use super::*;
    #[test]
    fn handshake_does_not_restart_lease_deadline() {
        let started = Instant::now() - Duration::from_secs(2);
        assert!(lease_deadline(started, 101, 100).is_err());
        let started = Instant::now();
        assert_eq!(
            lease_deadline(started, 101, 100).unwrap(),
            started + Duration::from_secs(1)
        );
    }
}
