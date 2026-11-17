//! Bounded, session-scoped building blocks for the future Player–Companion link.
//! The launcher remains the owner of the transport; snapshots are latest-only
//! while user commands have a small reliable queue and an out-of-band Stop flag.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};

pub const MAX_SNAPSHOT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub epoch: u64,
    pub revision: u64,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotError {
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
    SaveState,
    LoadState,
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
        LatestSnapshot, PlayerCommand, PlayerCommandQueue, QueueError, SessionSnapshot,
        SnapshotError,
    };
    use serde_json::json;

    fn snapshot(session_id: &str, epoch: u64, revision: u64) -> SessionSnapshot {
        SessionSnapshot {
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
        sender.send(PlayerCommand::SaveState).unwrap();
        assert_eq!(sender.send(PlayerCommand::LoadState), Err(QueueError::Full));
        sender.send(PlayerCommand::Stop).unwrap();
        assert_eq!(queue.try_next().unwrap(), Some(PlayerCommand::Stop));
        assert_eq!(queue.try_next().unwrap(), Some(PlayerCommand::SaveState));
        assert_eq!(queue.try_next().unwrap(), None);
    }
}
