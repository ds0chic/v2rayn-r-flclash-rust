//! Shared IPC wire helpers for net-host and its client (T03).
//!
//! The message *types* live in `ipc_contract`; this module only adds the
//! server-to-client frame wrapper and a small length-prefixed framing scheme.
//! Every frame is `u32 little-endian length || JSON payload`, capped at
//! `IPC_MAX_MESSAGE_BYTES`.
//!
//! The request/response envelope is a strict subset of what net-host can send:
//! responses from `ipc_contract`, plus unsolicited control events. Because
//! `RuntimeDetail` (PID, ports, session) is carried inside the free-form event
//! payload, no change to the frozen `ipc_contract` types is required.

use domain::event::EventEnvelope;
use domain::{DomainError, RuntimeState};
use ipc_contract::ResponseEnvelope;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Frame length prefix size in bytes.
pub const LEN_PREFIX_BYTES: usize = 4;

/// Well-known named pipe the net-host listens on (Windows).
pub const NET_HOST_PIPE_NAME: &str = r"\\.\pipe\v2rayn-r-net-host";

/// One server-to-client frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum ServerFrame {
    /// Response to a specific request id.
    Response(ResponseEnvelope),
    /// Unsolicited control/telemetry event.
    Event(EventEnvelope),
}

/// Event kind carrying the current runtime detail (PID, ports, session).
pub const RUNTIME_DETAIL_EVENT: &str = "runtime_detail";

/// The concrete runtime facts the UI needs, transported as an event payload.
/// This is the only place PID/ports/session are exposed; `ipc_contract`
/// remains unchanged.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RuntimeDetail {
    pub state: RuntimeState,
    pub applied_revision: u64,
    pub pid: Option<u32>,
    pub created_at_ms: Option<i64>,
    pub ports: Vec<u16>,
    pub session_id: Option<String>,
    pub config_sha256: Option<String>,
    pub operation_id: Option<String>,
    pub error: Option<DomainError>,
}

/// Encode a length-prefixed frame.
pub fn encode_frame(value: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    let payload = serde_json::to_vec(value)?;
    let mut out = Vec::with_capacity(LEN_PREFIX_BYTES + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Decode one JSON payload into a typed value.
pub fn decode_payload<T: DeserializeOwned>(payload: &[u8]) -> Result<T, serde_json::Error> {
    serde_json::from_slice(payload)
}

/// Interpret a 4-byte little-endian length prefix.
pub fn frame_len(prefix: [u8; LEN_PREFIX_BYTES]) -> u32 {
    u32::from_le_bytes(prefix)
}

/// Whether a declared frame length is within the protocol cap.
pub fn frame_len_ok(len: u32) -> bool {
    ipc_contract::check_frame_size(len as usize).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::event::{EventEpoch, EventKind, EventSeq};
    use ipc_contract::IpcResult;
    use serde_json::json;

    #[test]
    fn response_frame_roundtrips() {
        let frame = ServerFrame::Response(ResponseEnvelope {
            request_id: "r1".into(),
            result: IpcResult::Stopped,
        });
        let bytes = encode_frame(&frame).unwrap();
        let len = frame_len(bytes[..4].try_into().unwrap());
        assert_eq!(len as usize, bytes.len() - 4);
        let decoded: ServerFrame = decode_payload(&bytes[4..]).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn event_frame_carries_runtime_detail() {
        let detail = RuntimeDetail {
            state: RuntimeState::Running,
            applied_revision: 3,
            pid: Some(4242),
            created_at_ms: Some(1_700_000_000_000),
            ports: vec![11808],
            session_id: Some("s1".into()),
            config_sha256: Some("ab".into()),
            operation_id: Some("op1".into()),
            error: None,
        };
        let env = EventEnvelope::new(
            EventEpoch(1),
            EventSeq(7),
            EventKind::Other(RUNTIME_DETAIL_EVENT.into()),
            serde_json::to_value(&detail).unwrap(),
        );
        let bytes = encode_frame(&ServerFrame::Event(env)).unwrap();
        let frame: ServerFrame = decode_payload(&bytes[4..]).unwrap();
        match frame {
            ServerFrame::Event(env) => {
                let back: RuntimeDetail = serde_json::from_value(env.payload).unwrap();
                assert_eq!(back, detail);
            }
            ServerFrame::Response(_) => panic!("expected event frame"),
        }
    }

    #[test]
    fn size_cap_is_enforced() {
        assert!(frame_len_ok(1024));
        assert!(!frame_len_ok(u32::MAX));
        let _ = json!({});
    }
}
