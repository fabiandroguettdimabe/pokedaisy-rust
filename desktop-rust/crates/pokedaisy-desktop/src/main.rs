mod domain;
mod identity;
mod import;
mod inspect;
mod library;
mod migration;
#[cfg(target_os = "windows")]
mod player_pipe;
mod probe;
mod probe_pe;
mod protocol;
mod reference;
mod settings;
#[cfg(target_os = "windows")]
mod shell;
mod timing;
#[allow(dead_code)] // Wired into the process bridge in the remaining T10 work.
mod transport;
mod vault;
use fontdue::Font;
use sdl2::audio::AudioSpecDesired;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::{Point, Rect};
use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};
use settings::Settings;
use std::env;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use vyntra_libretro::{
    CorePaths, JoypadButton, JoypadState, LibretroCore, PixelFormat, VideoFrame,
};

const BG: Color = Color::RGB(11, 11, 13);
const INK: Color = Color::RGB(230, 230, 233);
const MUTED: Color = Color::RGB(165, 165, 175);
const GREEN: Color = Color::RGB(179, 18, 49);
const AQUA: Color = Color::RGB(230, 230, 233);
const PAPER: Color = Color::RGB(23, 23, 27);
const BORDER: Color = Color::RGB(57, 52, 60);
const SIDEBAR: Color = Color::RGB(0, 0, 0);
const DARK_CRIMSON: Color = Color::RGB(58, 10, 18);
const MAX_BACKUP_VERSIONS: usize = 16;
const MAX_BACKUP_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone)]
struct Game {
    path: PathBuf,
    title: String,
    system: &'static str,
    storage_key: std::sync::OnceLock<String>,
    rom_sha256: std::sync::OnceLock<String>,
}

#[cfg(test)]
mod migration_contract_tests {
    use super::migration::{MigrationItem, MigrationStatus, apply, preview};
    use std::fs;

