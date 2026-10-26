use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Preparing,
    Connecting,
    Connected,
    Disconnecting,
    FailedClosed,
}
/// The only connection messages accepted from an untrusted UI/IPC client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionRequest {
    Connect,
    Disconnect,
    DirectFallback,
}
/// Opaque, per-machine attempt identity. Never deserialize this from UI input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttemptId(u64);
/// In-process backend callbacks have no Deserialize implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendEvent {
    PrerequisitesVerified,
    TunnelConfirmed,
    TunnelLost,
    Failure,
    TunnelStopped,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionEvent {
    Request(ConnectionRequest),
    Backend {
        attempt: AttemptId,
        event: BackendEvent,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid, stale or forbidden connection transition")]
pub struct TransitionError;
#[derive(Default)]
pub struct ConnectionMachine {
    state: ConnectionState,
    generation: u64,
    attempt: Option<AttemptId>,
}
impl ConnectionMachine {
    /// Bind this ID to a newly created backend operation and capture it in every
    /// callback. A callback must never look up the machine's current ID on delivery.
    /// IPC exposes only ConnectionRequest; IDs are not authentication credentials.
    pub fn attempt(&self) -> Option<AttemptId> {
        self.attempt
    }
    pub fn transition(
        &mut self,
        event: ConnectionEvent,
    ) -> Result<ConnectionState, TransitionError> {
        use BackendEvent::*;
        use ConnectionRequest::*;
        use ConnectionState::*;
        let next = match event {
            ConnectionEvent::Request(Connect)
                if matches!(self.state, Disconnected | FailedClosed) =>
            {
                let generation = self.generation.checked_add(1).ok_or(TransitionError)?;
                self.generation = generation;
                self.attempt = Some(AttemptId(generation));
                Preparing
            }
            ConnectionEvent::Request(Disconnect)
                if matches!(
                    self.state,
                    Preparing | Connecting | Connected | FailedClosed
                ) =>
            {
                Disconnecting
            }
            ConnectionEvent::Request(_) => return Err(TransitionError),
            ConnectionEvent::Backend { attempt, event } => {
                if self.attempt != Some(attempt) {
                    return Err(TransitionError);
                }
                match (self.state, event) {
                    (Preparing, PrerequisitesVerified) => Connecting,
                    (Connecting, TunnelConfirmed) => Connected,
                    (Preparing | Connecting | Connected | Disconnecting, Failure | TunnelLost) => {
                        FailedClosed
                    }
                    (Disconnecting, TunnelStopped) => Disconnected,
                    _ => return Err(TransitionError),
                }
            }
        };
        self.state = next;
        if next == Disconnected {
            self.attempt = None;
        }
        Ok(next)
    }
    /// Fail-closed intent for the platform firewall adapter, not a firewall effect.
    pub fn traffic_blocked(&self) -> bool {
        self.state != ConnectionState::Connected
    }
    pub fn state(&self) -> ConnectionState {
        self.state
    }
}
