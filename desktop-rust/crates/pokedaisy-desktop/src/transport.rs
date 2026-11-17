//! Bounded, session-scoped building blocks for the future Player–Companion link.
//! The launcher remains the owner of the transport; snapshots are latest-only
//! while user commands have a small reliable queue and an out-of-band Stop flag.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};

pub const MAX_SNAPSHOT_BYTES: usize = 256 * 1024;
pub const MAX_FRAME_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransportFrame {
    pub protocol_version: u32,
    pub session_id: String,
    pub epoch: u64,
    pub nonce: String,
    pub sequence: u64,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    Io,
    Empty,
    TooLarge,
    InvalidJson,
    WrongVersion,
    WrongSession,
    StaleEpoch,
    InvalidNonce,
    Replay,
}

/// Read one little-endian length-prefixed JSON frame before allocating its body.
pub fn read_frame(reader: &mut impl Read) -> Result<TransportFrame, FrameError> {
    let mut prefix = [0u8; 4];
    reader.read_exact(&mut prefix).map_err(|_| FrameError::Io)?;
    let size = u32::from_le_bytes(prefix) as usize;
    if size == 0 {
        return Err(FrameError::Empty);
    }
    if size > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge);
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes).map_err(|_| FrameError::Io)?;
    serde_json::from_slice(&bytes).map_err(|_| FrameError::InvalidJson)
}

pub fn write_frame(writer: &mut impl Write, frame: &TransportFrame) -> Result<(), FrameError> {
    let bytes = serde_json::to_vec(frame).map_err(|_| FrameError::InvalidJson)?;
    if bytes.is_empty() {
        return Err(FrameError::Empty);
    }
    if bytes.len() > MAX_FRAME_BYTES || bytes.len() > u32::MAX as usize {
        return Err(FrameError::TooLarge);
    }
    writer
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .and_then(|_| writer.write_all(&bytes))
        .map_err(|_| FrameError::Io)
}

