//! Bounded structural checks for supported ROM formats. Header matches remain partial identity.
use crate::domain::IdentityConfidence;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomInspection {
    pub system: &'static str,
    pub title: Option<String>,
    pub game_code: Option<String>,
    pub revision: Option<u8>,
    pub confidence: IdentityConfidence,
    pub header_checksum_valid: Option<bool>,
}

pub fn inspect_bytes(file_name: &str, bytes: &[u8]) -> Option<RomInspection> {
    inspect_header(file_name, bytes, bytes.len() as u64)
}

pub fn inspect_file(path: &Path) -> Option<RomInspection> {
    let file_size = std::fs::metadata(path).ok()?.len();
    let mut header = [0u8; 0x200];
    let mut file = std::fs::File::open(path).ok()?;
    let count = file.read(&mut header).ok()?;
    inspect_header(path.file_name()?.to_str()?, &header[..count], file_size)
}

fn inspect_header(file_name: &str, bytes: &[u8], file_size: u64) -> Option<RomInspection> {
    let extension = file_name.rsplit('.').next()?.to_ascii_lowercase();
    match extension.as_str() {
        "gba" => inspect_gba(bytes),
        "gb" => inspect_gb(bytes, false),
        "gbc" => inspect_gb(bytes, true),
        "nds" => inspect_ds(bytes, file_size),
        _ => None,
    }
}

fn inspect_gba(bytes: &[u8]) -> Option<RomInspection> {
    if bytes.len() < 0xc0 || bytes[0xb2] != 0x96 {
        return None;
    }
    let code = clean_ascii(&bytes[0xac..0xb0]);
    if code.len() != 4 || !code.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    Some(RomInspection {
        system: "GBA",
        title: clean_ascii(&bytes[0xa0..0xac]).into_option(),
        game_code: Some(code),
        revision: Some(bytes[0xbc]),
        confidence: IdentityConfidence::Partial,
        header_checksum_valid: Some(bytes[0xbd] == gba_header_checksum(bytes)),
    })
}

pub fn gba_header_checksum(bytes: &[u8]) -> u8 {
    bytes[0xa0..0xbd]
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
        .wrapping_add(0x19)
        .wrapping_neg()
}

fn inspect_gb(bytes: &[u8], expect_color: bool) -> Option<RomInspection> {
    const LOGO: [u8; 48] = [
        0xce, 0xed, 0x66, 0x66, 0xcc, 0x0d, 0x00, 0x0b, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0c, 0x00,
        0x0d, 0x00, 0x08, 0x11, 0x1f, 0x88, 0x89, 0x00, 0x0e, 0xdc, 0xcc, 0x6e, 0xe6, 0xdd, 0xdd,
        0xd9, 0x99, 0xbb, 0xbb, 0x67, 0x63, 0x6e, 0x0e, 0xec, 0xcc, 0xdd, 0xdc, 0x99, 0x9f, 0xbb,
        0xb9, 0x33, 0x3e,
    ];
    if bytes.len() < 0x150 || bytes[0x104..0x134] != LOGO {
        return None;
    }
    let color = bytes[0x143] & 0x80 != 0;
    if color != expect_color {
        return None;
    }
    Some(RomInspection {
        system: if color { "GBC" } else { "GB" },
        title: clean_ascii(&bytes[0x134..0x143]).into_option(),
        game_code: None,
        revision: None,
        confidence: IdentityConfidence::Partial,
        header_checksum_valid: Some(bytes[0x14d] == gb_header_checksum(bytes)),
    })
}

fn gb_header_checksum(bytes: &[u8]) -> u8 {
    bytes[0x134..=0x14c]
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1))
}

