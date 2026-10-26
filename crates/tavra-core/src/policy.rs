use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "PolicyWire")]
pub struct SubscriptionPolicy {
    pub expires_at: i64,
    pub device_limit: i64,
    pub blocked: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("invalid policy or time")]
    Invalid,
    #[error("subscription blocked")]
    Blocked,
    #[error("subscription expired")]
    Expired,
    #[error("device limit reached")]
    DeviceLimit,
    #[error("lease must have finite positive duration")]
    InvalidLease,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "LeaseWire")]
pub struct Lease {
    issued_at: i64,
    expires_at: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LeaseWire {
    issued_at: i64,
    expires_at: i64,
}
impl TryFrom<LeaseWire> for Lease {
    type Error = PolicyError;
    fn try_from(v: LeaseWire) -> Result<Self, Self::Error> {
        Self::new(v.issued_at, v.expires_at)
    }
}
impl Lease {
    pub fn new(issued_at: i64, expires_at: i64) -> Result<Self, PolicyError> {
        if issued_at < 0
            || expires_at <= issued_at
            || expires_at
                .checked_sub(issued_at)
                .is_none_or(|ttl| ttl > DEFAULT_LEASE_TTL_S)
        {
            return Err(PolicyError::InvalidLease);
        }
        Ok(Self {
            issued_at,
            expires_at,
        })
    }
    pub fn validate_at(&self, now: i64) -> Result<(), PolicyError> {
        if now < self.issued_at {
            return Err(PolicyError::Invalid);
        }
        if now >= self.expires_at {
            return Err(PolicyError::Expired);
        }
        Ok(())
    }
    pub fn expires_at(&self) -> i64 {
        self.expires_at
    }
}
impl SubscriptionPolicy {
    pub fn validate_at(&self, now_unix_s: i64) -> Result<(), PolicyError> {
        if now_unix_s < 0 || self.expires_at < 0 || self.device_limit < 0 {
            return Err(PolicyError::Invalid);
        }
        if self.blocked {
            return Err(PolicyError::Blocked);
        }
        if self.expires_at != 0 && now_unix_s >= self.expires_at {
            return Err(PolicyError::Expired);
        }
        Ok(())
    }
    /// Admission precheck only; the control plane must serialize count-and-insert.
    pub fn can_register_device(&self, registered: u64) -> Result<(), PolicyError> {
        if self.expires_at < 0 || self.device_limit < 0 {
            return Err(PolicyError::Invalid);
        }
        if self.blocked {
            return Err(PolicyError::Blocked);
        }
        if self.device_limit != 0 && registered >= self.device_limit as u64 {
            return Err(PolicyError::DeviceLimit);
        }
        Ok(())
    }
    pub fn issue_lease(&self, now: i64, ttl: i64) -> Result<Lease, PolicyError> {
        self.validate_at(now)?;
        if ttl <= 0 || ttl > DEFAULT_LEASE_TTL_S {
            return Err(PolicyError::InvalidLease);
        }
        let end = now.checked_add(ttl).ok_or(PolicyError::InvalidLease)?;
        Lease::new(
            now,
            if self.expires_at == 0 {
                end
            } else {
                end.min(self.expires_at)
            },
        )
    }
}

/// Business unlimited values never disable lease revocation bounds.
pub const DEFAULT_LEASE_TTL_S: i64 = 900;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyWire {
    expires_at: i64,
    device_limit: i64,
    blocked: bool,
}
impl TryFrom<PolicyWire> for SubscriptionPolicy {
    type Error = PolicyError;
    fn try_from(v: PolicyWire) -> Result<Self, Self::Error> {
        if v.expires_at < 0 || v.device_limit < 0 {
            return Err(PolicyError::Invalid);
        }
        Ok(Self {
            expires_at: v.expires_at,
            device_limit: v.device_limit,
            blocked: v.blocked,
        })
    }
}
