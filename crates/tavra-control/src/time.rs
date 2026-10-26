//! Fail-closed authorization time, with a durable restart boundary and monotonic freeze detection.
use crate::{ApiError, AppState, Request, Response, State};
use axum::middleware::Next;
use std::time::Instant;

#[derive(Clone, Copy)]
pub struct VerifiedTime(pub i64, pub i64);
pub(crate) struct Anchor {
    pub wall: i64,
    pub instant: Instant,
}
impl Anchor {
    pub fn new(wall: i64) -> Self {
        Self {
            wall,
            instant: Instant::now(),
        }
    }
}
impl AppState {
    async fn verified_time(&self) -> Result<VerifiedTime, ApiError> {
        let mut tx = self.pool.begin().await?;
        let (high_water, generation): (i64, i64) = sqlx::query_as(
            "SELECT high_water,generation FROM security_clock WHERE singleton=true FOR UPDATE",
        )
        .fetch_one(&mut *tx)
        .await?;
        // Sample after the row lock: a queued request must not compare an old sample
        // against a newer accepted time from another request/process.
        let wall = self.now();
        let (monotonic_ok, floor) = {
            let mut anchor = self.time_anchor.lock().map_err(|_| ApiError::internal())?;
            let floor = anchor
                .wall
                .saturating_add(anchor.instant.elapsed().as_secs().min(i64::MAX as u64) as i64);
            let valid = wall >= floor && wall > 0 && wall < i64::MAX - 86400;
            if valid && wall > anchor.wall {
                *anchor = Anchor::new(wall);
            }
            (valid, floor)
        };
        if !monotonic_ok || wall < high_water {
            // Do not merely freeze grants at high_water: destroy outstanding grants.
            // The persistent admin key is retained so a corrected clock permits login.
            sqlx::query("UPDATE security_clock SET high_water=GREATEST(high_water,$1),generation=generation+1 WHERE singleton=true").bind(floor.max(0)).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM sessions")
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM challenges")
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Err(ApiError::clock_untrusted());
        }
        sqlx::query("UPDATE security_clock SET high_water=$1 WHERE singleton=true")
            .bind(wall)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(VerifiedTime(wall, generation))
    }
}
pub async fn require_trusted_time(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Health remains useful for diagnosing DB reachability while auth is fail-closed.
    if req.uri().path() != "/v1/health" {
        let now = state.verified_time().await?;
        req.extensions_mut().insert(now);
    }
    Ok(next.run(req).await)
}
