use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LeaseClaims {
    pub device_key: String,
    pub node_id: String,
    pub epoch: u64,
    pub issued_at: i64,
    pub expires_at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedLease {
    pub claims: LeaseClaims,
    pub signature: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthMessage {
    pub lease: SignedLease,
    pub proof: String,
}
#[derive(Debug, thiserror::Error)]
#[error("admission denied")]
pub struct AdmissionError;
fn validate_claims(c: &LeaseClaims) -> Result<(), AdmissionError> {
    if c.node_id.is_empty()
        || c.node_id.len() > 128
        || c.epoch == 0
        || c.issued_at < 0
        || c.expires_at
            .checked_sub(c.issued_at)
            .is_none_or(|t| !(1..=900).contains(&t))
    {
        return Err(AdmissionError);
    }
    public_key(&c.device_key)?;
    Ok(())
}
fn public_key(s: &str) -> Result<VerifyingKey, AdmissionError> {
    let b: [u8; 32] = hex::decode(s)
        .map_err(|_| AdmissionError)?
        .try_into()
        .map_err(|_| AdmissionError)?;
    VerifyingKey::from_bytes(&b).map_err(|_| AdmissionError)
}
fn signature(s: &str) -> Result<Signature, AdmissionError> {
    let b: [u8; 64] = hex::decode(s)
        .map_err(|_| AdmissionError)?
        .try_into()
        .map_err(|_| AdmissionError)?;
    Ok(Signature::from_bytes(&b))
}
fn lease_bytes(c: &LeaseClaims) -> Result<Vec<u8>, AdmissionError> {
    let mut b = b"tavra-link-lease/0\0".to_vec();
    b.extend(serde_json::to_vec(c).map_err(|_| AdmissionError)?);
    Ok(b)
}
fn proof_bytes(
    l: &SignedLease,
    binding: &[u8; 32],
    nonce: &[u8; 32],
) -> Result<Vec<u8>, AdmissionError> {
    let mut b = b"tavra-link-auth/0\0".to_vec();
    b.extend(binding);
    b.extend(nonce);
    b.extend(Sha256::digest(
        serde_json::to_vec(l).map_err(|_| AdmissionError)?,
    ));
    Ok(b)
}
pub fn issue_lease(
    authority: &SigningKey,
    claims: LeaseClaims,
) -> Result<SignedLease, AdmissionError> {
    validate_claims(&claims)?;
    let signature = hex::encode(authority.sign(&lease_bytes(&claims)?).to_bytes());
    Ok(SignedLease { claims, signature })
}
pub fn make_auth(
    lease: SignedLease,
    device: &SigningKey,
    binding: &[u8; 32],
    nonce: &[u8; 32],
) -> Result<AuthMessage, AdmissionError> {
    validate_claims(&lease.claims)?;
    if public_key(&lease.claims.device_key)? != device.verifying_key() {
        return Err(AdmissionError);
    }
    let proof = hex::encode(
        device
            .sign(&proof_bytes(&lease, binding, nonce)?)
            .to_bytes(),
    );
    Ok(AuthMessage { lease, proof })
}
pub fn verify_auth(
    auth: &AuthMessage,
    authority: &VerifyingKey,
    binding: &[u8; 32],
    nonce: &[u8; 32],
    node: &str,
    epoch: u64,
    now: i64,
) -> Result<LeaseClaims, AdmissionError> {
    let c = &auth.lease.claims;
    validate_claims(c)?;
    if c.node_id != node || c.epoch != epoch || now < c.issued_at || now >= c.expires_at {
        return Err(AdmissionError);
    }
    authority
        .verify_strict(&lease_bytes(c)?, &signature(&auth.lease.signature)?)
        .map_err(|_| AdmissionError)?;
    public_key(&c.device_key)?
        .verify_strict(
            &proof_bytes(&auth.lease, binding, nonce)?,
            &signature(&auth.proof)?,
        )
        .map_err(|_| AdmissionError)?;
    Ok(c.clone())
}
