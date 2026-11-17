//! Strict, read-only decoder for retail English Gen III GBA battery saves.
//! The game profile must come from verified ROM identity; save files alone do not encode it.

const SECTOR: usize = 0x1000;
const DATA: usize = 0xF80;
const SECTORS: usize = 14;
const SIGNATURE: u32 = 0x0801_2025;
const SUBSTRUCTS: [[usize; 4]; 24] = [
    [0, 1, 2, 3],
    [0, 1, 3, 2],
    [0, 2, 1, 3],
    [0, 2, 3, 1],
    [0, 3, 1, 2],
    [0, 3, 2, 1],
    [1, 0, 2, 3],
    [1, 0, 3, 2],
    [1, 2, 0, 3],
    [1, 2, 3, 0],
    [1, 3, 0, 2],
    [1, 3, 2, 0],
    [2, 0, 1, 3],
    [2, 0, 3, 1],
    [2, 1, 0, 3],
    [2, 1, 3, 0],
    [2, 3, 0, 1],
    [2, 3, 1, 0],
    [3, 0, 1, 2],
    [3, 0, 2, 1],
    [3, 1, 0, 2],
    [3, 1, 2, 0],
    [3, 2, 0, 1],
    [3, 2, 1, 0],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Gen3Format {
    RubySapphire,
    Emerald,
    FireRedLeafGreen,
}

impl Gen3Format {
    fn sizes(self) -> (usize, usize) {
        match self {
            Self::RubySapphire => (0x890, 0x3AC0),
            Self::Emerald => (0xF2C, 0x3D88),
            Self::FireRedLeafGreen => (0xF24, 0x3D68),
        }
    }
    fn party_offsets(self) -> (usize, usize) {
        match self {
            Self::FireRedLeafGreen => (0x34, 0x38),
            _ => (0x234, 0x238),
        }
    }
    fn encryption_key(self) -> Option<usize> {
        match self {
            Self::RubySapphire => None,
            Self::Emerald => Some(0xAC),
            Self::FireRedLeafGreen => Some(0xF20),
        }
    }
    fn pockets(self) -> [(usize, usize); 5] {
        match self {
            Self::RubySapphire => [
                (0x560, 20),
                (0x5B0, 20),
                (0x600, 16),
                (0x640, 64),
                (0x740, 46),
            ],
            Self::Emerald => [
                (0x560, 30),
                (0x5D8, 30),
                (0x650, 16),
                (0x690, 64),
                (0x790, 46),
            ],
            Self::FireRedLeafGreen => [
                (0x310, 42),
                (0x3B8, 30),
                (0x430, 13),
                (0x464, 58),
                (0x54C, 43),
            ],
        }
    }
    fn chunk_size(self, id: usize) -> usize {
        let (sb2, sb1) = self.sizes();
        let size = match id {
            0 => sb2,
            1..=4 => sb1 - (id - 1) * DATA,
            _ => 0x83D0 - (id - 5) * DATA,
        };
        size.min(DATA)
    }
    fn party_count(self) -> usize {
        self.party_offsets().0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PartyMember {
    pub species_id: u16,
    pub level: u8,
    pub is_egg: bool,
    pub hp: u16,
    pub max_hp: u16,
    pub moves: Vec<MoveSlot>,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct MoveSlot {
    pub move_id: u16,
    pub pp: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ItemStack {
    pub item_id: u16,
    pub quantity: u16,
}

/// Decode the six 100-byte `Pokemon` structs captured from Gen III GBA RAM.
/// The caller must resolve the game's exact ROM identity before choosing this layout.
pub fn decode_party_ram(count: usize, bytes: &[u8]) -> Result<Vec<PartyMember>, String> {
    if count > 6 {
        return Err(format!("party count inválido: {count}"));
    }
    if bytes.len() < 6 * 100 {
        return Err("captura de party truncada: se requieren seis slots".into());
    }
    let mut party = Vec::with_capacity(count);
    for i in 0..count {
        let off = i * 100;
        let record = decode_mon(&bytes[off..off + 80])
            .map_err(|error| format!("slot de equipo {}: {error}", i + 1))?;
        let level = bytes[off + 0x54];
        let hp = u16le(bytes, off + 0x56);
        let max_hp = u16le(bytes, off + 0x58);
        if !record.is_egg && !(1..=100).contains(&level) {
            return Err(format!("nivel inválido en slot de equipo {}", i + 1));
        }
        if max_hp == 0 || hp > max_hp {
            return Err(format!("HP inválido en slot de equipo {}", i + 1));
        }
        party.push(PartyMember {
            species_id: record.species_id,
            level,
            is_egg: record.is_egg,
            hp,
            max_hp,
            moves: record
                .move_ids
                .iter()
                .zip(record.move_pp)
                .map(|(&move_id, pp)| MoveSlot { move_id, pp })
                .collect(),
        });
    }
    Ok(party)
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Snapshot {
    pub format: Gen3Format,
    pub counter: u32,
    pub party: Vec<PartyMember>,
    pub bag: [Vec<ItemStack>; 5],
    pub sha256: String,
}

/// Exact-identity adapter for the party and party-count regions in Gen III GBA RAM.
/// Addresses are supplied only by a verified profile; unknown identities stay unresolved.
pub struct Gen3PartyMemoryAdapter {
    identity: crate::memory_adapter::GameIdentity,
    ranges: [crate::memory_adapter::MemoryRange; 2],
}

impl Gen3PartyMemoryAdapter {
    pub fn new(
        identity: crate::memory_adapter::GameIdentity,
        party_count_address: usize,
        party_address: usize,
    ) -> Self {
        Self::new_in_address_space(identity, party_count_address, party_address, None)
    }

    pub fn new_in_address_space(
        identity: crate::memory_adapter::GameIdentity,
        party_count_address: usize,
        party_address: usize,
        address_space: Option<String>,
    ) -> Self {
        Self {
            identity,
            ranges: [
                crate::memory_adapter::MemoryRange {
                    name: "party_count".into(),
                    address_space: address_space.clone(),
                    offset: party_count_address,
                    length: 1,
                },
                crate::memory_adapter::MemoryRange {
                    name: "party".into(),
                    address_space,
                    offset: party_address,
                    length: 600,
                },
            ],
        }
    }

    pub fn decode(
        &self,
        capture: &crate::memory_adapter::MemoryCapture,
    ) -> Result<Vec<PartyMember>, String> {
        if capture.identity != self.identity {
            return Err("memory capture identity does not match the Gen 3 profile".into());
        }
        let count = capture
            .segments
            .iter()
            .find(|segment| segment.name == "party_count")
            .filter(|segment| segment.bytes.len() == 1)
            .ok_or_else(|| "party count capture is missing or truncated".to_string())?
            .bytes[0] as usize;
        let party = capture
            .segments
            .iter()
            .find(|segment| segment.name == "party")
            .filter(|segment| segment.bytes.len() == 600)
            .ok_or_else(|| "party capture is missing or truncated".to_string())?;
        decode_party_ram(count, &party.bytes)
    }
}

impl crate::memory_adapter::MemoryAdapter for Gen3PartyMemoryAdapter {
    fn identity(&self) -> &crate::memory_adapter::GameIdentity {
        &self.identity
    }

    fn ranges(&self) -> &[crate::memory_adapter::MemoryRange] {
        &self.ranges
    }

    fn snapshot(
        &self,
        capture: &crate::memory_adapter::MemoryCapture,
    ) -> Result<serde_json::Value, crate::memory_adapter::CaptureError> {
        self.decode(capture)
            .map(|party| serde_json::json!({ "party_count": party.len(), "party": party }))
            .map_err(|_| crate::memory_adapter::CaptureError::DecodeRejected)
    }
}

pub fn read(bytes: &[u8], format: Gen3Format) -> Result<Snapshot, String> {
    if bytes.len() < SECTOR * SECTORS * 2 {
        return Err("save menor que los dos slots Gen 3".into());
    }
    let slots = [read_slot(bytes, 0, format), read_slot(bytes, 1, format)];
    let valid: Vec<_> = (0..2).filter(|&i| slots[i].is_some()).collect();
    if valid.is_empty() {
        return Err("ningún slot Gen 3 tiene sus 14 sectores y checksums válidos".into());
    }
    let slot = if valid.len() == 1 {
        valid[0]
    } else {
        let a = slots[0].unwrap().1;
        let b = slots[1].unwrap().1;
        if (a == u32::MAX && b == 0) || (b == u32::MAX && a == 0) {
            if a == u32::MAX { 1 } else { 0 }
        } else if a < b {
            1
        } else {
            0
        }
    };
    let (offsets, counter) = slots[slot].unwrap();
    let (sb2_size, sb1_size) = format.sizes();
    let mut sb2 = vec![0; sb2_size];
    let mut sb1 = vec![0; sb1_size];
    for id in 0..5 {
        let size = format.chunk_size(id);
        let dest = match id {
            0 => &mut sb2[..],
            _ => &mut sb1[(id - 1) * DATA..],
        };
        dest[..size].copy_from_slice(&bytes[offsets[id]..offsets[id] + size]);
    }
    let (count_off, party_off) = format.party_offsets();
    let count = sb1[count_off] as usize;
    if count > 6 {
        return Err(format!("party count inválido: {count}"));
    }
    let mut party = Vec::with_capacity(count);
    for i in 0..count {
        let off = party_off + i * 100;
        let rec = decode_mon(&sb1[off..off + 80])?;
        let level = sb1[off + 0x54];
        let hp = u16le(&sb1, off + 0x56);
        let max_hp = u16le(&sb1, off + 0x58);
        if !rec.is_egg && !(1..=100).contains(&level) {
            return Err(format!("nivel inválido en slot de equipo {}", i + 1));
        }
        if max_hp == 0 || hp > max_hp {
            return Err(format!("HP inválido en slot de equipo {}", i + 1));
        }
        party.push(PartyMember {
            species_id: rec.species_id,
            level,
            is_egg: rec.is_egg,
            hp,
            max_hp,
            moves: rec
                .move_ids
                .iter()
                .zip(rec.move_pp)
                .map(|(&move_id, pp)| MoveSlot { move_id, pp })
                .collect(),
        });
    }
    for i in count..6 {
        let off = party_off + i * 100;
        if sb1[off..off + 100].iter().any(|b| *b != 0) {
            return Err(format!(
                "slot de equipo {} no vacío fuera del conteo",
                i + 1
            ));
        }
    }
    let key = format
        .encryption_key()
        .map(|off| u32le(&sb2, off) as u16)
        .unwrap_or(0);
    let bag = (0..5)
        .map(|p| -> Result<Vec<ItemStack>, String> {
            let (off, slots) = format.pockets()[p];
            (0..slots)
                .filter_map(|i| {
                    let at = off + i * 4;
                    let item = u16le(&sb1, at);
                    let quantity = u16le(&sb1, at + 2) ^ key;
                    if item == 0 {
                        if quantity != 0 {
                            return Some(Err(
                                "cantidad guardada en un slot de mochila vacío".to_string()
                            ));
                        }
                        return None;
                    }
                    if quantity == 0 || quantity > 999 {
                        return Some(Err(format!("cantidad inválida para el objeto ID {item}")));
                    }
                    Some(Ok(ItemStack {
                        item_id: item,
                        quantity,
                    }))
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bag: [Vec<ItemStack>; 5] = bag.try_into().map_err(|_| "layout de mochila incompleto")?;
    Ok(Snapshot {
        format,
        counter,
        party,
        bag,
        sha256: sha256(bytes),
    })
}

fn read_slot(bytes: &[u8], slot: usize, format: Gen3Format) -> Option<([usize; SECTORS], u32)> {
    let mut offsets = [usize::MAX; SECTORS];
    let mut counter = None;
    for physical in 0..SECTORS {
        let off = (slot * SECTORS + physical) * SECTOR;
        if u32le(bytes, off + 0xFF8) != SIGNATURE {
            continue;
        }
        let id = u16le(bytes, off + 0xFF4) as usize;
        if id >= SECTORS || offsets[id] != usize::MAX {
            return None;
        }
        let size = format.chunk_size(id);
        if u16le(bytes, off + 0xFF6) != checksum(&bytes[off..off + size]) {
            return None;
        }
        let current = u32le(bytes, off + 0xFFC);
        if counter.is_some_and(|old| old != current) {
            return None;
        }
        counter = Some(current);
        offsets[id] = off;
    }
    if offsets.contains(&usize::MAX) {
        None
    } else {
        Some((offsets, counter?))
    }
}

struct DecodedMon {
    species_id: u16,
    is_egg: bool,
    move_ids: [u16; 4],
    move_pp: [u8; 4],
}

fn decode_mon(bytes: &[u8]) -> Result<DecodedMon, String> {
    let personality = u32le(bytes, 0);
    let key = personality ^ u32le(bytes, 4);
    let order = SUBSTRUCTS[(personality % 24) as usize];
    let mut data = [0u8; 48];
    let mut sum = 0u16;
    for physical in 0..4 {
        for word in 0..3 {
            let encrypted = u32le(bytes, 0x20 + physical * 12 + word * 4);
            let value = encrypted ^ key;
            let target = order[physical] * 12 + word * 4;
            data[target..target + 4].copy_from_slice(&value.to_le_bytes());
            sum = sum
                .wrapping_add(value as u16)
                .wrapping_add((value >> 16) as u16);
        }
    }
    if sum != u16le(bytes, 0x1C) {
        return Err("checksum de Pokémon inválido".into());
    }
    let species = u16le(&data, 0);
    let is_egg = (u32le(&data, 40) >> 30) & 1 != 0;
    if !is_egg && !(1..=412).contains(&species) {
        return Err(format!("species ID inválido: {species}"));
    }
    let move_ids = std::array::from_fn(|i| u16le(&data, 12 + i * 2));
    let move_pp = std::array::from_fn(|i| data[20 + i]);
    Ok(DecodedMon {
        species_id: species,
        is_egg,
        move_ids,
        move_pp,
    })
}
fn checksum(bytes: &[u8]) -> u16 {
    let sum = bytes.chunks_exact(4).fold(0u32, |s, w| {
        s.wrapping_add(u32::from_le_bytes(w.try_into().unwrap()))
    });
    (sum as u16).wrapping_add((sum >> 16) as u16)
}
fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put16(bytes: &mut [u8], off: usize, value: u16) {
        bytes[off..off + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put32(bytes: &mut [u8], off: usize, value: u32) {
        bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn live_party_decoder_rejects_bad_count_and_truncated_capture() {
        assert!(
            decode_party_ram(7, &[0; 600])
                .unwrap_err()
                .contains("count")
        );
        assert!(
            decode_party_ram(1, &[0; 599])
                .unwrap_err()
                .contains("truncada")
        );
    }

    #[test]
    fn live_party_decoder_checks_mon_checksum_and_party_stats() {
        let mut bytes = vec![0; 600];
        // PID/OTID zero means key zero and canonical G-A-E-M substructure order.
        put16(&mut bytes, 0x20, 25);
        put16(&mut bytes, 0x20 + 12, 33);
        bytes[0x20 + 12 + 8] = 15;
        let sum = checksum(&bytes[0x20..0x50]);
        put16(&mut bytes, 0x1C, sum);
        bytes[0x54] = 12;
        put16(&mut bytes, 0x56, 5);
        put16(&mut bytes, 0x58, 10);

        let party = decode_party_ram(1, &bytes).unwrap();
        assert_eq!(party.len(), 1);
        assert_eq!(party[0].species_id, 25);
        assert_eq!(party[0].level, 12);
        assert_eq!(party[0].hp, 5);
        assert_eq!(party[0].max_hp, 10);
        assert_eq!(
            party[0].moves[0],
            MoveSlot {
                move_id: 33,
                pp: 15
            }
        );

        bytes[0x1C] ^= 1;
        assert!(
            decode_party_ram(1, &bytes)
                .unwrap_err()
                .contains("checksum")
        );
    }

    #[test]
    fn exact_identity_party_adapter_decodes_only_its_own_complete_capture() {
        use crate::memory_adapter::{GameIdentity, MemoryCapture, MemoryReader, OwnedSegment};

        let identity = GameIdentity {
            system: "GBA".into(),
            rom_sha256: "rom-sha256".into(),
            core_sha256: "core-sha256".into(),
            game_revision: "BPEE-rev0".into(),
        };
        let adapter = Gen3PartyMemoryAdapter::new(identity.clone(), 0x0200_1000, 0x0200_2000);
        struct FixtureReader(Vec<u8>);
        impl MemoryReader for FixtureReader {
            fn read(
                &mut self,
                _address_space: Option<&str>,
                address: usize,
                length: usize,
            ) -> Result<Vec<u8>, crate::memory_adapter::CaptureError> {
                let offset = address
                    .checked_sub(0x0200_1000)
                    .ok_or(crate::memory_adapter::CaptureError::OutOfBounds)?;
                let end = offset
                    .checked_add(length)
                    .ok_or(crate::memory_adapter::CaptureError::OutOfBounds)?;
                self.0
                    .get(offset..end)
                    .map(|bytes| bytes.to_vec())
                    .ok_or(crate::memory_adapter::CaptureError::OutOfBounds)
            }
        }
        let mut memory = vec![0; 0x1000 + 600];
        memory[0x1000] = 0;
        let mut reader = FixtureReader(memory);
        let captured = crate::memory_adapter::capture(&adapter, &mut reader, 2, 8).unwrap();
        assert!(adapter.decode(&captured).unwrap().is_empty());

        let mut capture = MemoryCapture {
            identity,
            epoch: 2,
            revision: 8,
            segments: vec![
                OwnedSegment {
                    name: "party_count".into(),
                    bytes: vec![0],
                },
                OwnedSegment {
                    name: "party".into(),
                    bytes: vec![0; 600],
                },
            ],
        };
        assert!(adapter.decode(&capture).unwrap().is_empty());

        capture.identity.rom_sha256 = "different-rom".into();
        assert!(adapter.decode(&capture).unwrap_err().contains("identity"));
        capture.identity = adapter.identity.clone();
        capture.segments.pop();
        assert!(adapter.decode(&capture).unwrap_err().contains("party"));
    }

    #[test]
    fn memory_poller_emits_decoded_snapshot_only_for_registered_exact_identity() {
        use crate::memory_adapter::{
            GameIdentity, MemoryAdapterRegistry, MemoryPoller, MemoryReader,
        };
        let identity = GameIdentity {
            system: "GBA".into(),
            rom_sha256: "rom-sha256".into(),
            core_sha256: "core-sha256".into(),
            game_revision: "BPEE-rev0".into(),
        };
        let adapter = Gen3PartyMemoryAdapter::new(identity.clone(), 0x0200_1000, 0x0200_2000);
        let mut registry = MemoryAdapterRegistry::default();
        registry.register(Box::new(adapter)).unwrap();
        struct FixtureReader(Vec<u8>);
        impl MemoryReader for FixtureReader {
            fn read(
                &mut self,
                _address_space: Option<&str>,
                address: usize,
                length: usize,
            ) -> Result<Vec<u8>, crate::memory_adapter::CaptureError> {
                let offset = address
                    .checked_sub(0x0200_1000)
                    .ok_or(crate::memory_adapter::CaptureError::OutOfBounds)?;
                let end = offset
                    .checked_add(length)
                    .ok_or(crate::memory_adapter::CaptureError::OutOfBounds)?;
                self.0
                    .get(offset..end)
                    .map(|bytes| bytes.to_vec())
                    .ok_or(crate::memory_adapter::CaptureError::OutOfBounds)
            }
        }
        let mut memory = vec![0; 0x1000 + 600];
        memory[0] = 1;
        let mut reader = FixtureReader(memory);
        let mut poller = MemoryPoller::new("session-1", identity.clone(), 4);
        assert_eq!(
            poller.poll(&registry, &mut reader),
            Err(crate::memory_adapter::CaptureError::DecodeRejected)
        );
        let party = 0x1000;
        put16(&mut reader.0, party + 0x20, 25);
        put16(&mut reader.0, party + 0x20 + 12, 33);
        reader.0[party + 0x20 + 12 + 8] = 15;
        let sum = checksum(&reader.0[party + 0x20..party + 0x50]);
        put16(&mut reader.0, party + 0x1C, sum);
        reader.0[party + 0x54] = 12;
        put16(&mut reader.0, party + 0x56, 5);
        put16(&mut reader.0, party + 0x58, 10);
        let snapshot = poller.poll(&registry, &mut reader).unwrap().unwrap();
        assert_eq!(snapshot["party_count"], 1);
        assert_eq!(snapshot["party"][0]["species_id"], 25);

        let other = GameIdentity {
            rom_sha256: "other-rom".into(),
            ..identity.clone()
        };
        poller.reset("session-1", other, 5);
        assert_eq!(poller.poll(&registry, &mut reader).unwrap(), None);
    }

    fn fixture() -> Vec<u8> {
        let mut file = vec![0u8; 0x20000];
        let format = Gen3Format::Emerald;
        let (sb2, sb1) = format.sizes();
        let mut blocks = vec![vec![0u8; sb2], vec![0u8; sb1], vec![0u8; 0x83D0]];
        blocks[1][0x234] = 0;
        // A bag stack quantity is stored XORed with the low half of Emerald's encryption key.
        put32(&mut blocks[0], 0xAC, 0x00001234);
        for (offset, slots) in format.pockets() {
            for index in 0..slots {
                put16(&mut blocks[1], offset + index * 4 + 2, 0x1234);
            }
        }
        put16(&mut blocks[1], 0x560, 13);
        put16(&mut blocks[1], 0x562, 5 ^ 0x1234);
        for id in 0..14 {
            let start = id * SECTOR;
            let (block, source, size) = match id {
                0 => (&blocks[0], 0, blocks[0].len()),
                1..=4 => (
                    &blocks[1],
                    (id - 1) * DATA,
                    (blocks[1].len() - (id - 1) * DATA).min(DATA),
                ),
                _ => (
                    &blocks[2],
                    (id - 5) * DATA,
                    (blocks[2].len() - (id - 5) * DATA).min(DATA),
                ),
            };
            file[start..start + size].copy_from_slice(&block[source..source + size]);
            put16(&mut file, start + 0xFF4, id as u16);
            let sum = checksum(&file[start..start + size]);
            put16(&mut file, start + 0xFF6, sum);
            put32(&mut file, start + 0xFF8, SIGNATURE);
        }
        for id in 0..14 {
            let off = id * SECTOR;
            put32(&mut file, off + 0xFFC, 7);
        }
        file
    }

    #[test]
    fn decodes_empty_party_and_encrypted_bag_stack_without_mutating_save() {
        let bytes = fixture();
        let original = bytes.clone();
        let snapshot = read(&bytes, Gen3Format::Emerald).unwrap();
        assert!(snapshot.party.is_empty());
        assert_eq!(
            snapshot.bag[0],
            vec![ItemStack {
                item_id: 13,
                quantity: 5
            }]
        );
        assert_eq!(bytes, original);
        assert_eq!(snapshot.sha256, sha256(&bytes));
    }

    #[test]
    fn decodes_a_party_slot_and_level_after_mon_checksum_validation() {
        let mut bytes = fixture();
        let sector = 0x1000;
        let party = sector + 0x238;
        bytes[sector + 0x234] = 1;
        // Personality and OT ID are zero, so the first physical substructure is Growth.
        bytes[party + 0x20..party + 0x22].copy_from_slice(&25u16.to_le_bytes());
        bytes[party + 0x20 + 12..party + 0x20 + 14].copy_from_slice(&33u16.to_le_bytes());
        bytes[party + 0x20 + 12 + 8] = 15;
        bytes[party + 0x1C..party + 0x1E].copy_from_slice(&73u16.to_le_bytes());
        bytes[party + 0x54] = 12;
        bytes[party + 0x56..party + 0x58].copy_from_slice(&5u16.to_le_bytes());
        bytes[party + 0x58..party + 0x5A].copy_from_slice(&10u16.to_le_bytes());
        let sum = checksum(&bytes[sector..sector + DATA]);
        bytes[sector + 0xFF6..sector + 0xFF8].copy_from_slice(&sum.to_le_bytes());
        let snapshot = read(&bytes, Gen3Format::Emerald).unwrap();
        assert_eq!(
            snapshot.party,
            vec![PartyMember {
                species_id: 25,
                level: 12,
                is_egg: false,
                hp: 5,
                max_hp: 10,
                moves: vec![
                    MoveSlot {
                        move_id: 33,
                        pp: 15
                    },
                    MoveSlot { move_id: 0, pp: 0 },
                    MoveSlot { move_id: 0, pp: 0 },
                    MoveSlot { move_id: 0, pp: 0 }
                ],
            }]
        );
    }

    #[test]
    fn rejects_invalid_party_count_and_corrupt_sector_checksum() {
        let mut bytes = fixture();
        bytes[0x1000 + 0x234] = 255;
        // Recompute sector 1 checksum so this reaches the semantic count check.
        let sum = checksum(&bytes[0x1000..0x1000 + DATA]);
        put16(&mut bytes, 0x1000 + 0xFF6, sum);
        assert!(
            read(&bytes, Gen3Format::Emerald)
                .unwrap_err()
                .contains("count")
        );

        let mut bytes = fixture();
        bytes[0] ^= 1;
        assert!(read(&bytes, Gen3Format::Emerald).is_err());
    }

    #[test]
    fn rejects_wrong_size_and_corrupt_slots() {
        assert!(read(&[0; 32], Gen3Format::Emerald).is_err());
        let mut bytes = fixture();
        bytes[0x1000 + 0xFF4..0x1000 + 0xFF6].copy_from_slice(&99u16.to_le_bytes());
        assert!(read(&bytes, Gen3Format::Emerald).is_err());
    }

    #[test]
    fn rejects_invalid_party_hp_and_zero_bag_quantity_with_valid_sector_checksums() {
        let mut bytes = fixture();
        let sector = 0x1000;
        let party = sector + 0x238;
        bytes[sector + 0x234] = 1;
        bytes[party + 0x20..party + 0x22].copy_from_slice(&25u16.to_le_bytes());
        bytes[party + 0x1C..party + 0x1E].copy_from_slice(&25u16.to_le_bytes());
        bytes[party + 0x54] = 12;
        bytes[party + 0x56..party + 0x58].copy_from_slice(&11u16.to_le_bytes());
        bytes[party + 0x58..party + 0x5A].copy_from_slice(&10u16.to_le_bytes());
        let sum = checksum(&bytes[sector..sector + DATA]);
        bytes[sector + 0xFF6..sector + 0xFF8].copy_from_slice(&sum.to_le_bytes());
        assert!(
            read(&bytes, Gen3Format::Emerald)
                .unwrap_err()
                .contains("HP inválido")
        );

        let mut bytes = fixture();
        bytes[0x1000 + 0x560..0x1000 + 0x562].copy_from_slice(&13u16.to_le_bytes());
        bytes[0x1000 + 0x562..0x1000 + 0x564].copy_from_slice(&0x1234u16.to_le_bytes());
        let sum = checksum(&bytes[0x1000..0x1000 + DATA]);
        bytes[0x1000 + 0xFF6..0x1000 + 0xFF8].copy_from_slice(&sum.to_le_bytes());
        assert!(
            read(&bytes, Gen3Format::Emerald)
                .unwrap_err()
                .contains("cantidad inválida")
        );
    }
}
