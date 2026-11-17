//! Exact-identity, bounded and read-only boundary for future runtime memory adapters.
//! The owner must provide an authorized MemoryReader; adapters never receive pointers.

use std::collections::HashMap;

pub const MAX_CAPTURE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameIdentity {
    pub system: String,
    pub rom_sha256: String,
    pub core_sha256: String,
    pub game_revision: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRange {
    pub name: String,
    pub offset: usize,
    pub length: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedSegment {
    pub name: String,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCapture {
    pub identity: GameIdentity,
    pub epoch: u64,
    pub revision: u64,
    pub segments: Vec<OwnedSegment>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureError {
    DuplicateIdentity,
    WrongIdentity,
    OutOfBounds,
    TooLarge,
    Truncated,
    WrongSession,
    StaleEpoch,
    StaleRevision,
    ReaderUnavailable,
}

/// Owner-scoped read capability. Implementations return copied bytes only.
pub trait MemoryReader {
    fn read(&mut self, offset: usize, length: usize) -> Result<Vec<u8>, CaptureError>;
}
pub struct LibretroSystemRamReader<'a> {
    core: &'a mut vyntra_libretro::LibretroCore,
}

impl<'a> LibretroSystemRamReader<'a> {
    pub fn new(core: &'a mut vyntra_libretro::LibretroCore) -> Self {
        Self { core }
    }
}

impl MemoryReader for LibretroSystemRamReader<'_> {
    fn read(&mut self, offset: usize, length: usize) -> Result<Vec<u8>, CaptureError> {
        self.core
            .read_system_ram_range(offset, length)
            .map_err(|error| match error {
                vyntra_libretro::CoreError::SystemRamRangeOutOfBounds { .. } => {
                    CaptureError::OutOfBounds
                }
                vyntra_libretro::CoreError::SystemRamTooLarge { .. } => CaptureError::TooLarge,
                _ => CaptureError::ReaderUnavailable,
            })
    }
}
pub trait MemoryAdapter {
    fn identity(&self) -> &GameIdentity;
    fn ranges(&self) -> &[MemoryRange];
}

pub fn capture(
    adapter: &dyn MemoryAdapter,
    reader: &mut dyn MemoryReader,
    epoch: u64,
    revision: u64,
) -> Result<MemoryCapture, CaptureError> {
    let total = adapter.ranges().iter().try_fold(0usize, |sum, range| {
        range
            .offset
            .checked_add(range.length)
            .ok_or(CaptureError::OutOfBounds)?;
        sum.checked_add(range.length).ok_or(CaptureError::TooLarge)
    })?;
    if total > MAX_CAPTURE_BYTES {
        return Err(CaptureError::TooLarge);
    }
    let mut segments = Vec::with_capacity(adapter.ranges().len());
    for range in adapter.ranges() {
        let bytes = reader.read(range.offset, range.length)?;
        if bytes.len() != range.length {
            return Err(CaptureError::Truncated);
        }
        segments.push(OwnedSegment {
            name: range.name.clone(),
            bytes,
        });
    }
    Ok(MemoryCapture {
        identity: adapter.identity().clone(),
        epoch,
        revision,
        segments,
    })
}
#[derive(Default)]
pub struct MemoryAdapterRegistry {
    adapters: HashMap<GameIdentity, Box<dyn MemoryAdapter>>,
}
impl MemoryAdapterRegistry {
    pub fn register(&mut self, adapter: Box<dyn MemoryAdapter>) -> Result<(), CaptureError> {
        let identity = adapter.identity().clone();
        if self.adapters.contains_key(&identity) {
            return Err(CaptureError::DuplicateIdentity);
        }
        self.adapters.insert(identity, adapter);
        Ok(())
    }
    pub fn resolve(&self, identity: &GameIdentity) -> Option<&dyn MemoryAdapter> {
        self.adapters.get(identity).map(Box::as_ref)
    }
}
pub struct CaptureEpochGate {
    session_id: String,
    identity: GameIdentity,
    epoch: u64,
    last_revision: u64,
}
impl CaptureEpochGate {
    pub fn new(session_id: impl Into<String>, identity: GameIdentity, epoch: u64) -> Self {
        Self {
            session_id: session_id.into(),
            identity,
            epoch,
            last_revision: 0,
        }
    }
    pub fn advance(&mut self, session_id: impl Into<String>, identity: GameIdentity, epoch: u64) {
        self.session_id = session_id.into();
        self.identity = identity;
        self.epoch = epoch;
        self.last_revision = 0;
    }
    pub fn accept(
        &mut self,
        session_id: &str,
        capture: &MemoryCapture,
    ) -> Result<(), CaptureError> {
        if session_id != self.session_id {
            return Err(CaptureError::WrongSession);
        }
        if capture.epoch != self.epoch {
            return Err(CaptureError::StaleEpoch);
        }
        if capture.identity != self.identity {
            return Err(CaptureError::WrongIdentity);
        }
        if capture.revision <= self.last_revision {
            return Err(CaptureError::StaleRevision);
        }
        self.last_revision = capture.revision;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::{
        CaptureError, GameIdentity, MemoryAdapter, MemoryAdapterRegistry, MemoryRange,
        MemoryReader, capture,
    };

    struct FixtureReader {
        bytes: Vec<u8>,
    }
    struct ShortReader;
    impl MemoryReader for ShortReader {
        fn read(&mut self, _offset: usize, _length: usize) -> Result<Vec<u8>, CaptureError> {
            Ok(vec![1])
        }
    }
    impl MemoryReader for FixtureReader {
        fn read(&mut self, offset: usize, length: usize) -> Result<Vec<u8>, CaptureError> {
            let end = offset
                .checked_add(length)
                .ok_or(CaptureError::OutOfBounds)?;
            self.bytes
                .get(offset..end)
                .map(|slice| slice.to_vec())
                .ok_or(CaptureError::OutOfBounds)
        }
    }

    struct FixtureAdapter {
        identity: GameIdentity,
        ranges: Vec<MemoryRange>,
    }
    impl MemoryAdapter for FixtureAdapter {
        fn identity(&self) -> &GameIdentity {
            &self.identity
        }
        fn ranges(&self) -> &[MemoryRange] {
            &self.ranges
        }
    }

    fn identity(rom: &str) -> GameIdentity {
        GameIdentity {
            system: "GBA".into(),
            rom_sha256: rom.into(),
            core_sha256: "core-hash".into(),
            game_revision: "retail-1.0".into(),
        }
    }

    #[test]
    fn adapter_resolution_requires_exact_rom_core_and_revision() {
        let wanted = identity("rom-a");
        let mut registry = MemoryAdapterRegistry::default();
        registry
            .register(Box::new(FixtureAdapter {
                identity: wanted.clone(),
                ranges: vec![],
            }))
            .unwrap();
        assert!(registry.resolve(&wanted).is_some());
        assert!(registry.resolve(&identity("rom-similar")).is_none());
        let mut other_core = wanted.clone();
        other_core.core_sha256 = "other-core".into();
        assert!(registry.resolve(&other_core).is_none());
    }

    #[test]
    fn capture_copies_only_bounded_requested_ranges_into_owned_buffers() {
        let id = identity("rom-a");
        let adapter = FixtureAdapter {
            identity: id.clone(),
            ranges: vec![MemoryRange {
                name: "party".into(),
                offset: 2,
                length: 3,
            }],
        };
        let captured = capture(
            &adapter,
            &mut FixtureReader {
                bytes: vec![0, 1, 2, 3, 4, 5],
            },
            4,
            9,
        )
        .unwrap();
        assert_eq!(captured.segments[0].bytes, vec![2, 3, 4]);
        assert_eq!((captured.epoch, captured.revision), (4, 9));
    }

    #[test]
    fn capture_rejects_out_of_bounds_truncated_and_oversized_ranges() {
        let adapter = FixtureAdapter {
            identity: identity("rom-a"),
            ranges: vec![MemoryRange {
                name: "bad".into(),
                offset: usize::MAX,
                length: 2,
            }],
        };
        assert_eq!(
            capture(&adapter, &mut FixtureReader { bytes: vec![1] }, 1, 1).unwrap_err(),
            CaptureError::OutOfBounds
        );
        let truncated = FixtureAdapter {
            identity: identity("rom-a"),
            ranges: vec![MemoryRange {
                name: "bad".into(),
                offset: 0,
                length: 3,
            }],
        };
        assert_eq!(
            capture(&truncated, &mut ShortReader, 1, 1).unwrap_err(),
            CaptureError::Truncated
        );
        let oversized = FixtureAdapter {
            identity: identity("rom-a"),
            ranges: vec![MemoryRange {
                name: "large".into(),
                offset: 0,
                length: 1_048_577,
            }],
        };
        assert_eq!(
            capture(&oversized, &mut FixtureReader { bytes: vec![] }, 1, 1).unwrap_err(),
            CaptureError::TooLarge
        );
    }

    #[test]
    fn delayed_capture_is_rejected_after_epoch_changes() {
        let mut gate = super::CaptureEpochGate::new("session-a", identity("rom-a"), 3);
        let old_capture = capture(
            &FixtureAdapter {
                identity: identity("rom-a"),
                ranges: vec![],
            },
            &mut FixtureReader { bytes: vec![] },
            3,
            1,
        )
        .unwrap();
        gate.advance("session-a", identity("rom-a"), 4);
        assert_eq!(
            gate.accept("session-a", &old_capture),
            Err(CaptureError::StaleEpoch)
        );
        let current = capture(
            &FixtureAdapter {
                identity: identity("rom-a"),
                ranges: vec![],
            },
            &mut FixtureReader { bytes: vec![] },
            4,
            2,
        )
        .unwrap();
        assert_eq!(
            gate.accept("other-session", &current),
            Err(CaptureError::WrongSession)
        );
        assert_eq!(gate.accept("session-a", &current), Ok(()));
        assert_eq!(
            gate.accept("session-a", &current),
            Err(CaptureError::StaleRevision)
        );
    }
}