fn inspect_ds(bytes: &[u8], file_size: u64) -> Option<RomInspection> {
    if bytes.len() < 0x200 || u32::from_le_bytes(bytes[0x84..0x88].try_into().ok()?) < 0x160 {
        return None;
    }
    let arm9_offset = u32::from_le_bytes(bytes[0x20..0x24].try_into().ok()?) as usize;
    let arm9_size = u32::from_le_bytes(bytes[0x2c..0x30].try_into().ok()?) as usize;
    if arm9_offset < 0x200
        || arm9_size == 0
        || arm9_offset.checked_add(arm9_size)? as u64 > file_size
    {
        return None;
    }
    let code = clean_ascii(&bytes[0x0c..0x10]);
    let game_code =
        (code.len() == 4 && code.bytes().all(|b| b.is_ascii_alphanumeric())).then_some(code);
    Some(RomInspection {
        system: "DS",
        title: clean_ascii(&bytes[0..12]).into_option(),
        game_code,
        revision: None,
        confidence: IdentityConfidence::Partial,
        header_checksum_valid: None,
    })
}

fn clean_ascii(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .take_while(|c| *c != '\0')
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

trait NonEmpty {
    fn into_option(self) -> Option<String>;
}

impl NonEmpty for String {
    fn into_option(self) -> Option<String> {
        (!self.is_empty()).then_some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{inspect_bytes, inspect_file};
    use std::io::Write;

    fn gb_fixture(color: bool, title: &[u8]) -> Vec<u8> {
        const LOGO: [u8; 48] = [
            0xce, 0xed, 0x66, 0x66, 0xcc, 0x0d, 0x00, 0x0b, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0c,
            0x00, 0x0d, 0x00, 0x08, 0x11, 0x1f, 0x88, 0x89, 0x00, 0x0e, 0xdc, 0xcc, 0x6e, 0xe6,
            0xdd, 0xdd, 0xd9, 0x99, 0xbb, 0xbb, 0x67, 0x63, 0x6e, 0x0e, 0xec, 0xcc, 0xdd, 0xdc,
            0x99, 0x9f, 0xbb, 0xb9, 0x33, 0x3e,
        ];
        let mut bytes = vec![0; 0x150];
        bytes[0x104..0x134].copy_from_slice(&LOGO);
        let title_len = title.len().min(15);
        bytes[0x134..0x134 + title_len].copy_from_slice(&title[..title_len]);
        bytes[0x143] = if color { 0x80 } else { 0 };
        bytes[0x14d] = bytes[0x134..=0x14c]
            .iter()
            .fold(0u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1));
        bytes
    }

    #[test]
    fn gb_header_checksum_is_reported_but_does_not_gate_rom_inspection() {
        const LOGO: [u8; 48] = [
            0xce, 0xed, 0x66, 0x66, 0xcc, 0x0d, 0x00, 0x0b, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0c,
            0x00, 0x0d, 0x00, 0x08, 0x11, 0x1f, 0x88, 0x89, 0x00, 0x0e, 0xdc, 0xcc, 0x6e, 0xe6,
            0xdd, 0xdd, 0xd9, 0x99, 0xbb, 0xbb, 0x67, 0x63, 0x6e, 0x0e, 0xec, 0xcc, 0xdd, 0xdc,
            0x99, 0x9f, 0xbb, 0xb9, 0x33, 0x3e,
        ];
        let mut bytes = vec![0u8; 0x150];
        bytes[0x104..0x134].copy_from_slice(&LOGO);
        bytes[0x134..0x138].copy_from_slice(b"HOME");
        let checksum = bytes[0x134..=0x14c]
            .iter()
            .fold(0u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1));
        bytes[0x14d] = checksum;
        assert_eq!(
            inspect_bytes("homebrew.gb", &bytes)
                .unwrap()
                .header_checksum_valid,
            Some(true)
        );
        bytes[0x14d] ^= 1;
        assert_eq!(
            inspect_bytes("homebrew.gb", &bytes)
                .unwrap()
                .header_checksum_valid,
            Some(false)
        );
    }

    #[test]
    fn bounded_inspection_rejects_truncated_headers_and_bad_gba_signature() {
        assert!(inspect_bytes("short.gba", &[0; 0xbf]).is_none());
        let mut gba = vec![0; 0xc0];
        gba[0xac..0xb0].copy_from_slice(b"ABCE");
        gba[0xb2] = 0x96;
        assert_eq!(
            inspect_bytes("fixture.gba", &gba)
                .unwrap()
                .header_checksum_valid,
            Some(false)
        );
        gba[0xb2] = 0;
        assert!(inspect_bytes("fixture.gba", &gba).is_none());

        assert!(inspect_bytes("short.gb", &[0; 0x14f]).is_none());
        assert!(inspect_bytes("short.gbc", &[0; 0x14f]).is_none());
        assert!(inspect_bytes("unknown.bin", &[0; 0x400]).is_none());
    }

    #[test]
    fn gb_and_gbc_logo_color_flag_and_optional_title_are_checked() {
        let gb = gb_fixture(false, b"  HOME  ");
        let inspection = inspect_bytes("fixture.gb", &gb).unwrap();
        assert_eq!(inspection.system, "GB");
        assert_eq!(inspection.title.as_deref(), Some("HOME"));
        assert!(inspection.header_checksum_valid.unwrap());
        assert!(inspect_bytes("fixture.gbc", &gb).is_none());

        let gbc = gb_fixture(true, b"\0\0");
        let inspection = inspect_bytes("fixture.gbc", &gbc).unwrap();
        assert_eq!(inspection.system, "GBC");
        assert_eq!(inspection.title, None);
        assert!(inspect_bytes("fixture.gb", &gbc).is_none());
    }

    #[test]
    fn gba_fields_are_reported_and_invalid_game_codes_are_rejected() {
        let mut gba = vec![0; 0xc0];
        gba[0xa0..0xa4].copy_from_slice(b"HOME");
        gba[0xac..0xb0].copy_from_slice(b"ABCD");
        gba[0xb2] = 0x96;
        gba[0xbc] = 7;
        gba[0xbd] = super::gba_header_checksum(&gba);
        let inspection = inspect_bytes("fixture.gba", &gba).unwrap();
        assert_eq!(inspection.title.as_deref(), Some("HOME"));
        assert_eq!(inspection.game_code.as_deref(), Some("ABCD"));
        assert_eq!(inspection.revision, Some(7));
        assert_eq!(inspection.header_checksum_valid, Some(true));

        gba[0xac..0xb0].copy_from_slice(b"AB?D");
        assert!(inspect_bytes("fixture.gba", &gba).is_none());
    }

    #[test]
    fn file_inspection_reads_only_a_bounded_header_from_large_roms() {
        let path =
            std::env::temp_dir().join(format!("vyntra-inspect-bounded-{}.GBA", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut file = std::fs::File::create(&path).unwrap();
        let mut header = vec![0; 0xc0];
        header[0xac..0xb0].copy_from_slice(b"ABCD");
        header[0xb2] = 0x96;
        file.write_all(&header).unwrap();
        file.set_len(32 * 1024 * 1024).unwrap();
        drop(file);

        let inspection = inspect_file(&path).unwrap();
        assert_eq!(inspection.system, "GBA");
        assert_eq!(inspection.game_code.as_deref(), Some("ABCD"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn ds_header_requires_arm9_region_to_fit_file() {
        let mut nds = vec![0; 0x300];
        nds[0x20..0x24].copy_from_slice(&0x200u32.to_le_bytes());
        nds[0x2c..0x30].copy_from_slice(&0x100u32.to_le_bytes());
        nds[0x84..0x88].copy_from_slice(&0x160u32.to_le_bytes());
        nds[0..4].copy_from_slice(b"HOME");
        nds[0x0c..0x10].copy_from_slice(b"ABCD");
        let inspection = inspect_bytes("fixture.nds", &nds).unwrap();
        assert_eq!(inspection.system, "DS");
        assert_eq!(inspection.title.as_deref(), Some("HOME"));
        assert_eq!(inspection.game_code.as_deref(), Some("ABCD"));
        assert_eq!(inspection.header_checksum_valid, None);

        nds[0x0c..0x10].copy_from_slice(b"A?CD");
        let inspection = inspect_bytes("fixture.nds", &nds).unwrap();
        assert_eq!(inspection.system, "DS");
        assert_eq!(inspection.game_code, None);

        nds[0x2c..0x30].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(inspect_bytes("truncated.nds", &nds).is_none());
    }
}