    #[test]
    fn migration_preview_is_read_only_and_apply_is_idempotent() {
        let root =
            std::env::temp_dir().join(format!("vyntra-migration-red-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("legacy/game.sav");
        let destination = root.join("new/by-id/game/battery.sav");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"save-data").unwrap();
        let item = MigrationItem::new(
            source.clone(),
            destination.clone(),
            "rom-id".into(),
            "battery".into(),
        );
        let report = preview(&[item.clone()]).unwrap();
        assert!(!destination.exists());
        assert_eq!(report.items[0].status, MigrationStatus::Ready);
        apply(&root.join("journal.json"), &report).unwrap();
        apply(&root.join("journal.json"), &report).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"save-data");
        assert!(source.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_recovers_when_destination_committed_before_journal_update() {
        let root =
            std::env::temp_dir().join(format!("vyntra-migration-recover-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("legacy/game.sav");
        let destination = root.join("new/by-id/game/battery.sav");
        let journal = root.join("migration.json");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"committed-before-journal").unwrap();
        let report = preview(&[MigrationItem::new(
            source.clone(),
            destination.clone(),
            "rom-id".into(),
            "battery".into(),
        )])
        .unwrap();
        assert_eq!(report.items[0].status, MigrationStatus::Ready);

        // Simulate process loss after the destination rename but before the
        // journal could record Committed.
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(&source, &destination).unwrap();
        fs::write(&journal, serde_json::to_vec(&report).unwrap()).unwrap();
        apply(&journal, &report).unwrap();

        let recovered: super::migration::MigrationReport =
            serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
        assert_eq!(recovered.items[0].status, MigrationStatus::Committed);
        assert_eq!(fs::read(&destination).unwrap(), b"committed-before-journal");
        assert!(source.is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_owner_uses_captured_session_after_library_selection_changes() {
        let first = super::game_from_path(std::path::PathBuf::from("first.gb"));
        let second = super::game_from_path(std::path::PathBuf::from("second.gb"));
        let active = Some(first.clone());
        let games = vec![second];
        let selected = 0;
        assert_eq!(super::session_save_owner(&active).unwrap().path, first.path);
        assert_ne!(
            super::session_save_owner(&active).unwrap().path,
            games[selected].path
        );
    }

    #[test]
    fn stable_game_id_resolves_after_library_order_changes() {
        let first_path = std::env::temp_dir().join("vyntra-id-first.gb");
        let second_path = std::env::temp_dir().join("vyntra-id-second.gb");
        std::fs::write(&first_path, b"first-rom").unwrap();
        std::fs::write(&second_path, b"second-rom").unwrap();
        let games = vec![
            super::game_from_path(second_path.clone()),
            super::game_from_path(first_path.clone()),
        ];
        let id = super::game_storage_key(&super::game_from_path(first_path.clone()));
        assert_eq!(
            super::resolve_game_by_storage_id(&games, &id).unwrap().path,
            first_path
        );
        let _ = std::fs::remove_file(first_path);
        let _ = std::fs::remove_file(second_path);
    }

    #[test]
    fn homonymous_roms_keep_separate_save_paths_and_session_owner() {
        let root = std::env::temp_dir().join(format!("vyntra-homonyms-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let first_path = root.join("first/Same.gb");
        let second_path = root.join("second/Same.gb");
        std::fs::create_dir_all(first_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(second_path.parent().unwrap()).unwrap();
        std::fs::write(&first_path, b"first ROM content").unwrap();
        std::fs::write(&second_path, b"second ROM content").unwrap();

        let first = super::game_from_path(first_path.clone());
        let second = super::game_from_path(second_path.clone());
        let first_save = super::preferred_battery_path(&first, &root.join("saves"));
        let second_save = super::preferred_battery_path(&second, &root.join("saves"));
        assert_ne!(
            super::game_storage_key(&first),
            super::game_storage_key(&second)
        );
        assert_ne!(first_save, second_save);

        let active_session = Some(first.clone());
        let library_selection = second;
        assert_eq!(
            super::session_save_owner(&active_session).unwrap().path,
            first_path
        );
        assert_ne!(
            super::session_save_owner(&active_session).unwrap().path,
            library_selection.path
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_sram_is_not_assigned_to_homonymous_different_roms() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-legacy-sram-ambiguity-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let first_path = root.join("first/Same.gb");
        let second_path = root.join("second/Same.gb");
        std::fs::create_dir_all(first_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(second_path.parent().unwrap()).unwrap();
        std::fs::write(&first_path, b"first ROM content").unwrap();
        std::fs::write(&second_path, b"second ROM content").unwrap();
        let first = super::game_from_path(first_path.clone());
        let second = super::game_from_path(second_path);

        assert!(super::legacy_battery_is_unambiguous(
            &first,
            std::slice::from_ref(&first)
        ));
        assert!(!super::legacy_battery_is_unambiguous(
            &first,
            &[first.clone(), second]
        ));
        let identical_path = root.join("backup/Same.gb");
        std::fs::create_dir_all(identical_path.parent().unwrap()).unwrap();
        std::fs::write(&identical_path, b"first ROM content").unwrap();
        let identical = super::game_from_path(identical_path);
        assert!(super::legacy_battery_is_unambiguous(
            &first,
            &[first.clone(), identical]
        ));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unique_legacy_sram_remains_readable_and_ambiguous_one_is_rejected() {
        let root =
            std::env::temp_dir().join(format!("vyntra-legacy-sram-read-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let first_path = root.join("roms/one/Same.gb");
        let second_path = root.join("roms/two/Same.gb");
        std::fs::create_dir_all(first_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(second_path.parent().unwrap()).unwrap();
        std::fs::write(&first_path, b"first ROM content").unwrap();
        std::fs::write(&second_path, b"second ROM content").unwrap();
        let first = super::game_from_path(first_path);
        let second = super::game_from_path(second_path);
        let save_dir = root.join("saves");
        let legacy_path = super::legacy_battery_path(&first, &save_dir);
        std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_path, b"legacy SRAM bytes").unwrap();

        assert_eq!(
            super::read_legacy_battery(&first, std::slice::from_ref(&first), &save_dir)
                .unwrap()
                .as_deref(),
            Some(b"legacy SRAM bytes".as_slice())
        );
        assert!(super::read_legacy_battery(&first, &[first.clone(), second], &save_dir).is_err());
        assert_eq!(std::fs::read(&legacy_path).unwrap(), b"legacy SRAM bytes");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn migration_preview_marks_homonymous_destinations_as_conflicts() {
        let root = std::env::temp_dir().join(format!("vyntra-conflict-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let a = root.join("a.sav");
        let b = root.join("b.sav");
        std::fs::write(&a, b"save-a").unwrap();
        std::fs::write(&b, b"save-b").unwrap();
        let dest = root.join("by-id/game/battery.sav");
        let report = super::migration::preview(&[
            super::migration::MigrationItem::new(a, dest.clone(), "g1".into(), "battery".into()),
            super::migration::MigrationItem::new(b, dest, "g2".into(), "battery".into()),
        ])
        .unwrap();
        assert!(
            report
                .items
                .iter()
                .all(|item| item.status == super::migration::MigrationStatus::Conflict)
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rom_inspection_rejects_truncated_and_false_extension_files() {
        assert!(super::inspect::inspect_bytes("bad.gb", b"not a rom").is_none());
        assert!(super::inspect::inspect_bytes("short.nds", &[0; 128]).is_none());
    }

    #[test]
    fn gba_inspection_extracts_header_identity_only_when_structure_is_valid() {
        let mut bytes = vec![0u8; 0xC0];
        bytes[0xB2] = 0x96;
        bytes[0xA0..0xAC].copy_from_slice(b"TEST ROM    ");
        bytes[0xAC..0xB0].copy_from_slice(b"ABCD");
        bytes[0xBC] = 0x19;
        let info = super::inspect::inspect_bytes("game.gba", &bytes).unwrap();
        assert_eq!(info.system, "GBA");
        assert_eq!(info.game_code.as_deref(), Some("ABCD"));
        assert_eq!(info.title.as_deref(), Some("TEST ROM"));
        assert!(super::inspect::inspect_bytes("game.gb", &bytes).is_none());
    }

    #[test]
    fn gba_checksum_is_reported_without_rejecting_homebrew_headers() {
        let mut bytes = vec![0u8; 0xc0];
        bytes[0xb2] = 0x96;
        bytes[0xac..0xb0].copy_from_slice(b"HACK");
        bytes[0xbc] = 1;
        bytes[0xbd] = super::inspect::gba_header_checksum(&bytes);
        assert_eq!(
            super::inspect::inspect_bytes("homebrew.gba", &bytes)
                .unwrap()
                .header_checksum_valid,
            Some(true)
        );

        bytes[0xbd] ^= 1;
        let damaged = super::inspect::inspect_bytes("homebrew.gba", &bytes).unwrap();
        assert_eq!(damaged.header_checksum_valid, Some(false));
    }

    #[test]
    fn changed_rom_is_rejected_against_the_catalog_content_identity() {
        let path =
            std::env::temp_dir().join(format!("vyntra-rom-change-{}.gba", std::process::id()));
        std::fs::write(&path, b"original").unwrap();
        let game = super::game_from_path(path.clone());
        let _published_id = super::game_storage_key(&game);
        std::fs::write(&path, b"changed").unwrap();
        assert!(super::validate_game_identity(&game).is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rom_identity_validation_checks_the_full_hash_after_storage_key_prefix_matches() {
        let path =
            std::env::temp_dir().join(format!("vyntra-rom-prefix-{}.gb", std::process::id()));
        std::fs::write(&path, b"original ROM").unwrap();
        let game = super::game_from_path(path.clone());
        let current = super::identity::sha256_file(&path).unwrap();
        let mut forged_expected = current.clone().into_bytes();
        let last = forged_expected.len() - 1;
        forged_expected[last] = if forged_expected[last] == b'0' {
            b'1'
        } else {
            b'0'
        };
        game.storage_key.set(current[..16].to_string()).unwrap();
        game.rom_sha256
            .set(String::from_utf8(forged_expected).unwrap())
            .unwrap();

        assert!(current.starts_with(&super::game_storage_key(&game)));
        assert!(super::validate_game_identity(&game).is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migration_journal_accepts_a_new_preview_after_prior_items_finish() {
        let root = std::env::temp_dir().join(format!("vyntra-journal-next-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let journal = root.join("journal.json");
        for name in ["one", "two"] {
            let source = root.join(format!("{name}.sav"));
            let destination = root.join(format!("{name}/battery.sav"));
            std::fs::write(&source, name.as_bytes()).unwrap();
            let item = super::migration::MigrationItem::new(
                source,
                destination,
                name.into(),
                "battery".into(),
            );
            let report = super::migration::preview(&[item]).unwrap();
            super::migration::apply(&journal, &report).unwrap();
        }
        assert!(root.join("two/battery.sav").is_file());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn save_state_manifest_rejects_a_different_core_or_rom() {
        let manifest = super::domain::SaveManifest {
            save_id: "game-state".into(),
            kind: super::domain::SaveKind::SaveState,
            rom_sha256: Some("rom-hash".into()),
            core_hash: Some("core-hash".into()),
            state_format: Some("libretro-serialize-v1".into()),
            slot: Some(1),
            bytes: Some(5),
            sha256: Some(super::identity::sha256_bytes(b"state")),
            parent_id: None,
            created_at_utc: None,
            created_at_unix: None,
            status: "committed".into(),
        };
        assert!(super::vault::validate_state(&manifest, b"state", "rom-hash", "core-hash").is_ok());
        assert!(
            super::vault::validate_state(&manifest, b"state", "other-rom", "core-hash").is_err()
        );
        assert!(
            super::vault::validate_state(&manifest, b"state", "rom-hash", "other-core").is_err()
        );
    }

    #[test]
    fn pe_probe_reports_architecture_and_missing_libretro_exports() {
        let mut pe = vec![0u8; 0x600];
        pe[0..2].copy_from_slice(b"MZ");
        pe[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        pe[0x80..0x84].copy_from_slice(b"PE\0\0");
        pe[0x84..0x86].copy_from_slice(&0x8664u16.to_le_bytes());
        pe[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
        pe[0x94..0x96].copy_from_slice(&0xF0u16.to_le_bytes());
        pe[0x98..0x9A].copy_from_slice(&0x20Bu16.to_le_bytes());
        pe[0x98 + 108..0x98 + 112].copy_from_slice(&16u32.to_le_bytes());
        pe[0x98 + 112..0x98 + 116].copy_from_slice(&0x1000u32.to_le_bytes());
        let section = 0x98 + 0xF0;
        pe[section + 8..section + 12].copy_from_slice(&0x400u32.to_le_bytes());
        pe[section + 12..section + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        pe[section + 16..section + 20].copy_from_slice(&0x400u32.to_le_bytes());
        pe[section + 20..section + 24].copy_from_slice(&0x200u32.to_le_bytes());
        pe[0x200 + 24..0x200 + 28].copy_from_slice(&1u32.to_le_bytes());
        pe[0x200 + 32..0x200 + 36].copy_from_slice(&0x1040u32.to_le_bytes());
        pe[0x240..0x244].copy_from_slice(&0x1050u32.to_le_bytes());
        pe[0x250..0x250 + 18].copy_from_slice(b"retro_api_version\0");
        let report = super::probe_pe::inspect_bytes(&pe).unwrap();
        assert_eq!(report.architecture, "x86_64");
        assert!(
            report
                .exports
                .iter()
                .any(|name| name == "retro_api_version")
        );
        assert!(
            report
                .missing_exports
                .contains(&"retro_get_system_info".to_string())
        );
        assert!(super::probe::extensions_match("GBA", Some("gba|bin")));
        assert!(!super::probe::extensions_match("DS", Some("gba|bin")));
    }
}

#[derive(Clone, Copy)]
enum Screen {
    Library,
    Tools,
    Settings,
    Playing,
}

fn main() -> Result<(), String> {
    if env::args().nth(1).as_deref() == Some("--probe-core") {
        return probe::run_child();
    }
    #[cfg(target_os = "windows")]
    if !env::args().any(|s| s == "--emulator" || s == "--sdl") {
        return shell::run();
    }
    sdl_main()
}

fn sdl_main() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut root = env::current_dir().map_err(|e| e.to_string())?;
    let mut rom_arg = None;
    let mut pipe_name = None;
    let mut pipe_nonce = None;
    let mut pipe_session = None;
    let mut pipe_epoch = None;
    let mut arg = 0;
    while arg < args.len() {
        match args[arg].as_str() {
            "--data" => {
                if let Some(value) = args.get(arg + 1) {
                    root = PathBuf::from(value);
                }
                arg += 2;
            }
            "--pipe" => {
                pipe_name = args.get(arg + 1).cloned();
                arg += 2;
            }
            "--nonce" => {
                pipe_nonce = args.get(arg + 1).cloned();
                arg += 2;
            }
            "--session-id" => {
                pipe_session = args.get(arg + 1).cloned();
                arg += 2;
            }
            "--session-epoch" => {
                pipe_epoch = args
                    .get(arg + 1)
                    .and_then(|value| value.parse::<u64>().ok());
                arg += 2;
            }
            _ => {
                let path = PathBuf::from(&args[arg]);
                if supported(&path) && path.exists() {
                    rom_arg = Some(path);
                }
                arg += 1;
            }
        }
    }
    #[cfg(target_os = "windows")]
    let mut player_pipe = match (pipe_name, pipe_nonce, pipe_session, pipe_epoch) {
        (Some(name), Some(nonce), Some(session), Some(epoch)) => {
            Some(player_pipe::connect(&name, &nonce, &session, epoch)?)
        }
        (None, None, None, None) => None,
        _ => return Err("Contexto de named pipe Player incompleto".into()),
    };
    #[cfg(not(target_os = "windows"))]
    let _ = (pipe_name, pipe_nonce, pipe_session, pipe_epoch);
    let data = root.join("data");
    fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    let config_path = data.join("vyntra-settings.json");
    let mut config = Settings::load(&config_path, &root)?;
    let mut rom_dir = config.library.clone();
    let mut save_dir = config.saves.clone();
    let mut state_dir = config.states.clone();
    let mut system_dir = config.system.clone();
    for dir in [&rom_dir, &save_dir, &state_dir, &system_dir] {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut settings_tab = 0usize;
    let mut settings_row = 0usize;
    let mut settings_message =
        "Selecciona una opción. Las rutas nuevas no trasladan partidas.".to_owned();
    let mut capture_key = None;
    let mut settings_action: Option<i32> = None;
    let core_status =
        [("GB/GBC", "GBC"), ("GBA", "GBA"), ("Nintendo DS", "DS")].map(|(label, system)| {
            let status = find_core(system)
                .map(|path| {
                    format!(
                        "Archivo detectado: {} · sin probar",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    )
                })
                .unwrap_or_else(|_| "No instalado".to_owned());
            (label, status)
        });

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let audio_subsystem = sdl.audio()?;
    let audio_device = audio_subsystem.open_queue::<i16, _>(
        None,
        &AudioSpecDesired {
            freq: Some(32_768),
            channels: Some(2),
            samples: Some(1024),
        },
    )?;
    audio_device.resume();
    let controller_subsystem = sdl.game_controller()?;
    let mut game_controller = if controller_subsystem.num_joysticks()? > 0 {
        controller_subsystem.open(0).ok()
    } else {
        None
    };
    let display = video.desktop_display_mode(0)?;
    let window_w = ((display.w as f32 * 0.60) as u32).clamp(820, 1120);
    let window_h = ((display.h as f32 * 0.58) as u32).clamp(520, 740);
    let window = video
        .window("Vyntra Desktop", window_w.max(1000), window_h.max(660))
        .position_centered()
        .resizable()
        .allow_highdpi()
        .build()
        .map_err(|e| e.to_string())?;
    let mut canvas = window
        .into_canvas()
        .accelerated()
        .build()
        .map_err(|e| e.to_string())?;
    canvas
        .window_mut()
        .set_minimum_size(1000, 660)
        .map_err(|e| e.to_string())?;
    canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
    if config.fullscreen {
        canvas
            .window_mut()
            .set_fullscreen(sdl2::video::FullscreenType::Desktop)?;
    }
    let texture_creator = canvas.texture_creator();
    let pixel_font = Font::from_bytes(
        include_bytes!("../../../assets/AtkinsonHyperlegible-Bold.ttf") as &[u8],
        fontdue::FontSettings::default(),
    )
    .map_err(|e| e.to_string())?;
    let body_font = Font::from_bytes(
        include_bytes!("../../../assets/AtkinsonHyperlegible-Regular.ttf") as &[u8],
        fontdue::FontSettings::default(),
    )
    .map_err(|e| e.to_string())?;
    let strong_font = Font::from_bytes(
        include_bytes!("../../../assets/AtkinsonHyperlegible-Bold.ttf") as &[u8],
        fontdue::FontSettings::default(),
    )
    .map_err(|e| e.to_string())?;
    let mut event_pump = sdl.event_pump()?;
    let mut games = scan_games(&rom_dir, config.recursive);
    let mut selected = 0usize;
    let mut screen = Screen::Library;
    let mut core: Option<LibretroCore> = None;
    let mut active_game: Option<Game> = None;
    let mut active_core_hash: Option<String> = None;
    let mut last_error = None;
    let mut last_frame = None;
    let mut last_auto = Instant::now();
    let mut last_save = Instant::now();
    let mut last_pipe_snapshot = Instant::now();
    let mut companion_paused = false;
    if let Some(path) = rom_arg {
        if let Some(index) = games.iter().position(|g| g.path == path) {
            selected = index;
        } else {
            games.insert(0, game_from_path(path));
        }
        if let Some(game) = games.get(selected) {
            match open_game(game, &games, &save_dir, &system_dir, &config) {
                Ok(c) => {
                    core = Some(c);
                    active_game = Some(game.clone());
                    active_core_hash = core_hash_for(&config, game.system);
                    screen = Screen::Playing;
                    last_error = None;
                }
                Err(e) => {
                    if args.iter().any(|s| s == "--emulator") {
                        rfd::MessageDialog::new()
                            .set_title("No se pudo iniciar el juego")
                            .set_description(&e)
                            .set_level(rfd::MessageLevel::Error)
                            .show();
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }
        }
    }

    'app: loop {
        #[cfg(target_os = "windows")]
        if let Some(pipe) = player_pipe.as_ref() {
            match pipe.try_command() {
                Ok(Some(transport::PlayerCommand::Stop)) => break 'app,
                Ok(Some(transport::PlayerCommand::SaveState(request_id))) => {
                    let result = (|| -> Result<(), String> {
                        let c = core.as_mut().ok_or("No hay un juego activo")?;
                        let g = session_save_owner(&active_game)
                            .ok_or("La sesión no tiene una ROM activa")?;
                        validate_game_identity(g)?;
                        let core_hash = active_core_hash
                            .as_deref()
                            .ok_or("No se pudo verificar el núcleo activo")?;
                        let bytes = c.serialize().map_err(|error| {
                            format!("No se pudo serializar el estado: {error:?}")
                        })?;
                        backup_verified_state(g, &state_dir, Some(core_hash))?;
                        let path = preferred_state_path(g, &state_dir);
                        atomic_write(&path, &bytes)?;
                        write_save_manifest_checked(
                            g,
                            &state_dir,
                            domain::SaveKind::SaveState,
                            &bytes,
                            Some(1),
                            Some(core_hash),
                            "committed",
                        )
                    })();
                    if let Some(pipe) = player_pipe.as_mut() {
                        let _ = pipe.send_command_result(request_id, "save_state", result);
                    }
                }
                Ok(Some(transport::PlayerCommand::LoadState(request_id))) => {
                    let result = (|| -> Result<(), String> {
                        let c = core.as_mut().ok_or("No hay un juego activo")?;
                        let g = session_save_owner(&active_game)
                            .ok_or("La sesión no tiene una ROM activa")?;
                        let bytes =
                            load_compatible_state(g, &state_dir, active_core_hash.as_deref())?;
                        c.unserialize(&bytes)
                            .map_err(|error| format!("El núcleo rechazó el estado: {error:?}"))
                    })();
                    if let Some(pipe) = player_pipe.as_mut() {
                        let _ = pipe.send_command_result(request_id, "load_state", result);
                    }
                }
                Ok(Some(transport::PlayerCommand::Pause(paused))) => {
                    companion_paused = paused;
                    if paused {
                        audio_device.pause();
                    } else {
                        audio_device.resume();
                    }
                    last_auto = Instant::now();
                }
                Ok(None) => {}
                Err(_) => break 'app,
            }
        }
        let (logical_w, logical_h) = canvas.window().size();
        canvas
            .set_logical_size(logical_w, logical_h)
            .map_err(|e| e.to_string())?;
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'app,
                Event::DropFile { filename, .. } => {
                    let p = PathBuf::from(filename);
                    if supported(&p) {
                        if let Some(i) = games.iter().position(|g| g.path == p) {
                            selected = i;
                        } else {
                            games.insert(0, game_from_path(p));
                            selected = 0;
                        }
                        if core.is_none() {
                            screen = Screen::Library;
                        }
                    }
                }
                Event::ControllerDeviceAdded { which, .. } if game_controller.is_none() => {
                    game_controller = controller_subsystem.open(which).ok();
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    if game_controller
                        .as_ref()
                        .is_some_and(|c| c.instance_id() == which)
                    {
                        game_controller = None;
                    }
                }
                Event::MouseButtonDown { x, y, clicks, .. }
                    if !matches!(screen, Screen::Playing) =>
                {
                    if x < 176 {
                        screen = match y {
                            116..=174 => Screen::Library,
                            182..=240 => Screen::Tools,
                            248..=306 => Screen::Settings,
                            _ => screen,
                        };
                        continue;
                    }
                    if matches!(screen, Screen::Settings) {
                        let (w, h) = canvas.window().size();
                        if (116..160).contains(&y) && x >= 204 && x < w as i32 - 32 {
                            settings_tab = ((x - 204) / ((w - 236) / 4) as i32).min(3) as usize;
                            settings_row = 0;
                        } else if y >= 178 && y < h as i32 - 72 {
                            let visible = ((h.saturating_sub(250)) / 72).max(1) as usize;
                            let row = settings_row / visible * visible + ((y - 178) / 72) as usize;
                            if row < config.rows(settings_tab).len() {
                                settings_row = row;
                                settings_action = Some(1);
                            }
                        }
                        continue;
                    }
                    if !matches!(screen, Screen::Library) {
                        continue;
                    }
                    if y >= canvas.window().size().1 as i32 - 95 {
                        if let Some(error) = last_error.as_deref() {
                            rfd::MessageDialog::new()
                                .set_title("Diagnóstico de Vyntra")
                                .set_description(error)
                                .set_level(rfd::MessageLevel::Error)
                                .show();
                            continue;
                        }
                    }
                    let (w, _) = canvas.window().size();
                    let right = w as i32 - 228;
                    if x >= right && x < w as i32 - 24 && (468..512).contains(&y) {
                        screen = Screen::Settings;
                        settings_tab = 1;
                        settings_row = 0;
                        continue;
                    }
                    if x >= right && x < w as i32 - 24 && (360..404).contains(&y) {
                        if let Some(game) = games.get(selected).cloned() {
                            match open_game(&game, &games, &save_dir, &system_dir, &config) {
                                Ok(c) => {
                                    core = Some(c);
                                    active_game = Some(game.clone());
                                    active_core_hash = core_hash_for(&config, game.system);
                                    screen = Screen::Playing;
                                    last_error = None;
                                }
                                Err(e) => {
                                    last_error = Some(e);
                                }
                            }
                        }
                        continue;
                    }
                    let add_rom_clicked =
                        x >= right && x < w as i32 - 24 && (414..458).contains(&y);
                    if add_rom_clicked {
                        if let Some(path) = pick_rom() {
                            if let Some(i) = games.iter().position(|g| g.path == path) {
                                selected = i;
                            } else {
                                games.insert(0, game_from_path(path));
                                selected = 0;
                            }
                            if clicks >= 1 {
                                if let Some(game) = games.get(selected).cloned() {
                                    match open_game(&game, &games, &save_dir, &system_dir, &config)
                                    {
                                        Ok(c) => {
                                            core = Some(c);
                                            active_game = Some(game.clone());
                                            active_core_hash = core_hash_for(&config, game.system);
                                            screen = Screen::Playing;
                                            last_error = None;
                                        }
                                        Err(e) => last_error = Some(e),
                                    }
                                }
                            }
                        }
                        continue;
                    }
                    let layout = library_grid_metrics(w, canvas.window().size().1);
                    if let Some(index) = layout.index_at(x, y, selected) {
                        if index < games.len() {
                            selected = index;
                            if clicks >= 2 {
                                if let Some(game) = games.get(selected).cloned() {
                                    match open_game(&game, &games, &save_dir, &system_dir, &config)
                                    {
                                        Ok(c) => {
                                            core = Some(c);
                                            screen = Screen::Playing;
                                            last_error = None;
                                        }
                                        Err(e) => last_error = Some(e),
                                    }
                                }
                            }
                        }
                    }
                }
                Event::MouseWheel { y, .. } if matches!(screen, Screen::Settings) => {
                    settings_row = (settings_row as i32 - y)
                        .clamp(0, config.rows(settings_tab).len() as i32 - 1)
                        as usize;
                }
                Event::KeyDown {
                    keycode: Some(key),
                    repeat: false,
                    ..
                } if capture_key.is_some() => {
                    if key != Keycode::Escape {
                        let reserved = [Keycode::Space, Keycode::F2, Keycode::F4, Keycode::F11];
                        if reserved.contains(&key)
                            || config.keys.iter().enumerate().any(|(i, k)| {
                                Some(i) != capture_key && Keycode::from_name(k) == Some(key)
                            })
                        {
                            settings_message =
                                "Tecla reservada o ya asignada. Elige otra; Esc cancela.".into();
                            continue;
                        }
                        let mut next = config.clone();
                        next.keys[capture_key.unwrap()] = key.name();
                        match next.save(&config_path) {
                            Ok(()) => {
                                config = next;
                                settings_message = "Asignación guardada.".into();
                            }
                            Err(e) => settings_message = e,
                        }
                    } else {
                        settings_message = "Asignación cancelada.".into();
                    }
                    capture_key = None;
                }
                Event::MouseWheel { y, .. } if matches!(screen, Screen::Library) => {
                    let (w, h) = canvas.window().size();
                    let step = library_grid_metrics(w, h).columns.max(1);
                    if y < 0 {
                        selected = (selected + step).min(games.len().saturating_sub(1));
                    } else if y > 0 {
                        selected = selected.saturating_sub(step);
                    }
                }
                Event::KeyDown {
                    keycode: Some(key),
                    repeat: false,
                    ..
                } => match screen {
                    Screen::Library => match key {
                        Keycode::Down | Keycode::J => {
                            if !games.is_empty() {
                                selected = (selected + 1).min(games.len() - 1);
                            }
                        }
                        Keycode::Up | Keycode::K => selected = selected.saturating_sub(1),
                        Keycode::Return | Keycode::Space => {
                            if let Some(game) = games.get(selected).cloned() {
                                match open_game(&game, &games, &save_dir, &system_dir, &config) {
                                    Ok(c) => {
                                        core = Some(c);
                                        active_game = Some(game.clone());
                                        active_core_hash = core_hash_for(&config, game.system);
                                        screen = Screen::Playing;
                                        last_error = None;
                                    }
                                    Err(e) => last_error = Some(e),
                                }
                            }
                        }
                        Keycode::F5 => games = scan_games(&rom_dir, config.recursive),
                        Keycode::Escape => break 'app,
                        _ => {}
                    },
                    Screen::Settings => match key {
                        Keycode::Escape => screen = Screen::Library,
                        Keycode::Tab => {
                            settings_tab = (settings_tab + 1) % 4;
                            settings_row = 0;
                        }
                        Keycode::Up => settings_row = settings_row.saturating_sub(1),
                        Keycode::Down => {
                            settings_row =
                                (settings_row + 1).min(config.rows(settings_tab).len() - 1)
                        }
                        Keycode::Return | Keycode::Right | Keycode::Space => {
                            settings_action = Some(1)
                        }
                        Keycode::Left => settings_action = Some(-1),
                        _ => {}
                    },
                    Screen::Tools => match key {
                        Keycode::Escape | Keycode::G | Keycode::Home => screen = Screen::Library,
                        Keycode::Return | Keycode::Space => {
                            if let Some(game) = games.get(selected).cloned() {
                                match open_game(&game, &games, &save_dir, &system_dir, &config) {
                                    Ok(c) => {
                                        core = Some(c);
                                        screen = Screen::Playing;
                                        last_error = None;
                                    }
                                    Err(e) => last_error = Some(e),
                                }
                            }
                        }
                        _ => {}
                    },
                    Screen::Playing => match key {
                        Keycode::Escape => {
                            if let (Some(c), Some(g)) =
                                (core.as_mut(), session_save_owner(&active_game))
                            {
                                save_battery(c, g, &save_dir, active_core_hash.as_deref());
                            }
                            core = None;
                            active_game = None;
                            active_core_hash = None;
                            screen = Screen::Library;
                            last_frame = None;
                        }
                        Keycode::F2 => {
                            if let (Some(c), Some(g)) =
                                (core.as_mut(), session_save_owner(&active_game))
                            {
                                if let Err(error) = validate_game_identity(g) {
                                    last_error = Some(error);
                                    continue 'app;
                                }
                                match c.serialize() {
                                    Ok(bytes) => {
                                        let path = state_path(g, &state_dir);
                                        if let Err(e) = backup_verified_state(
                                            g,
                                            &state_dir,
                                            active_core_hash.as_deref(),
                                        ) {
                                            last_error = Some(format!(
                                                "No se pudo respaldar el estado anterior; se conserva sin reemplazar: {e}"
                                            ));
                                            continue 'app;
                                        }
                                        match atomic_write(&path, &bytes) {
                                            Ok(()) => write_save_manifest(
                                                g,
                                                &state_dir,
                                                domain::SaveKind::SaveState,
                                                &bytes,
                                                Some(1),
                                                active_core_hash.as_deref(),
                                                "committed",
                                            ),
                                            Err(e) => {
                                                last_error =
                                                    Some(format!("No se pudo guardar estado: {e}"))
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        last_error = Some(format!("Estado no disponible: {e:?}"))
                                    }
                                }
                            }
                        }
                        Keycode::F4 => {
                            if let (Some(c), Some(g)) =
                                (core.as_mut(), session_save_owner(&active_game))
                            {
                                match load_compatible_state(
                                    g,
                                    &state_dir,
                                    active_core_hash.as_deref(),
                                ) {
                                    Ok(bytes) => {
                                        if let Err(e) = c.unserialize(&bytes) {
                                            last_error =
                                                Some(format!("No se pudo cargar estado: {e:?}"));
                                        }
                                    }
                                    Err(e) => {
                                        last_error = Some(format!("No se encontró el estado: {e}"))
                                    }
                                }
                            }
                        }
                        Keycode::F11 => {
                            let target = if canvas.window().fullscreen_state()
                                == sdl2::video::FullscreenType::Off
                            {
                                sdl2::video::FullscreenType::Desktop
                            } else {
                                sdl2::video::FullscreenType::Off
                            };
                            if let Err(e) = canvas.window_mut().set_fullscreen(target) {
                                last_error = Some(e);
                            }
                        }
                        _ => {}
                    },
                },
                _ => {}
            }
        }

        if let Some(direction) = settings_action.take() {
            if settings_tab == 3 && settings_row < 14 {
                capture_key = Some(settings_row);
                settings_message =
                    "Pulsa la nueva tecla; Esc cancela. Espacio/F2/F4/F11 están reservadas.".into();
            } else {
                let mut next = config.clone();
                match next.activate(settings_tab, settings_row, direction, &config_path) {
                    Ok(true) => {
                        let fullscreen = if next.fullscreen {
                            sdl2::video::FullscreenType::Desktop
                        } else {
                            sdl2::video::FullscreenType::Off
                        };
                        match canvas
                            .window_mut()
                            .set_fullscreen(fullscreen)
                            .and_then(|_| next.save(&config_path))
                        {
                            Ok(()) => {
                                let rescan = config.library != next.library
                                    || config.recursive != next.recursive;
                                config = next;
                                rom_dir = config.library.clone();
                                save_dir = config.saves.clone();
                                state_dir = config.states.clone();
                                system_dir = config.system.clone();
                                if rescan {
                                    games = scan_games(&rom_dir, config.recursive);
                                    selected = 0;
                                }
                                settings_message =
                                    "Cambios guardados. Los archivos anteriores se conservan."
                                        .into();
                            }
                            Err(e) => settings_message = format!("No se pudo guardar: {e}"),
                        }
                    }
                    Ok(false) => {}
                    Err(e) => settings_message = e,
                }
            }
        }
        let (w, h) = canvas.window().size();
        canvas.set_draw_color(BG);
        canvas.clear();
        match screen {
            Screen::Library => draw_library(
                &mut canvas,
                &pixel_font,
                &body_font,
                &strong_font,
                &games,
                selected,
                w,
                h,
                last_error.as_deref(),
                &config,
            )?,
            Screen::Tools => draw_info_page(
                &mut canvas,
                &pixel_font,
                &body_font,
                &strong_font,
                &core_status,
                w,
                h,
                false,
            )?,
            Screen::Settings => settings::draw(
                &mut canvas,
                &body_font,
                &strong_font,
                &config,
                settings_tab,
                settings_row,
                w,
                h,
                &settings_message,
            )?,
            Screen::Playing => {
                let fast_forward = event_pump
                    .keyboard_state()
                    .is_scancode_pressed(sdl2::keyboard::Scancode::Space);
                let fps = core.as_ref().map(|c| c.av_info().fps).unwrap_or(60.0);
                let frame_interval = timing::frame_interval(fps);
                let frame_interval = if fast_forward {
                    frame_interval.div_f64(config.fast_forward.max(1) as f64)
                } else {
                    frame_interval
                };
                if !companion_paused && last_auto.elapsed() >= frame_interval {
                    if let Some(c) = core.as_mut() {
                        c.set_fast_forward(fast_forward);
                        let mut input = keyboard_joypad_state(&config, |key| {
                            event_pump
                                .keyboard_state()
                                .is_scancode_pressed(pressed_key(key))
                        });
                        if let Some(pad) = game_controller.as_ref() {
                            use sdl2::controller::{Axis, Button};
                            for (button, retro) in [
                                (Button::B, JoypadButton::B),
                                (Button::A, JoypadButton::A),
                                (Button::Y, JoypadButton::Y),
                                (Button::X, JoypadButton::X),
                                (Button::Start, JoypadButton::Start),
                                (Button::Back, JoypadButton::Select),
                                (Button::DPadUp, JoypadButton::Up),
                                (Button::DPadDown, JoypadButton::Down),
                                (Button::DPadLeft, JoypadButton::Left),
                                (Button::DPadRight, JoypadButton::Right),
                                (Button::LeftShoulder, JoypadButton::L),
                                (Button::RightShoulder, JoypadButton::R),
                            ] {
                                if pad.button(button) {
                                    input.set(retro, true);
                                }
                            }
                            if pad.axis(Axis::TriggerLeft) > 16_384 {
                                input.set(JoypadButton::L2, true);
                            }
                            if pad.axis(Axis::TriggerRight) > 16_384 {
                                input.set(JoypadButton::R2, true);
                            }
                        }
                        match c.run_frame(input) {
                            Ok(f) => {
                                last_frame = f.video;
                                if !f.audio.is_empty() {
                                    if fast_forward {
                                        audio_device.clear();
                                    } else {
                                        let target_rate = audio_device.spec().freq.max(1) as u32;
                                        let source_rate = core
                                            .as_ref()
                                            .map(|core| core.av_info().sample_rate)
                                            .unwrap_or(target_rate as f64);
                                        let samples: Vec<i16> = timing::resample_stereo(
                                            &f.audio,
                                            source_rate,
                                            target_rate,
                                        )
                                        .iter()
                                        .map(|s| (*s as i32 * config.volume as i32 / 100) as i16)
                                        .collect();
                                        let queue_limit_bytes =
                                            (target_rate as u64 * 4 * 12 / 100) as u32;
                                        let queued = audio_device.size();
                                        let incoming_bytes = (samples.len() as u64
                                            * std::mem::size_of::<i16>() as u64)
                                            .min(u32::MAX as u64)
                                            as u32;
                                        if queued.saturating_add(incoming_bytes)
                                            <= queue_limit_bytes
                                        {
                                            let _ = audio_device.queue_audio(&samples);
                                        }
                                    }
                                }
                            }
                            Err(e) => last_error = Some(format!("Error del core: {e:?}")),
                        }
                        if let Some(frame) = last_frame.as_ref() {
                            draw_play(&mut canvas, &texture_creator, frame, w, h, config.smooth)?;
                        }
                        if let Some(e) = last_error.as_deref() {
                            text(
                                &mut canvas,
                                &pixel_font,
                                e,
                                40,
                                h as i32 - 38,
                                14,
                                Color::RGB(196, 48, 48),
                            );
                        }
                        if last_save.elapsed() > Duration::from_secs(config.autosave) {
                            if let (Some(g), Some(c)) =
                                (session_save_owner(&active_game).cloned(), core.as_mut())
                            {
                                save_battery(c, &g, &save_dir, active_core_hash.as_deref());
                                last_save = Instant::now();
                            }
                        }
                    }
                    last_auto =
                        timing::next_frame_anchor(last_auto, Instant::now(), frame_interval);
                } else if let Some(frame) = last_frame.as_ref() {
                    draw_play(&mut canvas, &texture_creator, frame, w, h, config.smooth)?;
                }
                draw_play_header(&mut canvas, &pixel_font, w)?;
            }
        }
        canvas.present();
        #[cfg(target_os = "windows")]
        if last_pipe_snapshot.elapsed() >= Duration::from_millis(500) {
            if let Some(pipe) = player_pipe.as_mut() {
                let snapshot = serde_json::json!({
                    "title": active_game.as_ref().map(|game| game.title.as_str()),
                    "system": active_game.as_ref().map(|game| game.system),
                    "screen": if matches!(screen, Screen::Playing) { "playing" } else { "menu" },
                    "paused": companion_paused,
                });
                let _ = pipe.send_snapshot(snapshot);
            }
            last_pipe_snapshot = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(
            if matches!(screen, Screen::Playing) {
                4
            } else {
                16
            },
        ));
    }
    if let Some(c) = core.as_mut() {
        if let Some(g) = session_save_owner(&active_game) {
            save_battery(c, g, &save_dir, active_core_hash.as_deref());
        }
    }
    Ok(())
}

// KeyboardState uses the SDL scancode enum; keep the game mapping in one compact helper.
fn pressed_key(key: Keycode) -> sdl2::keyboard::Scancode {
    sdl2::keyboard::Scancode::from_keycode(key).unwrap_or(sdl2::keyboard::Scancode::A)
}

fn keyboard_joypad_state(
    config: &Settings,
    mut is_pressed: impl FnMut(Keycode) -> bool,
) -> JoypadState {
    let mut input = JoypadState::default();
    for (binding, (default_key, button)) in [
        (Keycode::Z, JoypadButton::A),
        (Keycode::X, JoypadButton::B),
        (Keycode::F, JoypadButton::Y),
        (Keycode::Tab, JoypadButton::X),
        (Keycode::Return, JoypadButton::Start),
        (Keycode::Backspace, JoypadButton::Select),
        (Keycode::Up, JoypadButton::Up),
        (Keycode::Down, JoypadButton::Down),
        (Keycode::Left, JoypadButton::Left),
        (Keycode::Right, JoypadButton::Right),
        (Keycode::Q, JoypadButton::L),
        (Keycode::E, JoypadButton::R),
        (Keycode::Num1, JoypadButton::L2),
        (Keycode::Num2, JoypadButton::R2),
    ]
    .into_iter()
    .enumerate()
    {
        let key = config
            .keys
            .get(binding)
            .and_then(|name| Keycode::from_name(name))
            .unwrap_or(default_key);
        if is_pressed(key) {
            input.set(button, true);
        }
    }
    input
}

fn game_storage_key(game: &Game) -> String {
    game.storage_key
        .get_or_init(|| match identity::sha256_file(&game.path) {
            Ok(hash) => {
                let _ = game.rom_sha256.set(hash.clone());
                hash[..16].to_string()
            }
            Err(_) => identity::storage_key(&game.path),
        })
        .clone()
}

fn session_save_owner(active: &Option<Game>) -> Option<&Game> {
    active.as_ref()
}

fn resolve_game_by_source<'a>(
    games: &'a [Game],
    id: &str,
    source_index: usize,
) -> Option<&'a Game> {
    games
        .get(source_index)
        .filter(|game| game_storage_key(game) == id)
}

#[cfg(test)]
fn resolve_game_by_storage_id<'a>(games: &'a [Game], id: &str) -> Option<&'a Game> {
    games.iter().find(|game| game_storage_key(game) == id)
}

fn validate_game_identity(game: &Game) -> Result<(), String> {
    let _ = game_storage_key(game);
    let current = identity::sha256_file(&game.path)
        .map_err(|_| "No se pudo verificar la ROM. Vuelve a escanear la biblioteca.".to_string())?;
    if game
        .rom_sha256
        .get()
        .is_some_and(|expected| expected == &current)
    {
        Ok(())
    } else {
        Err(
            "La ROM cambió desde que se mostró la biblioteca. Vuelve a escanear antes de jugar."
                .into(),
        )
    }
}

fn legacy_battery_path(game: &Game, dir: &Path) -> PathBuf {
    dir.join(format!(
        "{}.sav",
        game.path.file_name().unwrap_or_default().to_string_lossy()
    ))
}

fn preferred_battery_path(game: &Game, dir: &Path) -> PathBuf {
    dir.join("by-id")
        .join(game_storage_key(game))
        .join("battery.sav")
}

fn legacy_state_path(game: &Game, dir: &Path) -> PathBuf {
    dir.join(format!(
        "{}.ss",
        game.path.file_name().unwrap_or_default().to_string_lossy()
    ))
}

fn preferred_state_path(game: &Game, dir: &Path) -> PathBuf {
    dir.join("by-id")
        .join(game_storage_key(game))
        .join("slot1.ss")
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    atomic_write_with(path, data, |file, bytes| {
        file.write_all(bytes)?;
        file.sync_all()
    })
}

fn safe_log_file_name(path: &Path) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let sanitized = name
        .chars()
        .take(128)
        .map(|character| {
            if character.is_control() {
                '\u{fffd}'
            } else {
                character
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "archivo".into()
    } else {
        sanitized
    }
}

fn player_file_error(action: &str, path: &Path) -> String {
    format!(
        "No se pudo {action} «{}». Comprueba que el archivo exista y sea compatible.",
        safe_log_file_name(path)
    )
}

fn atomic_write_with(
    path: &Path,
    data: &[u8],
    stage: impl FnOnce(&mut fs::File, &[u8]) -> std::io::Result<()>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp");
    let staged = (|| -> Result<(), String> {
        let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        stage(&mut f, data).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(error) = staged {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    if let Err(error) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(error.to_string());
    }
    Ok(())
}

fn open_game(
    game: &Game,
    games: &[Game],
    saves: &Path,
    system: &Path,
    config: &Settings,
) -> Result<LibretroCore, String> {
    let core_path = config.core(game.system)?;
    let mut core = LibretroCore::load(
        &core_path,
        CorePaths {
            save_dir: saves.to_path_buf(),
            system_dir: system.to_path_buf(),
            panel_width: 1280,
            panel_height: 760,
        },
    )
    .map_err(|error| {
        eprintln!(
            "Falló la carga del núcleo {}: {error:?}",
            safe_log_file_name(&core_path)
        );
        player_file_error("iniciar el núcleo", &core_path)
    })?;
    core.load_game(&game.path).map_err(|error| {
        eprintln!(
            "Falló la apertura de la ROM {}: {error:?}",
            safe_log_file_name(&game.path)
        );
        player_file_error("abrir la ROM", &game.path)
    })?;
    let preferred = preferred_battery_path(game, saves);
    let save_bytes = if preferred.is_file() {
        match load_compatible_battery(game, saves, &core_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!(
                    "No se pudo validar/restaurar SRAM para {}: {error}",
                    safe_log_file_name(&game.path)
                );
                None
            }
        }
    } else {
        match read_legacy_battery(game, games, saves) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!(
                    "No se pudo cargar SRAM legacy para {}: {error}",
                    safe_log_file_name(&game.path)
                );
                None
            }
        }
    };
    if let Some(bytes) = save_bytes {
        if let Err(error) = core.write_save_ram(&bytes) {
            eprintln!(
                "El núcleo rechazó la SRAM de {}: {error:?}",
                safe_log_file_name(&game.path)
            );
        }
    }
    Ok(core)
}

fn legacy_battery_is_unambiguous(game: &Game, games: &[Game]) -> bool {
    let Some(file_name) = game.path.file_name() else {
        return false;
    };
    let Ok(game_hash) = identity::sha256_file(&game.path) else {
        return false;
    };
    let mut found_matching_name = false;
    for candidate in games
        .iter()
        .filter(|candidate| candidate.path.file_name() == Some(file_name))
    {
        found_matching_name = true;
        if !identity::sha256_file(&candidate.path)
            .is_ok_and(|candidate_hash| candidate_hash == game_hash)
        {
            return false;
        }
    }
    found_matching_name
}

fn read_legacy_battery(
    game: &Game,
    games: &[Game],
    saves: &Path,
) -> Result<Option<Vec<u8>>, String> {
    let path = legacy_battery_path(game, saves);
    if !path.is_file() {
        return Ok(None);
    }
    if !legacy_battery_is_unambiguous(game, games) {
        return Err("El nombre de archivo coincide con varias identidades de ROM".into());
    }
    fs::read(path)
        .map(Some)
        .map_err(|error| format!("No se pudo leer el archivo: {error}"))
}

fn load_compatible_battery(
    game: &Game,
    dir: &Path,
    core_path: &Path,
) -> Result<Option<Vec<u8>>, String> {
    let rom_hash = identity::sha256_file(&game.path).map_err(|e| e.to_string())?;
    let core_hash = identity::sha256_file(core_path).map_err(|e| e.to_string())?;
    let path = preferred_battery_path(game, dir);
    let manifest_path = path
        .parent()
        .ok_or("Ruta de SRAM inválida")?
        .join("manifest.json");
    let mut candidates = vec![(path.clone(), manifest_path)];
    let backup_root = path
        .parent()
        .ok_or("Ruta de SRAM inválida")?
        .join("backups")
        .join("battery");
    if let Ok(entries) = fs::read_dir(&backup_root) {
        let mut backups: Vec<_> = entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.path())
            .collect();
        backups.sort();
        backups.reverse();
        candidates.extend(
            backups
                .into_iter()
                .take(32)
                .map(|backup| (backup.join("battery.sav"), backup.join("manifest.json"))),
        );
    }
    let mut last_error = None;
    for (candidate, manifest_path) in candidates {
        if !candidate.is_file() {
            continue;
        }
        match validate_battery_files(&candidate, &manifest_path, &rom_hash, &core_hash) {
            Ok(bytes) => return Ok(Some(bytes)),
            Err(error) => last_error = Some(error),
        }
    }
    if let Some(error) = last_error {
        Err(error)
    } else {
        Ok(None)
    }
}

fn validate_battery_files(
    save_path: &Path,
    manifest_path: &Path,
    rom_hash: &str,
    core_hash: &str,
) -> Result<Vec<u8>, String> {
    let bytes = fs::read(save_path).map_err(|e| format!("No se pudo leer SRAM: {e}"))?;
    let manifest: domain::SaveManifest = serde_json::from_slice(
        &fs::read(manifest_path).map_err(|_| "La SRAM no tiene un manifiesto verificable")?,
    )
    .map_err(|_| "El manifiesto de SRAM está dañado")?;
    vault::validate_battery(&manifest, &bytes, rom_hash, core_hash)?;
    Ok(bytes)
}

fn find_core(system: &str) -> Result<PathBuf, String> {
    let mut dirs = vec![env::current_dir().map_err(|e| e.to_string())?.join("cores")];
    if let Some(dir) = env::var_os("POKEDAISY_CORE_DIR") {
        if cfg!(debug_assertions) && env::var_os("VYNTRA_CORE_DIR").is_none() {
            eprintln!("POKEDAISY_CORE_DIR está obsoleta; usa VYNTRA_CORE_DIR.");
        }
        dirs.insert(0, PathBuf::from(dir));
    }
    if let Some(dir) = env::var_os("VYNTRA_CORE_DIR") {
        dirs.insert(0, PathBuf::from(dir));
    }
    find_core_in_dirs(&dirs, system).ok_or_else(|| {
        format!(
            "No se encontró un núcleo Vyntra compatible para {system}. Coloca un núcleo Vyntra en ./cores o define VYNTRA_CORE_DIR. Los núcleos estándar de RetroArch no se consideran compatibles sin validar su interfaz."
        )
    })
}

fn find_core_in_dirs(dirs: &[PathBuf], system: &str) -> Option<PathBuf> {
    dirs.iter()
        .flat_map(|dir| core_names(system).iter().map(move |name| dir.join(name)))
        .find(|path| path.is_file())
}

fn core_names(system: &str) -> &'static [&'static str] {
    match system {
        "GBA" => &[
            "vyntra_gba_libretro.dll",
            "pokedaisy_gba_libretro.dll",
            "vyntra_gba_libretro.so",
            "pokedaisy_gba_libretro.so",
        ],
        "DS" => &[
            "vyntra_ds_libretro.dll",
            "pokedaisy_ds_libretro.dll",
            "vyntra_ds_libretro.so",
            "pokedaisy_ds_libretro.so",
        ],
        _ => &[
            "vyntra_gbc_libretro.dll",
            "pokedaisy_gbc_libretro.dll",
            "vyntra_gbc_libretro.so",
            "pokedaisy_gbc_libretro.so",
        ],
    }
}

fn scan_games(root: &Path, recursive: bool) -> Vec<Game> {
    scan_games_controlled(root, recursive, &AtomicBool::new(false))
}

fn scan_games_controlled(root: &Path, recursive: bool, cancelled: &AtomicBool) -> Vec<Game> {
    scan_games_controlled_with(root, recursive, cancelled, |_| {})
}

fn scan_games_controlled_with(
    root: &Path,
    recursive: bool,
    cancelled: &AtomicBool,
    mut on_entry: impl FnMut(&Path),
) -> Vec<Game> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop().filter(|_| !cancelled.load(Ordering::Acquire)) {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if cancelled.load(Ordering::Acquire) {
                return Vec::new();
            }
            let p = entry.path();
            if p.is_symlink() {
            } else if p.is_dir() {
                if recursive {
                    stack.push(p.clone());
                }
            } else if supported(&p) {
                out.push(game_from_path(p.clone()));
            }
            on_entry(&p);
            if cancelled.load(Ordering::Acquire) {
                return Vec::new();
            }
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}
fn supported(p: &Path) -> bool {
    let extension_supported = p.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        matches!(
            s.to_ascii_lowercase().as_str(),
            "gb" | "gbc" | "gba" | "nds"
        )
    });
    extension_supported && (!p.is_file() || inspect::inspect_file(p).is_some())
}
fn pick_rom() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Selecciona una ROM")
        .add_filter(
            "Game Boy / Game Boy Color / GBA / Nintendo DS",
            &["gb", "gbc", "gba", "nds"],
        )
        .pick_file()
}
fn game_from_path(path: PathBuf) -> Game {
    let inspection = inspect::inspect_file(&path);
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let extension_system = match ext.as_str() {
        "gba" => "GBA",
        "nds" => "DS",
        "gb" => "GB",
        "gbc" => "GBC",
        _ => "ROM",
    };
    let system = inspection
        .as_ref()
        .map_or(extension_system, |info| info.system);
    let title = inspection
        .as_ref()
        .and_then(|info| info.title.clone())
        .unwrap_or_else(|| {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .replace(['_', '-'], " ")
        });
    Game {
        path,
        title,
        system,
        storage_key: std::sync::OnceLock::new(),
        rom_sha256: std::sync::OnceLock::new(),
    }
}
fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn core_hash_for(config: &Settings, system: &str) -> Option<String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CORE_HASHES: OnceLock<Mutex<HashMap<PathBuf, String>>> = OnceLock::new();
    let path = config.core(system).ok()?;
    let cache = CORE_HASHES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock() {
        if let Some(hash) = map.get(&path) {
            return Some(hash.clone());
        }
    }
    let hash = identity::sha256_file(&path).ok()?;
    if let Ok(mut map) = cache.lock() {
        map.insert(path, hash.clone());
    }
    Some(hash)
}

fn write_save_manifest(
    game: &Game,
    dir: &Path,
    kind: domain::SaveKind,
    bytes: &[u8],
    slot: Option<u32>,
    core_hash: Option<&str>,
    status: &str,
) {
    if let Err(error) = write_save_manifest_checked(game, dir, kind, bytes, slot, core_hash, status)
    {
        eprintln!("No se pudo escribir manifiesto de guardado: {error}");
    }
}

fn write_save_manifest_checked(
    game: &Game,
    dir: &Path,
    kind: domain::SaveKind,
    bytes: &[u8],
    slot: Option<u32>,
    core_hash: Option<&str>,
    status: &str,
) -> Result<(), String> {
    validate_game_identity(game)?;
    let key = game_storage_key(game);
    let rom_hash = identity::sha256_file(&game.path)
        .map_err(|error| format!("No se pudo verificar ROM para el manifiesto: {error}"))?;
    let manifest = domain::SaveManifest {
        save_id: format!("{key}-{:?}", kind).to_lowercase(),
        kind,
        rom_sha256: Some(rom_hash),
        core_hash: core_hash.map(str::to_owned),
        state_format: (kind == domain::SaveKind::SaveState)
            .then(|| "libretro-serialize-v1".to_string()),
        slot,
        bytes: Some(bytes.len() as u64),
        sha256: Some(identity::sha256_bytes(bytes)),
        parent_id: None,
        created_at_utc: None,
        created_at_unix: Some(now_unix()),
        status: status.to_string(),
    };
    let file = if kind == domain::SaveKind::SaveState {
        "state.manifest.json"
    } else {
        "manifest.json"
    };
    let path = dir.join("by-id").join(key).join(file);
    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    atomic_write(&path, &json)
}

fn save_battery(core: &mut LibretroCore, game: &Game, dir: &Path, core_hash: Option<&str>) {
    if let Err(error) = validate_game_identity(game) {
        eprintln!("No se guardó SRAM porque cambió la ROM activa: {error}");
        return;
    }
    match core.read_save_ram() {
        Ok(data) => {
            let path = preferred_battery_path(game, dir);
            if let Err(error) = backup_verified_battery(game, dir, core_hash) {
                eprintln!(
                    "No se pudo respaldar SRAM anterior para {}: {error}",
                    safe_log_file_name(&game.path)
                );
                return;
            }
            match atomic_write(&path, &data) {
                Ok(()) => write_save_manifest(
                    game,
                    dir,
                    domain::SaveKind::BatteryRam,
                    &data,
                    None,
                    core_hash,
                    "committed",
                ),
                Err(e) => eprintln!(
                    "No se pudo guardar SRAM para {} en {}: {e}",
                    safe_log_file_name(&game.path),
                    safe_log_file_name(&path)
                ),
            }
        }
        Err(e) => {
            eprintln!(
                "No se pudo leer SRAM del núcleo para {}: {e:?}",
                safe_log_file_name(&game.path)
            );
        }
    }
}

fn state_path(game: &Game, dir: &Path) -> PathBuf {
    preferred_state_path(game, dir)
}

fn load_compatible_state(
    game: &Game,
    dir: &Path,
    core_hash: Option<&str>,
) -> Result<Vec<u8>, String> {
    validate_game_identity(game)?;
    let core_hash = core_hash.ok_or("No se pudo verificar el hash del núcleo; estado bloqueado")?;
    let path = preferred_state_path(game, dir);
    let rom_hash =
        identity::sha256_file(&game.path).map_err(|e| format!("No se pudo verificar ROM: {e}"))?;
    let manifest_path = path
        .parent()
        .ok_or("Ruta de estado inválida")?
        .join("state.manifest.json");
    let mut candidates = vec![(path.clone(), manifest_path)];
    let backup_root = path
        .parent()
        .ok_or("Ruta de estado inválida")?
        .join("backups");
    if let Ok(entries) = fs::read_dir(&backup_root) {
        let mut backups: Vec<_> = entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.path())
            .collect();
        backups.sort();
        backups.reverse();
        candidates.extend(
            backups
                .into_iter()
                .take(32)
                .map(|backup| (backup.join("slot1.ss"), backup.join("state.manifest.json"))),
        );
    }
    let mut last_error = None;
    for (candidate, manifest_file) in candidates {
        if !candidate.is_file() {
            continue;
        }
        match validate_state_files(&candidate, &manifest_file, &rom_hash, core_hash) {
            Ok(bytes) => return Ok(bytes),
            Err(error) => last_error = Some(error),
        }
    }
    if last_error.is_some() {
        return Err(last_error.unwrap());
    }
    if legacy_state_path(game, dir).is_file() {
        return Err(
            "El estado antiguo no tiene metadatos suficientes para verificar ROM y núcleo".into(),
        );
    }
    Err("No se encontró un estado guardado".into())
}

fn validate_state_files(
    state_path: &Path,
    manifest_path: &Path,
    rom_hash: &str,
    core_hash: &str,
) -> Result<Vec<u8>, String> {
    let bytes = fs::read(state_path).map_err(|e| format!("No se pudo leer el estado: {e}"))?;
    let manifest: domain::SaveManifest = serde_json::from_slice(
        &fs::read(manifest_path).map_err(|_| "El estado no tiene un manifiesto verificable")?,
    )
    .map_err(|_| "El manifiesto del estado está dañado")?;
    vault::validate_state(&manifest, &bytes, rom_hash, core_hash)?;
    Ok(bytes)
}

/// Preserve the last committed state before replacement. Each backup is a
/// self-contained directory; an interrupted copy cannot replace an older one.
fn backup_verified_state(
    game: &Game,
    dir: &Path,
    core_hash: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    let Some(core_hash) = core_hash else {
        return Ok(None);
    };
    let path = preferred_state_path(game, dir);
    if !path.is_file() {
        return Ok(None);
    }
    let rom_hash = identity::sha256_file(&game.path).map_err(|e| e.to_string())?;
    let manifest_path = path
        .parent()
        .ok_or("Ruta de estado inválida")?
        .join("state.manifest.json");
    let bytes = match validate_state_files(&path, &manifest_path, &rom_hash, core_hash) {
        Ok(bytes) => bytes,
        // A corrupt primary is not promoted over existing versioned backups.
        // The new F2 state can replace it while older verified backups remain.
        Err(_) => return Ok(None),
    };
    let manifest = fs::read(&manifest_path).map_err(|e| e.to_string())?;
    let backup_root = path
        .parent()
        .ok_or("Ruta de estado inválida")?
        .join("backups");
    fs::create_dir_all(&backup_root).map_err(|e| e.to_string())?;
    let base = now_unix().to_string();
    let backup_dir = (0u32..)
        .find_map(|suffix| {
            let candidate = backup_root.join(format!("{base}-{suffix:04}"));
            match fs::create_dir(&candidate) {
                Ok(()) => Some(Ok(candidate)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error.to_string())),
            }
        })
        .ok_or("No se pudo reservar una carpeta de respaldo")??;
    atomic_write(&backup_dir.join("slot1.ss"), &bytes)?;
    atomic_write(&backup_dir.join("state.manifest.json"), &manifest)?;
    if let Err(error) = prune_backup_root(
        &backup_root,
        "slot1.ss",
        "state.manifest.json",
        domain::SaveKind::SaveState,
        MAX_BACKUP_VERSIONS,
        MAX_BACKUP_BYTES,
    ) {
        eprintln!("No se pudo aplicar retención de estados: {error}");
    }
    Ok(Some(backup_dir))
}

fn backup_verified_battery(
    game: &Game,
    dir: &Path,
    core_hash: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    let Some(core_hash) = core_hash else {
        return Ok(None);
    };
    let path = preferred_battery_path(game, dir);
    if !path.is_file() {
        return Ok(None);
    }
    let rom_hash = identity::sha256_file(&game.path).map_err(|e| e.to_string())?;
    let manifest_path = path
        .parent()
        .ok_or("Ruta de SRAM inválida")?
        .join("manifest.json");
    let bytes = match validate_battery_files(&path, &manifest_path, &rom_hash, core_hash) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(None),
    };
    let manifest = fs::read(&manifest_path).map_err(|e| e.to_string())?;
    let backup_root = path
        .parent()
        .ok_or("Ruta de SRAM inválida")?
        .join("backups")
        .join("battery");
    fs::create_dir_all(&backup_root).map_err(|e| e.to_string())?;
    let base = now_unix().to_string();
    let backup_dir = (0u32..)
        .find_map(|suffix| {
            let candidate = backup_root.join(format!("{base}-{suffix:04}"));
            match fs::create_dir(&candidate) {
                Ok(()) => Some(Ok(candidate)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error.to_string())),
            }
        })
        .ok_or("No se pudo reservar una carpeta de respaldo de SRAM")??;
    atomic_write(&backup_dir.join("battery.sav"), &bytes)?;
    atomic_write(&backup_dir.join("manifest.json"), &manifest)?;
    if let Err(error) = prune_backup_root(
        &backup_root,
        "battery.sav",
        "manifest.json",
        domain::SaveKind::BatteryRam,
        MAX_BACKUP_VERSIONS,
        MAX_BACKUP_BYTES,
    ) {
        eprintln!("No se pudo aplicar retención de SRAM: {error}");
    }
    Ok(Some(backup_dir))
}

fn prune_backup_root(
    root: &Path,
    data_name: &str,
    manifest_name: &str,
    expected_kind: domain::SaveKind,
    max_versions: usize,
    max_bytes: u64,
) -> Result<(), String> {
    let mut backups = fs::read_dir(root)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if !entry.file_type().ok()?.is_dir() {
                return None;
            }
            let path = entry.path();
            let data = path.join(data_name);
            let manifest_path = path.join(manifest_name);
            let metadata = fs::metadata(&data).ok()?;
            let manifest: domain::SaveManifest =
                serde_json::from_slice(&fs::read(manifest_path).ok()?).ok()?;
            let hash = identity::sha256_file(&data).ok()?;
            if manifest.kind != expected_kind
                || manifest.status != "committed"
                || manifest.bytes != Some(metadata.len())
                || manifest.sha256.as_deref() != Some(&hash)
            {
                return None;
            }
            Some((
                entry.file_name().to_string_lossy().into_owned(),
                metadata.len(),
                path,
            ))
        })
        .collect::<Vec<_>>();
    backups.sort_by(|left, right| right.0.cmp(&left.0));

    let mut kept_versions = 0usize;
    let mut kept_bytes = 0u64;
    for (_, size, path) in backups {
        let fits = kept_versions == 0
            || (kept_versions < max_versions.max(1)
                && kept_bytes.saturating_add(size) <= max_bytes);
        if fits {
            kept_versions += 1;
            kept_bytes = kept_bytes.saturating_add(size);
        } else {
            fs::remove_dir_all(path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct GridLayout {
    left: i32,
    top: i32,
    card_w: i32,
    card_h: i32,
    gap: i32,
    columns: usize,
    rows: usize,
}

impl GridLayout {
    fn index_at(self, x: i32, y: i32, selected: usize) -> Option<usize> {
        if x < self.left || y < self.top {
            return None;
        }
        let col = (x - self.left) / (self.card_w + self.gap);
        let row = (y - self.top) / (self.card_h + self.gap);
        if col < 0 || row < 0 || col as usize >= self.columns || row as usize >= self.rows {
            return None;
        }
        if (x - self.left) % (self.card_w + self.gap) >= self.card_w
            || (y - self.top) % (self.card_h + self.gap) >= self.card_h
        {
            return None;
        }
        let page_size = self.columns * self.rows;
        Some((selected / page_size) * page_size + row as usize * self.columns + col as usize)
    }
}

fn library_grid_metrics(w: u32, h: u32) -> GridLayout {
    let available = (w as i32 - 452).max(320);
    let columns = (available / 156).max(2) as usize;
    let gap = 12;
    GridLayout {
        left: 200,
        top: 130,
        card_w: (available - (columns as i32 - 1) * gap) / columns as i32,
        card_h: 216,
        gap,
        columns,
        rows: ((h as i32 - 350) / 228).max(1) as usize,
    }
}

fn draw_library(
    c: &mut Canvas<Window>,
    _pixel: &Font,
    body: &Font,
    strong: &Font,
    games: &[Game],
    selected: usize,
    w: u32,
    h: u32,
    error: Option<&str>,
    config: &Settings,
) -> Result<(), String> {
    draw_sidebar(c, body, h, 0)?;
    text(c, strong, "Biblioteca de juegos", 200, 34, 27, INK);
    text(
        c,
        body,
        &format!("{} juegos · Colección local", games.len()),
        200,
        77,
        16,
        MUTED,
    );
    c.set_draw_color(BORDER);
    c.draw_line(Point::new(200, 110), Point::new(w as i32 - 24, 110))?;
    let right = w as i32 - 228;
    let layout = library_grid_metrics(w, h);
    if games.is_empty() {
        panel(c, Rect::new(200, 130, (right - 224) as u32, 216), PAPER)?;
        draw_logo(c, 222, 152, 58)?;
        text(c, strong, "Tu biblioteca empieza aquí", 222, 226, 22, INK);
        text(
            c,
            body,
            "Añade tus ROMs de GB, GBC, GBA o DS.",
            222,
            266,
            16,
            MUTED,
        );
        text(
            c,
            body,
            "Arrastra un archivo o pulsa Importar ROM.",
            222,
            295,
            16,
            MUTED,
        );
    } else {
        let count = layout.columns * layout.rows;
        let start = selected / count * count;
        for (slot, game) in games.iter().enumerate().skip(start).take(count) {
            let x = layout.left
                + ((slot - start) % layout.columns) as i32 * (layout.card_w + layout.gap);
            let y = layout.top
                + ((slot - start) / layout.columns) as i32 * (layout.card_h + layout.gap);
            let rect = Rect::new(x, y, layout.card_w as u32, layout.card_h as u32);
            panel(
                c,
                rect,
                if slot == selected {
                    DARK_CRIMSON
                } else {
                    PAPER
                },
            )?;
            c.set_clip_rect(rect);
            // Original geometric cover, explicitly identified as a placeholder.
            c.set_draw_color(Color::RGB(28, 23, 30));
            c.fill_rect(Rect::new(x + 8, y + 8, layout.card_w as u32 - 16, 118))?;
            c.set_draw_color(system_color(game.system));
            c.draw_line(
                Point::new(x + 8, y + 125),
                Point::new(x + layout.card_w - 8, y + 8),
            )?;
            draw_logo(c, x + (layout.card_w - 64) / 2, y + 28, 64)?;
            text(c, body, "Sin portada", x + 14, y + 101, 14, MUTED);
            let label = fit_label(strong, &game.title, 17, (layout.card_w - 24) as f32);
            text(c, strong, &label, x + 12, y + 140, 17, INK);
            text(c, body, game.system, x + 12, y + 170, 15, MUTED);
            if slot == selected {
                c.set_draw_color(GREEN);
                c.draw_rect(rect)?;
                c.fill_rect(Rect::new(x + 12, y + 199, layout.card_w as u32 - 24, 2))?;
            }
            c.set_clip_rect(None);
        }
    }
    panel(c, Rect::new(right, 130, 204, 216), PAPER)?;
    text(c, strong, "Sistemas", right + 16, 144, 18, INK);
    for (i, system) in ["GB", "GBC", "GBA", "DS"].iter().enumerate() {
        let yy = 182 + i as i32 * 38;
        let count = games.iter().filter(|g| g.system == *system).count();
        text(c, strong, system, right + 16, yy, 16, INK);
        text(
            c,
            body,
            &format!("{count} juegos"),
            right + 107,
            yy,
            15,
            MUTED,
        );
    }
    let can_play = games
        .get(selected)
        .is_some_and(|game| game.path.is_file() && config.core(game.system).is_ok());
    if can_play {
        draw_button(c, strong, Rect::new(right, 360, 204, 44), "Jugar  →")?;
    } else {
        panel(c, Rect::new(right, 360, 204, 44), PAPER)?;
        text(
            c,
            strong,
            if games.is_empty() {
                "Selecciona un juego"
            } else {
                "Configura el núcleo"
            },
            right + 12,
            372,
            15,
            MUTED,
        );
    }
    draw_button(c, strong, Rect::new(right, 414, 204, 44), "+  Importar ROM")?;
    panel(c, Rect::new(right, 468, 204, 44), PAPER)?;
    text(c, strong, "Ajustes de núcleos", right + 16, 480, 16, INK);

    let bottom = (h as i32 - 236).max(370);
    let center_width = right - 224;
    let half = (center_width - 12) / 2;
    for (xx, title) in [(200, "Vyntra Vault"), (212 + half, "Estado de núcleos")] {
        panel(c, Rect::new(xx, bottom, half as u32, 158), PAPER)?;
        text(c, strong, title, xx + 14, bottom + 12, 17, INK);
        c.set_draw_color(BORDER);
        c.draw_line(
            Point::new(xx + 14, bottom + 42),
            Point::new(xx + half - 14, bottom + 42),
        )?;
    }
    // Snapshot file metadata only; no state is claimed compatible merely for existing.
    let mut saves: Vec<_> = fs::read_dir(&config.saves)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            Some((meta.modified().ok(), entry.file_name(), meta.len()))
        })
        .collect();
    saves.sort_by(|a, b| b.0.cmp(&a.0));
    if saves.is_empty() {
        text(
            c,
            body,
            "Sin guardados en esta carpeta",
            214,
            bottom + 56,
            14,
            MUTED,
        );
    } else {
        for (i, (_, name, size)) in saves.iter().take(2).enumerate() {
            let label = fit_label(body, &name.to_string_lossy(), 15, (half - 28) as f32);
            text(c, body, &label, 214, bottom + 52 + i as i32 * 45, 15, INK);
            text(
                c,
                body,
                &format!("{} KB · guardado del juego", size / 1024),
                214,
                bottom + 73 + i as i32 * 45,
                14,
                MUTED,
            );
        }
    }
    for (i, system) in ["GBC", "GBA", "DS"].iter().enumerate() {
        let status = if config.core(system).is_ok() {
            "Detectado · sin probar"
        } else {
            "No instalado"
        };
        let label = fit_label(body, &format!("{system}: {status}"), 14, (half - 28) as f32);
        text(
            c,
            body,
            &label,
            226 + half,
            bottom + 55 + i as i32 * 29,
            14,
            MUTED,
        );
    }
    text(
        c,
        body,
        "↑ ↓ seleccionar   ·   Intro jugar   ·   F5 actualizar",
        200,
        h as i32 - 46,
        14,
        MUTED,
    );
    if let Some(msg) = error {
        let yy = h as i32 - 95;
        panel(
            c,
            Rect::new(200, yy, w.saturating_sub(224), 80),
            DARK_CRIMSON,
        )?;
        text(
            c,
            strong,
            "No se pudo iniciar el juego",
            216,
            yy + 8,
            17,
            INK,
        );
        text(
            c,
            body,
            &fit_label(body, msg, 15, w.saturating_sub(264) as f32),
            216,
            yy + 32,
            15,
            INK,
        );
        text(
            c,
            body,
            "Haz clic aquí para ver el diagnóstico completo.",
            216,
            yy + 54,
            14,
            MUTED,
        );
    }
    Ok(())
}

fn draw_sidebar(c: &mut Canvas<Window>, body: &Font, h: u32, active: u8) -> Result<(), String> {
    c.set_draw_color(SIDEBAR);
    c.fill_rect(Rect::new(0, 0, 176, h))
        .map_err(|e| e.to_string())?;
    draw_logo(c, 23, 24, 48)?;
    text(c, body, "Vyntra", 24, 82, 22, INK);
    text(c, body, "DESKTOP", 25, 109, 11, AQUA);
    for (i, label, y) in [
        (0, "Biblioteca", 134),
        (1, "Acompañante", 200),
        (2, "Ajustes", 266),
    ] {
        if active == i {
            c.set_draw_color(DARK_CRIMSON);
            c.fill_rect(Rect::new(12, y - 7, 152, 48))
                .map_err(|e| e.to_string())?;
        }
        draw_nav_icon(c, i, 27, y + 7, if active == i { INK } else { AQUA })?;
        text(
            c,
            body,
            label,
            58,
            y + 7,
            16,
            if active == i { INK } else { MUTED },
        );
    }
    c.set_draw_color(BORDER);
    c.draw_line(Point::new(22, 352), Point::new(154, 352))
        .map_err(|e| e.to_string())?;
    text(c, body, "Tu biblioteca", 24, 378, 13, MUTED);
    text(c, body, "de juegos.", 24, 399, 13, MUTED);
    text(c, body, "VYNTRA · WINDOWS", 24, h as i32 - 28, 11, MUTED);
    Ok(())
}

fn draw_info_page(
    c: &mut Canvas<Window>,
    pixel: &Font,
    body: &Font,
    strong: &Font,
    core_status: &[(&str, String); 3],
    w: u32,
    h: u32,
    settings: bool,
) -> Result<(), String> {
    draw_sidebar(c, body, h, if settings { 2 } else { 1 })?;
    let title = if settings {
        "Ajustes de Vyntra Desktop"
    } else {
        "Vyntra Companion"
    };
    text(c, pixel, title, 203, 45, 26, INK);
    text(
        c,
        body,
        if settings {
            "Configura núcleos, biblioteca y guardados."
        } else {
            "Los datos se muestran solo si el juego y el núcleo los admiten."
        },
        204,
        86,
        15,
        MUTED,
    );
    if settings {
        let rows = [
            ("NÚCLEO GB / GBC", core_status[0].1.as_str()),
            ("NÚCLEO GBA", core_status[1].1.as_str()),
            ("NÚCLEO DS", core_status[2].1.as_str()),
            ("BIBLIOTECA", "./data/roms · F5 actualiza esta carpeta"),
            ("DATOS", "./data/saves y ./data/states"),
            (
                "RUTA DE NÚCLEOS",
                "./cores o VYNTRA_CORE_DIR · POKEDAISY_CORE_DIR legado",
            ),
        ];
        for (i, (label, value)) in rows.iter().enumerate() {
            let y = 137 + i as i32 * 56;
            panel(
                c,
                Rect::new(202, y, (w.saturating_sub(234)).max(400), 48),
                PAPER,
            )?;
            text(c, strong, label, 220, y + 9, 12, GREEN);
            text(c, body, value, 388, y + 9, 13, INK);
        }
    } else {
        let rows = [
            ("Equipo", "Aún no disponible en Desktop"),
            ("Mochila", "Aún no disponible en Desktop"),
            ("Atlas", "Aún no disponible en Desktop"),
            ("Vyntra Dex", "Aún no disponible en Desktop"),
            ("Combate", "Aún no disponible en Desktop"),
            ("Menú rápido", "Solo disponible durante una partida"),
        ];
        for (i, (label, value)) in rows.iter().enumerate() {
            let y = 143 + i as i32 * 67;
            panel(
                c,
                Rect::new(202, y, (w.saturating_sub(234)).max(400), 54),
                if i % 2 == 0 {
                    PAPER
                } else {
                    Color::RGB(23, 23, 27)
                },
            )?;
            text(c, strong, label, 220, y + 8, 14, INK);
            text(c, body, value, 430, y + 8, 14, MUTED);
        }
        text(
            c,
            body,
            "La integración del acompañante de escritorio está pendiente.",
            202,
            h as i32 - 31,
            13,
            GREEN,
        );
    }
    Ok(())
}

fn draw_logo(c: &mut Canvas<Window>, x: i32, y: i32, size: u32) -> Result<(), String> {
    let size = size as i32;
    let left = x + size / 7;
    let center = x + size / 2;
    let right = x + size * 6 / 7;
    let top = y + size / 6;
    let bottom = y + size * 5 / 6;
    let stroke = (size / 9).max(3);
    c.set_draw_color(GREEN);
    for offset in 0..stroke {
        c.draw_line(
            Point::new(left + offset, top),
            Point::new(center + offset, bottom),
        )
        .map_err(|e| e.to_string())?;
        c.draw_line(
            Point::new(center + offset, bottom),
            Point::new(right + offset, top),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn draw_nav_icon(
    c: &mut Canvas<Window>,
    which: u8,
    x: i32,
    y: i32,
    color: Color,
) -> Result<(), String> {
    c.set_draw_color(color);
    match which {
        0 => {
            c.draw_rect(Rect::new(x, y, 22, 19))
                .map_err(|e| e.to_string())?;
            c.draw_line(Point::new(x + 4, y + 6), Point::new(x + 18, y + 6))
                .map_err(|e| e.to_string())?;
            c.draw_line(Point::new(x + 4, y + 12), Point::new(x + 18, y + 12))
                .map_err(|e| e.to_string())?;
        }
        1 => {
            c.draw_rect(Rect::new(x, y, 9, 9))
                .map_err(|e| e.to_string())?;
            c.draw_rect(Rect::new(x + 13, y + 10, 9, 9))
                .map_err(|e| e.to_string())?;
            c.draw_line(Point::new(x + 10, y + 5), Point::new(x + 20, y + 15))
                .map_err(|e| e.to_string())?;
        }
        _ => {
            for yy in [y, y + 8, y + 16] {
                c.draw_line(Point::new(x, yy), Point::new(x + 22, yy))
                    .map_err(|e| e.to_string())?;
                c.fill_rect(Rect::new(x + 5, yy - 2, 4, 4))
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
fn draw_button(c: &mut Canvas<Window>, font: &Font, r: Rect, label: &str) -> Result<(), String> {
    c.set_draw_color(GREEN);
    c.fill_rect(r).map_err(|e| e.to_string())?;
    text(c, font, label, r.x() + 16, r.y() + 13, 16, INK);
    Ok(())
}
fn system_color(system: &str) -> Color {
    match system {
        "GBA" => Color::RGB(118, 89, 175),
        "DS" => Color::RGB(62, 125, 177),
        "GBC" => Color::RGB(210, 157, 55),
        "GB" => Color::RGB(83, 140, 105),
        _ => GREEN,
    }
}
fn panel(c: &mut Canvas<Window>, r: Rect, color: Color) -> Result<(), String> {
    c.set_draw_color(color);
    c.fill_rect(r).map_err(|e| e.to_string())?;
    c.set_draw_color(BORDER);
    c.draw_rect(r).map_err(|e| e.to_string())?;
    Ok(())
}
fn draw_play_header(c: &mut Canvas<Window>, f: &Font, w: u32) -> Result<(), String> {
    c.set_draw_color(Color::RGBA(24, 37, 39, 220));
    c.fill_rect(Rect::new(0, 0, w, 48))
        .map_err(|e| e.to_string())?;
    text(c, f, "VYNTRA", 20, 16, 13, PAPER);
    text(
        c,
        f,
        "Esc: biblioteca    F11: pantalla completa",
        (w as i32) - 325,
        16,
        11,
        AQUA,
    );
    Ok(())
}
fn draw_play(
    c: &mut Canvas<Window>,
    tc: &TextureCreator<WindowContext>,
    frame: &VideoFrame,
    w: u32,
    h: u32,
    smooth: bool,
) -> Result<(), String> {
    if frame.width == 0 || frame.height == 0 {
        return Ok(());
    };
    let mut rgba = vec![0u8; (frame.width * frame.height * 4) as usize];
    for y in 0..frame.height as usize {
        for x in 0..frame.width as usize {
            let src = y * frame.pitch + x * frame.format.bytes_per_pixel();
            let dst = (y * frame.width as usize + x) * 4;
            if src + frame.format.bytes_per_pixel() > frame.pixels.len() {
                continue;
            }
            let (r, g, b) = match frame.format {
                PixelFormat::Xrgb8888 => (
                    frame.pixels[src + 2],
                    frame.pixels[src + 1],
                    frame.pixels[src],
                ),
                PixelFormat::Rgb565 => {
                    let p = u16::from_le_bytes([frame.pixels[src], frame.pixels[src + 1]]);
                    (
                        ((p >> 11 & 31) * 255 / 31) as u8,
                        (((p >> 5 & 63) * 255 / 63) as u8),
                        ((p & 31) * 255 / 31) as u8,
                    )
                }
                PixelFormat::Rgb1555 => {
                    let p = u16::from_le_bytes([frame.pixels[src], frame.pixels[src + 1]]);
                    (
                        ((p >> 10 & 31) * 255 / 31) as u8,
                        (((p >> 5 & 31) * 255 / 31) as u8),
                        ((p & 31) * 255 / 31) as u8,
                    )
                }
            };
            rgba[dst..dst + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }
    let mut tex = tc
        .create_texture_streaming(
            sdl2::pixels::PixelFormatEnum::RGBA32,
            frame.width,
            frame.height,
        )
        .map_err(|e| e.to_string())?;
    tex.set_scale_mode(if smooth {
        sdl2::render::ScaleMode::Linear
    } else {
        sdl2::render::ScaleMode::Nearest
    });
    tex.update(None, &rgba, frame.width as usize * 4)
        .map_err(|e| e.to_string())?;
    let margin = 24;
    let availw = w.saturating_sub(margin * 2);
    let availh = h.saturating_sub(72);
    let scale = (availw as f32 / frame.width as f32).min(availh as f32 / frame.height as f32);
    let dw = (frame.width as f32 * scale).round().max(1.0) as u32;
    let dh = (frame.height as f32 * scale).round().max(1.0) as u32;
    let x = (w.saturating_sub(dw) / 2) as i32;
    let y = (48 + availh.saturating_sub(dh) / 2) as i32;
    c.set_draw_color(Color::RGB(14, 24, 22));
    c.fill_rect(Rect::new(0, 48, w, h - 48))
        .map_err(|e| e.to_string())?;
    c.copy(&tex, None, Rect::new(x, y, dw, dh))
        .map_err(|e| e.to_string())?;
    c.set_draw_color(AQUA);
    c.draw_rect(Rect::new(x - 5, y - 5, dw + 10, dh + 10))
        .map_err(|e| e.to_string())?;
    Ok(())
}
// Rasterize with the font's coverage mask intact. Thresholding the mask destroyed
// antialiasing and made small text illegible. Blend a single texture per label.
fn fit_label(font: &Font, value: &str, size: u32, available: f32) -> String {
    let width = |s: &str| {
        s.chars()
            .map(|c| font.metrics(c, size as f32).advance_width)
            .sum::<f32>()
    };
    if width(value) <= available {
        return value.to_owned();
    }
    let mut result = String::new();
    for ch in value.chars() {
        if width(&result) + width(&ch.to_string()) + width("…") > available {
            break;
        }
        result.push(ch);
    }
    result.push('…');
    result
}

fn text(c: &mut Canvas<Window>, font: &Font, s: &str, x: i32, y: i32, size: u32, color: Color) {
    let color = if color == GREEN {
        Color::RGB(255, 142, 163)
    } else if color == PAPER {
        INK
    } else {
        color
    };
    let logical_size = size.max(14);
    let density = c
        .output_size()
        .map(|(w, _)| w as f32 / c.window().size().0 as f32)
        .unwrap_or(1.0)
        .max(1.0);
    let size = (logical_size as f32 * density).round() as u32;
    let limit = ((c.window().size().0 as i32 - x - 20).max(1) as f32 * density) as usize;
    let glyphs: Vec<_> = s
        .chars()
        .map(|ch| font.rasterize(ch, size as f32))
        .collect();
    let width = (glyphs
        .iter()
        .map(|(m, _)| m.advance_width)
        .sum::<f32>()
        .ceil() as usize
        + 4)
    .min(limit)
    .max(1);
    let height = size as usize + 8;
    let mut rgba = vec![0u8; width * height * 4];
    let mut pen = 0.0f32;
    for (m, bitmap) in glyphs {
        for by in 0..m.height {
            for bx in 0..m.width {
                let px = pen.round() as i32 + m.xmin + bx as i32;
                let py = size as i32 - m.height as i32 - m.ymin + by as i32;
                if px >= 0 && px < width as i32 && py >= 0 && py < height as i32 {
                    let offset = (py as usize * width + px as usize) * 4;
                    rgba[offset..offset + 4].copy_from_slice(&[
                        color.r,
                        color.g,
                        color.b,
                        bitmap[by * m.width + bx],
                    ]);
                }
            }
        }
        pen += m.advance_width;
        if pen > width as f32 {
            break;
        }
    }
    let creator = c.texture_creator();
    if let Ok(mut texture) = creator.create_texture_streaming(
        sdl2::pixels::PixelFormatEnum::RGBA32,
        width as u32,
        height as u32,
    ) {
        texture.set_blend_mode(sdl2::render::BlendMode::Blend);
        texture.set_scale_mode(sdl2::render::ScaleMode::Linear);
        if texture.update(None, &rgba, width * 4).is_ok() {
            let _ = c.copy(
                &texture,
                None,
                Rect::new(
                    x,
                    y,
                    (width as f32 / density).ceil() as u32,
                    (height as f32 / density).ceil() as u32,
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{atomic_write, core_names, find_core_in_dirs, scan_games_controlled};
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn configured_keyboard_bindings_map_to_libretro_buttons() {
        let mut config = super::Settings::default();
        config.keys[0] = "A".into();
        let input = super::keyboard_joypad_state(&config, |key| key == super::Keycode::A);
        assert!(input.contains(super::JoypadButton::A));
        assert!(!input.contains(super::JoypadButton::B));
    }

    #[test]
    fn explicit_game_source_selects_matching_duplicate_and_rejects_mismatch() {
        let root =
            std::env::temp_dir().join(format!("vyntra-source-choice-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let first_path = root.join("first.gb");
        let second_path = root.join("second.gb");
        fs::write(&first_path, b"same rom bytes").unwrap();
        fs::write(&second_path, b"same rom bytes").unwrap();
        let games = vec![
            super::game_from_path(first_path),
            super::game_from_path(second_path),
        ];
        let storage_id = super::game_storage_key(&games[0]);

        assert_eq!(
            super::resolve_game_by_source(&games, &storage_id, 1)
                .unwrap()
                .path,
            games[1].path
        );
        assert!(super::resolve_game_by_source(&games, "different-content", 1).is_none());
        assert!(super::resolve_game_by_source(&games, &storage_id, 2).is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn filesystem_diagnostic_labels_hide_parent_paths_and_control_characters() {
        let raw_path = format!("C:\\private\\user-data\\ROM{}.gba", '\u{1b}');
        let label = super::safe_log_file_name(std::path::Path::new(&raw_path));
        assert_eq!(label, "ROM\u{fffd}.gba");
        assert!(!label.contains("private"));

        let long_path =
            std::path::Path::new(&format!("C:\\private\\{}.gba", "R".repeat(200))).to_path_buf();
        assert!(super::safe_log_file_name(&long_path).len() <= 128);
    }

    #[test]
    fn player_start_errors_name_the_file_without_exposing_parent_directories() {
        let path = std::path::Path::new("C:\\Users\\PrivateUser\\cores\\test.dll");
        let error = super::player_file_error("iniciar el núcleo", path);
        assert!(error.contains("test.dll"));
        assert!(error.contains("Comprueba que el archivo exista"));
        assert!(!error.contains("PrivateUser"));
        assert!(!error.contains("cores"));
    }

    fn exercise_real_core_frame_and_state(core: &mut super::LibretroCore) {
        let first = core.run_frame(super::JoypadState::default()).unwrap();
        assert!(first.video.as_ref().is_some_and(|frame| {
            frame.width > 0 && frame.height > 0 && !frame.pixels.is_empty()
        }));
        let state = core
            .serialize()
            .expect("core must serialize a loaded homebrew game");
        assert!(!state.is_empty());
        let _ = core.run_frame(super::JoypadState::default()).unwrap();
        core.unserialize(&state)
            .expect("core must restore its serialized homebrew state");
    }

    #[test]
    #[ignore = "smoke manual de ROM homebrew/core local; requiere VYNTRA_CORE_GAME_SMOKE_PATH y VYNTRA_CORE_GAME_SMOKE_ROM"]
    fn configured_core_opens_the_homebrew_gba_diagnostic_rom() {
        let core_path = std::path::PathBuf::from(
            std::env::var_os("VYNTRA_CORE_GAME_SMOKE_PATH")
                .expect("define VYNTRA_CORE_GAME_SMOKE_PATH"),
        );
        let rom_path = std::path::PathBuf::from(
            std::env::var_os("VYNTRA_CORE_GAME_SMOKE_ROM")
                .expect("define VYNTRA_CORE_GAME_SMOKE_ROM"),
        );
        let root =
            std::env::temp_dir().join(format!("vyntra-homebrew-smoke-{}", std::process::id()));
        let saves = root.join("saves");
        let system = root.join("system");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::create_dir_all(&system).unwrap();
        let mut config = super::Settings::default();
        config.cores[1] = Some(core_path);
        let game = super::game_from_path(rom_path);
        let mut core =
            super::open_game(&game, std::slice::from_ref(&game), &saves, &system, &config)
                .expect("mGBA must load the homebrew ROM through the production open_game path");
        let info = core.system_info();
        assert_eq!(info.library_name.as_deref(), Some("mGBA"));
        assert_eq!(
            info.library_version.as_deref(),
            Some("0.11-212-7a12d6d"),
            "the tested mGBA build must report its pinned upstream revision"
        );
        exercise_real_core_frame_and_state(&mut core);
        let mut battery = core.read_save_ram().expect("mGBA must expose save RAM");
        assert!(!battery.is_empty(), "mGBA must expose writable save RAM");
        battery[0] ^= 0xff;
        core.write_save_ram(&battery)
            .expect("mGBA must accept changed save RAM data");
        let read_back = core
            .read_save_ram()
            .expect("mGBA must return save RAM after a write");
        assert_eq!(read_back, battery);
        let core_hash = super::identity::sha256_file(&config.cores[1].clone().unwrap()).unwrap();
        super::save_battery(&mut core, &game, &saves, Some(&core_hash));
        assert_eq!(
            super::load_compatible_battery(&game, &saves, config.cores[1].as_ref().unwrap())
                .unwrap()
                .as_deref(),
            Some(battery.as_slice()),
            "the production save manifest must validate the persisted core bytes"
        );
        drop(core);
        let mut reopened =
            super::open_game(&game, std::slice::from_ref(&game), &saves, &system, &config)
                .expect("mGBA must reopen the game through the production path");
        assert_eq!(
            reopened.read_save_ram().unwrap(),
            battery,
            "open_game must restore the committed battery save"
        );
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "smoke manual de ROM NDS homebrew/core local; requiere VYNTRA_CORE_GAME_SMOKE_PATH y VYNTRA_CORE_GAME_SMOKE_ROM"]
    fn configured_core_opens_the_homebrew_ds_diagnostic_rom() {
        let core_path = std::path::PathBuf::from(
            std::env::var_os("VYNTRA_CORE_GAME_SMOKE_PATH")
                .expect("define VYNTRA_CORE_GAME_SMOKE_PATH"),
        );
        let rom_path = std::path::PathBuf::from(
            std::env::var_os("VYNTRA_CORE_GAME_SMOKE_ROM")
                .expect("define VYNTRA_CORE_GAME_SMOKE_ROM"),
        );
        let root =
            std::env::temp_dir().join(format!("vyntra-homebrew-ds-smoke-{}", std::process::id()));
        let saves = root.join("saves");
        let system = root.join("system");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::create_dir_all(&system).unwrap();
        let mut config = super::Settings::default();
        config.cores[2] = Some(core_path);
        assert_eq!(
            super::inspect::inspect_file(&rom_path).map(|info| info.system),
            Some("DS")
        );
        let game = super::game_from_path(rom_path);
        assert_eq!(game.system, "DS");
        assert_eq!(
            config.core("DS").unwrap(),
            std::path::PathBuf::from(std::env::var_os("VYNTRA_CORE_GAME_SMOKE_PATH").unwrap())
        );
        let mut core =
            super::open_game(&game, std::slice::from_ref(&game), &saves, &system, &config).expect(
                "melonDS must load the homebrew NDS ROM through the production open_game path",
            );
        let info = core.system_info();
        assert_eq!(info.library_name.as_deref(), Some("melonDS DS"));
        assert_eq!(
            info.library_version.as_deref(),
            Some("1.4.0"),
            "the tested melonDS DS build must report the audited release"
        );
        assert!(info.valid_extensions.unwrap_or_default().contains("nds"));
        exercise_real_core_frame_and_state(&mut core);
        drop(core);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancelled_library_scan_discards_partial_results() {
        let dir = std::env::temp_dir().join(format!("vyntra-cancel-scan-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let rom = dir.join("fixture.gba");
        fs::write(&rom, vec![0u8; 0xc0]).unwrap();
        let cancelled = AtomicBool::new(true);
        assert!(scan_games_controlled(&dir, true, &cancelled).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn superseded_library_scan_discards_results_after_io_has_started() {
        let dir =
            std::env::temp_dir().join(format!("vyntra-cancel-mid-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("first.gba"), {
            let mut rom = vec![0u8; 0xc0];
            rom[0xb2] = 0x96;
            rom[0xac..0xb0].copy_from_slice(b"ABCD");
            rom[0xbc] = 0x19;
            rom
        })
        .unwrap();
        fs::write(dir.join("second.gba"), {
            let mut rom = vec![0u8; 0xc0];
            rom[0xb2] = 0x96;
            rom[0xac..0xb0].copy_from_slice(b"EFGH");
            rom[0xbc] = 0x19;
            rom
        })
        .unwrap();
        let cancelled = AtomicBool::new(false);

        let found = super::scan_games_controlled_with(&dir, false, &cancelled, |_| {
            cancelled.store(true, Ordering::Release);
        });

        assert!(found.is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_atomic_rename_preserves_existing_target_and_cleans_staging_file() {
        let root =
            std::env::temp_dir().join(format!("vyntra-atomic-rename-{}", std::process::id()));
        let target = root.join("target");
        let marker = target.join("original.txt");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&target).unwrap();
        fs::write(&marker, b"original").unwrap();

        assert!(atomic_write(&target, b"replacement").is_err());
        assert_eq!(fs::read(&marker).unwrap(), b"original");
        assert!(!root.join("target.tmp").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_atomic_staging_preserves_existing_target_for_storage_and_permission_errors() {
        let root = std::env::temp_dir().join(format!("vyntra-atomic-full-{}", std::process::id()));
        let target = root.join("save.sav");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(&target, b"committed-save").unwrap();

        let error = super::atomic_write_with(&target, b"replacement", |file, data| {
            use std::io::Write;
            file.write_all(&data[..3])?;
            Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
        });

        assert!(error.is_err());
        assert_eq!(fs::read(&target).unwrap(), b"committed-save");
        assert!(!root.join("save.tmp").exists());

        let denied = super::atomic_write_with(&target, b"replacement", |_, _| {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        });
        assert!(denied.is_err());
        assert_eq!(fs::read(&target).unwrap(), b"committed-save");
        assert!(!root.join("save.tmp").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn atomic_write_recovers_after_process_termination_during_staging() {
        const CRASH_PATH_ENV: &str = "VYNTRA_ATOMIC_WRITE_CRASH_PATH";
        if let Some(path) = std::env::var_os(CRASH_PATH_ENV) {
            let target = std::path::PathBuf::from(path);
            let _ = super::atomic_write_with(&target, b"replacement", |file, _| {
                use std::io::Write;
                file.write_all(b"partial")?;
                std::process::exit(73);
            });
            unreachable!("the subprocess exits while staging the replacement");
        }

        let root = std::env::temp_dir().join(format!(
            "vyntra-atomic-crash-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("save.sav");
        fs::write(&target, b"committed-save").unwrap();

        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::atomic_write_recovers_after_process_termination_during_staging",
            ])
            .env(CRASH_PATH_ENV, &target)
            .status()
            .unwrap();

        assert_eq!(child.code(), Some(73));
        assert_eq!(fs::read(&target).unwrap(), b"committed-save");
        assert_eq!(fs::read(root.join("save.tmp")).unwrap(), b"partial");

        atomic_write(&target, b"recovered-save").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"recovered-save");
        assert!(!root.join("save.tmp").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn each_system_prefers_vyntra_core_names_and_keeps_legacy_aliases() {
        for (system, expected, legacy) in [
            (
                "GB",
                "vyntra_gbc_libretro.dll",
                "pokedaisy_gbc_libretro.dll",
            ),
            (
                "GBA",
                "vyntra_gba_libretro.dll",
                "pokedaisy_gba_libretro.dll",
            ),
            ("DS", "vyntra_ds_libretro.dll", "pokedaisy_ds_libretro.dll"),
        ] {
            let names = core_names(system);
            assert_eq!(names.first(), Some(&expected));
            assert!(names.contains(&legacy));
        }
    }

    #[test]
    fn core_search_keeps_directory_priority_and_requires_the_expected_name() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("vyntra-core-test-{nonce}"));
        let preferred = root.join("preferred");
        let fallback = root.join("fallback");
        fs::create_dir_all(&preferred).unwrap();
        fs::create_dir_all(&fallback).unwrap();
        fs::write(preferred.join("pokedaisy_gba_libretro.dll"), b"legacy").unwrap();
        fs::write(fallback.join("vyntra_gba_libretro.dll"), b"current").unwrap();

        assert_eq!(
            find_core_in_dirs(&[preferred.clone(), fallback], "GBA"),
            Some(preferred.join("pokedaisy_gba_libretro.dll"))
        );
        assert!(find_core_in_dirs(&[root.clone()], "GBA").is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn save_manifest_is_written_atomically() {
        let dir = std::env::temp_dir().join(format!("vyntra-manifest-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let rom = dir.join("game.gb");
        fs::write(&rom, b"rom").unwrap();
        let game = super::Game {
            path: rom,
            title: "game".into(),
            system: "GB",
            storage_key: std::sync::OnceLock::new(),
            rom_sha256: std::sync::OnceLock::new(),
        };
        let core_path = dir.join("fixture-core.dll");
        fs::write(&core_path, b"core bytes").unwrap();
        let core_hash = super::identity::sha256_file(&core_path).unwrap();
        super::write_save_manifest(
            &game,
            &dir,
            super::domain::SaveKind::BatteryRam,
            b"ram",
            None,
            Some(&core_hash),
            "committed",
        );
        let key = super::game_storage_key(&game);
        let manifest_path = dir.join("by-id").join(key).join("manifest.json");
        assert!(manifest_path.is_file());
        assert!(!manifest_path.with_extension("tmp").exists());
        let parsed: super::domain::SaveManifest =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        assert_eq!(parsed.bytes, Some(3));
        assert_eq!(parsed.kind, super::domain::SaveKind::BatteryRam);
        assert!(parsed.sha256.is_some());
        assert!(parsed.state_format.is_none());
        let committed_manifest = fs::read(&manifest_path).unwrap();
        fs::write(&game.path, b"ROM changed during the active session").unwrap();
        assert!(
            super::write_save_manifest_checked(
                &game,
                &dir,
                super::domain::SaveKind::BatteryRam,
                b"new ram",
                None,
                Some(&core_hash),
                "committed",
            )
            .is_err()
        );
        assert_eq!(fs::read(&manifest_path).unwrap(), committed_manifest);
        fs::write(&game.path, b"rom").unwrap();
        fs::write(super::preferred_battery_path(&game, &dir), b"ram").unwrap();
        let battery_backup = super::backup_verified_battery(&game, &dir, Some(&core_hash))
            .unwrap()
            .unwrap();
        let partial_battery = battery_backup
            .parent()
            .unwrap()
            .join("zz-incomplete-backup");
        fs::create_dir(&partial_battery).unwrap();
        fs::write(
            partial_battery.join("battery.sav"),
            b"orphan payload without manifest",
        )
        .unwrap();
        fs::write(super::preferred_battery_path(&game, &dir), b"damaged").unwrap();
        assert_eq!(
            super::load_compatible_battery(&game, &dir, &core_path)
                .unwrap()
                .unwrap(),
            b"ram"
        );
        fs::write(battery_backup.join("battery.sav"), b"damaged backup").unwrap();
        assert!(super::load_compatible_battery(&game, &dir, &core_path).is_err());

        super::write_save_manifest(
            &game,
            &dir,
            super::domain::SaveKind::SaveState,
            b"state",
            Some(1),
            Some("corehash"),
            "committed",
        );
        fs::write(super::state_path(&game, &dir), b"state").unwrap();
        let state_manifest_path = manifest_path.with_file_name("state.manifest.json");
        assert!(state_manifest_path.is_file());
        let parsed: super::domain::SaveManifest =
            serde_json::from_slice(&fs::read(&state_manifest_path).unwrap()).unwrap();
        assert_eq!(parsed.slot, Some(1));
        assert_eq!(parsed.core_hash.as_deref(), Some("corehash"));
        assert_eq!(
            parsed.state_format.as_deref(),
            Some("libretro-serialize-v1")
        );
        assert_eq!(
            super::load_compatible_state(&game, &dir, Some("corehash")).unwrap(),
            b"state"
        );
        let committed_manifest = fs::read(&state_manifest_path).unwrap();
        let backup = super::backup_verified_state(&game, &dir, Some("corehash"))
            .unwrap()
            .unwrap();
        assert_eq!(fs::read(backup.join("slot1.ss")).unwrap(), b"state");
        let partial_state = backup.parent().unwrap().join("zz-incomplete-backup");
        fs::create_dir(&partial_state).unwrap();
        fs::write(
            partial_state.join("slot1.ss"),
            b"orphan state without manifest",
        )
        .unwrap();
        const INTERRUPT_PATH_ENV: &str = "VYNTRA_STATE_PAYLOAD_INTERRUPT_PATH";
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::interrupt_after_state_payload_write_before_manifest",
            ])
            .env(INTERRUPT_PATH_ENV, super::state_path(&game, &dir))
            .status()
            .unwrap();
        assert_eq!(child.code(), Some(74));
        assert_eq!(fs::read(&state_manifest_path).unwrap(), committed_manifest);
        assert_eq!(
            super::load_compatible_state(&game, &dir, Some("corehash")).unwrap(),
            b"state"
        );
        fs::write(backup.join("slot1.ss"), b"also tampered").unwrap();
        assert!(super::load_compatible_state(&game, &dir, Some("corehash")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn interrupt_after_state_payload_write_before_manifest() {
        const INTERRUPT_PATH_ENV: &str = "VYNTRA_STATE_PAYLOAD_INTERRUPT_PATH";
        let Some(path) = std::env::var_os(INTERRUPT_PATH_ENV) else {
            return;
        };
        atomic_write(
            &std::path::PathBuf::from(path),
            b"new state without its manifest",
        )
        .unwrap();
        std::process::exit(74);
    }

    #[test]
    fn backup_retention_keeps_newest_verified_copies_only() {
        let root = std::env::temp_dir().join(format!("vyntra-retention-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for (name, payload) in [
            ("001", b"old".as_slice()),
            ("002", b"middle"),
            ("003", b"new"),
        ] {
            let backup = root.join(name);
            std::fs::create_dir_all(&backup).unwrap();
            std::fs::write(backup.join("slot1.ss"), payload).unwrap();
            let hash = super::identity::sha256_file(&backup.join("slot1.ss")).unwrap();
            std::fs::write(
                backup.join("state.manifest.json"),
                serde_json::json!({
                    "save_id": name, "kind": "save_state", "status": "committed",
                    "bytes": payload.len(), "sha256": hash
                })
                .to_string(),
            )
            .unwrap();
        }

        super::prune_backup_root(
            &root,
            "slot1.ss",
            "state.manifest.json",
            super::domain::SaveKind::SaveState,
            2,
            1024,
        )
        .unwrap();

        assert!(!root.join("001").exists());
        assert!(root.join("002/slot1.ss").is_file());
        assert!(root.join("003/slot1.ss").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }
}