/// Check a peer frame and advance the accepted monotonic sequence only on success.
pub fn validate_frame(
    frame: &TransportFrame,
    session_id: &str,
    epoch: u64,
    nonce: &str,
    last_sequence: &mut u64,
) -> Result<(), FrameError> {
    if frame.protocol_version != crate::domain::CONTRACT_VERSION {
        return Err(FrameError::WrongVersion);
    }
    if frame.session_id != session_id {
        return Err(FrameError::WrongSession);
    }
    if frame.epoch != epoch {
        return Err(FrameError::StaleEpoch);
    }
    if !constant_time_eq(frame.nonce.as_bytes(), nonce.as_bytes()) {
        return Err(FrameError::InvalidNonce);
    }
    if frame.sequence <= *last_sequence {
        return Err(FrameError::Replay);
    }
    *last_sequence = frame.sequence;
    Ok(())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub protocol_version: u32,
    pub session_id: String,
    pub epoch: u64,
    pub revision: u64,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotError {
    WrongVersion,
    InvalidPayload,
    WrongSession,
    StaleEpoch,
    StaleRevision,
    TooLarge,
    Poisoned,
}

#[derive(Debug)]
struct SnapshotState {
    session_id: String,
    epoch: u64,
    latest: Option<SessionSnapshot>,
}

/// One bounded slot: a slow Companion can skip old snapshots without growing memory.
#[derive(Debug, Clone)]
pub struct LatestSnapshot {
    state: Arc<Mutex<SnapshotState>>,
}

impl LatestSnapshot {
    pub fn new(session_id: impl Into<String>, epoch: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(SnapshotState {
                session_id: session_id.into(),
                epoch,
                latest: None,
            })),
        }
    }

    pub fn publish(&self, snapshot: SessionSnapshot) -> Result<(), SnapshotError> {
        if serde_json::to_vec(&snapshot).map_or(true, |bytes| bytes.len() > MAX_SNAPSHOT_BYTES) {
            return Err(SnapshotError::TooLarge);
        }
        let mut state = self.state.lock().map_err(|_| SnapshotError::Poisoned)?;
        if snapshot.protocol_version != crate::domain::CONTRACT_VERSION {
            return Err(SnapshotError::WrongVersion);
        }
        if !snapshot.payload.is_object() {
            return Err(SnapshotError::InvalidPayload);
        }
        if snapshot.session_id != state.session_id {
            return Err(SnapshotError::WrongSession);
        }
        if snapshot.epoch != state.epoch {
            return Err(SnapshotError::StaleEpoch);
        }
        if state
            .latest
            .as_ref()
            .is_some_and(|latest| snapshot.revision <= latest.revision)
        {
            return Err(SnapshotError::StaleRevision);
        }
        state.latest = Some(snapshot);
        Ok(())
    }

    pub fn take_latest(&self) -> Result<Option<SessionSnapshot>, SnapshotError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| SnapshotError::Poisoned)?
            .latest
            .take())
    }

    /// Invalidate in-flight work after load, rewind, restore, or session change.
    pub fn reset(&self, session_id: impl Into<String>, epoch: u64) -> Result<(), SnapshotError> {
        let mut state = self.state.lock().map_err(|_| SnapshotError::Poisoned)?;
        state.session_id = session_id.into();
        state.epoch = epoch;
        state.latest = None;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerCommand {
    Pause(bool),
    SaveState(u64),
    LoadState(u64),
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    Full,
    Disconnected,
}

/// Commands never wait behind an unbounded backlog; Stop bypasses that backlog.
pub struct PlayerCommandQueue {
    sender: SyncSender<PlayerCommand>,
    receiver: Receiver<PlayerCommand>,
    stop_requested: Arc<AtomicBool>,
}

impl PlayerCommandQueue {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::sync_channel(capacity.max(1));
        Self {
            sender,
            receiver,
            stop_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn sender(&self) -> PlayerCommandSender {
        PlayerCommandSender {
            sender: self.sender.clone(),
            stop_requested: Arc::clone(&self.stop_requested),
        }
    }

    pub fn try_next(&self) -> Result<Option<PlayerCommand>, QueueError> {
        if self.stop_requested.swap(false, Ordering::AcqRel) {
            return Ok(Some(PlayerCommand::Stop));
        }
        match self.receiver.try_recv() {
            Ok(command) => Ok(Some(command)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(QueueError::Disconnected),
        }
    }
}

#[derive(Clone)]
pub struct PlayerCommandSender {
    sender: SyncSender<PlayerCommand>,
    stop_requested: Arc<AtomicBool>,
}

impl PlayerCommandSender {
    pub fn send(&self, command: PlayerCommand) -> Result<(), QueueError> {
        if command == PlayerCommand::Stop {
            self.stop_requested.store(true, Ordering::Release);
            return Ok(());
        }
        self.sender.try_send(command).map_err(|error| match error {
            TrySendError::Full(_) => QueueError::Full,
            TrySendError::Disconnected(_) => QueueError::Disconnected,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FrameError, LatestSnapshot, PlayerCommand, PlayerCommandQueue, QueueError, SessionSnapshot,
        SnapshotError, TransportFrame, read_frame, validate_frame, write_frame,
    };
    use serde_json::json;

    #[test]
    fn shared_session_snapshot_fixture_is_supported_v1() {
        let value: SessionSnapshot = serde_json::from_str(include_str!(
            "../../../../contracts/vyntra/v1/session-snapshot-v1.json"
        ))
        .unwrap();
        assert_eq!(value.protocol_version, crate::domain::CONTRACT_VERSION);
        assert_eq!(value.session_id, "fixture-session");
        assert_eq!(value.epoch, 3);
        assert_eq!(value.revision, 8);
        assert_eq!(value.payload["game_code"], "BPRE");
    }

    #[test]
    fn shared_session_snapshot_fixture_rejects_incompatible_major() {
        let value: SessionSnapshot = serde_json::from_str(include_str!(
            "../../../../contracts/vyntra/v1/session-snapshot-v2-incompatible.json"
        ))
        .unwrap();
        let slot = LatestSnapshot::new("fixture-session", 3);
        assert_eq!(slot.publish(value), Err(SnapshotError::WrongVersion));
    }

    #[test]
    fn snapshot_payload_must_be_an_object() {
        let mut value = snapshot("fixture-session", 3, 8);
        value.payload = json!("not-an-object");
        let slot = LatestSnapshot::new("fixture-session", 3);
        assert_eq!(slot.publish(value), Err(SnapshotError::InvalidPayload));
    }

    fn snapshot(session_id: &str, epoch: u64, revision: u64) -> SessionSnapshot {
        SessionSnapshot {
            protocol_version: crate::domain::CONTRACT_VERSION,
            session_id: session_id.into(),
            epoch,
            revision,
            payload: json!({"value": revision}),
        }
    }

    #[test]
    fn slow_reader_observes_only_the_latest_snapshot() {
        let slot = LatestSnapshot::new("session-a", 4);
        slot.publish(snapshot("session-a", 4, 1)).unwrap();
        slot.publish(snapshot("session-a", 4, 2)).unwrap();
        assert_eq!(slot.take_latest().unwrap().unwrap().revision, 2);
        assert!(slot.take_latest().unwrap().is_none());
    }

    #[test]
    fn snapshots_reject_wrong_session_old_epoch_and_old_revision() {
        let slot = LatestSnapshot::new("session-a", 4);
        assert_eq!(
            slot.publish(snapshot("session-b", 4, 1)),
            Err(SnapshotError::WrongSession)
        );
        assert_eq!(
            slot.publish(snapshot("session-a", 3, 1)),
            Err(SnapshotError::StaleEpoch)
        );
        slot.publish(snapshot("session-a", 4, 2)).unwrap();
        assert_eq!(
            slot.publish(snapshot("session-a", 4, 2)),
            Err(SnapshotError::StaleRevision)
        );
        slot.reset("session-a", 5).unwrap();
        assert_eq!(
            slot.publish(snapshot("session-a", 4, 3)),
            Err(SnapshotError::StaleEpoch)
        );
        assert!(slot.take_latest().unwrap().is_none());
    }

    #[test]
    fn oversized_snapshot_is_rejected_before_it_enters_the_slot() {
        let slot = LatestSnapshot::new("session-a", 0);
        let mut value = snapshot("session-a", 0, 1);
        value.payload = json!({"data": "x".repeat(300 * 1024)});
        assert_eq!(slot.publish(value), Err(SnapshotError::TooLarge));
        assert!(slot.take_latest().unwrap().is_none());
    }

    #[test]
    fn command_queue_is_bounded_and_stop_bypasses_pending_commands() {
        let queue = PlayerCommandQueue::new(1);
        let sender = queue.sender();
        sender.send(PlayerCommand::SaveState(1)).unwrap();
        assert_eq!(
            sender.send(PlayerCommand::LoadState(2)),
            Err(QueueError::Full)
        );
        sender.send(PlayerCommand::Stop).unwrap();
        assert_eq!(queue.try_next().unwrap(), Some(PlayerCommand::Stop));
        assert_eq!(queue.try_next().unwrap(), Some(PlayerCommand::SaveState(1)));
        assert_eq!(queue.try_next().unwrap(), None);
    }

    #[test]
    fn frames_roundtrip_and_validate_session_nonce_epoch_and_replay() {
        let frame = TransportFrame {
            protocol_version: crate::domain::CONTRACT_VERSION,
            session_id: "session-a".into(),
            epoch: 9,
            nonce: "a1b2c3".into(),
            sequence: 1,
            payload: json!({"command": "ping"}),
        };
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &frame).unwrap();
        let mut stream = std::io::Cursor::new(bytes);
        let decoded = read_frame(&mut stream).unwrap();
        assert_eq!(decoded, frame);
        let mut last_sequence = 0;
        validate_frame(&decoded, "session-a", 9, "a1b2c3", &mut last_sequence).unwrap();
        assert_eq!(last_sequence, 1);
        assert_eq!(
            validate_frame(&decoded, "session-a", 9, "wrong", &mut last_sequence),
            Err(FrameError::InvalidNonce)
        );
        let mut replay_sequence = 1;
        assert_eq!(
            validate_frame(&decoded, "session-a", 9, "a1b2c3", &mut replay_sequence),
            Err(FrameError::Replay)
        );
        assert_eq!(
            validate_frame(&decoded, "other", 9, "a1b2c3", &mut 0),
            Err(FrameError::WrongSession)
        );
        assert_eq!(
            validate_frame(&decoded, "session-a", 8, "a1b2c3", &mut 0),
            Err(FrameError::StaleEpoch)
        );
    }

    #[test]
    fn frame_reader_rejects_oversized_length_before_reading_body() {
        let bytes = ((super::MAX_FRAME_BYTES as u32) + 1).to_le_bytes();
        let mut stream = std::io::Cursor::new(bytes);
        assert_eq!(read_frame(&mut stream), Err(FrameError::TooLarge));
    }
}
