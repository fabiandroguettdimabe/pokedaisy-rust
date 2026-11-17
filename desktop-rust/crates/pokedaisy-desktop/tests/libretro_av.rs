#![cfg(windows)]

use sha2::Digest;
use std::fs;
use std::path::Path;
use std::process::Command;
use vyntra_libretro::{CorePaths, JoypadButton, JoypadState, LibretroCore, PixelFormat};

#[test]
fn synthetic_core_runs_video_audio_and_runtime_av_through_libretro_api() {
    let compiler = "x86_64-w64-mingw32-gcc";
    if Command::new(compiler).arg("--version").output().is_err() {
        eprintln!("Skipping libretro AV fixture: MinGW compiler is unavailable");
        return;
    }

    let root = std::env::temp_dir().join(format!("vyntra-libretro-av-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/av_core_fixture.c");
    let dll = root.join("vyntra-av-fixture.dll");
    let build = Command::new(compiler)
        .args(["-std=c11", "-O2", "-shared"])
        .arg(&fixture)
        .arg("-o")
        .arg(&dll)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "fixture build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    println!(
        "fixture_core_hash={:x}",
        sha2::Sha256::digest(fs::read(&dll).unwrap())
    );

    let save_dir = root.join("saves");
    let system_dir = root.join("system");
    fs::create_dir_all(&save_dir).unwrap();
    fs::create_dir_all(&system_dir).unwrap();
    let rom = root.join("fixture.gba");
    fs::write(&rom, b"fixture rom").unwrap();

    let mut core = LibretroCore::load(
        &dll,
        CorePaths {
            save_dir,
            system_dir,
            panel_width: 640,
            panel_height: 480,
        },
    )
    .unwrap();
    let initial = core.load_game(&rom).unwrap();
    let core_info = core.system_info();
    assert_eq!(core_info.library_name.as_deref(), Some("Vyntra AV Fixture"));
    assert_eq!(core_info.library_version.as_deref(), Some("1.0"));
    assert!(!core.supports_save_states());
    assert_eq!(initial.geometry.base_width, 2);
    assert_eq!(initial.geometry.base_height, 1);
    assert_eq!(initial.fps, 60.0);
    assert_eq!(initial.sample_rate, 32_000.0);

    let frame = core.run_frame(Default::default()).unwrap();
    let owned_ram = core.read_system_ram().expect("fixture exposes system RAM");
    assert_eq!(
        core.read_memory_range(None, 0x0200_0002, 3).unwrap(),
        [3, 4, 5]
    );
    assert_eq!(
        core.read_memory_range(None, 0x0300_0001, 2).unwrap(),
        [8, 9]
    );
    assert_eq!(owned_ram, [1, 2, 3, 4]);
    let video = frame.video.expect("fixture emits a video frame");
    assert_eq!((video.width, video.height, video.pitch), (2, 1, 4));
    assert_eq!(video.format, PixelFormat::Rgb1555);
    assert_eq!(video.pixels, [0x1f, 0x00, 0x00, 0x7c]);
    assert_eq!(frame.audio, [3000, -3000, 1000, -1000, 2000, -2000]);

    let mut input = JoypadState::default();
    input.set(JoypadButton::A, true);
    let pressed = core
        .run_frame(input)
        .unwrap()
        .video
        .expect("fixture emits a frame after input");
    assert_eq!(pressed.pixels, [0xe0, 0x03, 0x00, 0x7c]);
    assert_eq!(
        owned_ram,
        [1, 2, 3, 4],
        "the capture owns its bytes after the core advances"
    );
    assert_eq!(core.read_system_ram().unwrap(), [2, 2, 3, 4]);

    assert_eq!(core.read_system_ram_range(1, 2).unwrap(), [2, 3]);
    assert!(matches!(
        core.read_system_ram_range(usize::MAX, 2),
        Err(vyntra_libretro::CoreError::SystemRamRangeOutOfBounds { .. })
    ));
    let updated = core.av_info();
    assert_eq!(
        (updated.geometry.base_width, updated.geometry.base_height),
        (4, 3)
    );
    assert_eq!(
        (updated.geometry.max_width, updated.geometry.max_height),
        (8, 6)
    );
    assert!((updated.geometry.aspect_ratio - 4.0 / 3.0).abs() < 0.001);
    assert_eq!(updated.fps, 50.0);
    assert_eq!(updated.sample_rate, 22_050.0);

    drop(core);
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "local core smoke: set VYNTRA_CORE_SMOKE_PATH and VYNTRA_CORE_SMOKE_ROM"]
fn configured_local_gbc_core_runs_generated_diagnostic_rom() {
    let core_path = std::env::var_os("VYNTRA_CORE_SMOKE_PATH").expect("VYNTRA_CORE_SMOKE_PATH");
    let rom_path = std::env::var_os("VYNTRA_CORE_SMOKE_ROM").expect("VYNTRA_CORE_SMOKE_ROM");
    let core_path = Path::new(&core_path);
    let rom_path = Path::new(&rom_path);
    let root = std::env::temp_dir().join(format!("vyntra-gbc-smoke-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let save_dir = root.join("saves");
    let system_dir = root.join("system");
    fs::create_dir_all(&save_dir).unwrap();
    fs::create_dir_all(&system_dir).unwrap();
    println!(
        "fixture_core_hash={:x}",
        sha2::Sha256::digest(fs::read(core_path).unwrap())
    );
    println!(
        "fixture_rom_hash={:x}",
        sha2::Sha256::digest(fs::read(rom_path).unwrap())
    );

    let mut core = LibretroCore::load(
        core_path,
        CorePaths {
            save_dir,
            system_dir,
            panel_width: 640,
            panel_height: 480,
        },
    )
    .unwrap();
    let av = core
        .load_game(rom_path)
        .expect("generated diagnostic ROM loads");
    assert_eq!(
        core.system_info().library_name.as_deref(),
        Some("PokeDaisy")
    );
    assert!(av.geometry.base_width >= 160);
    assert!(av.geometry.base_height >= 144);
    let mut saw_video = false;
    let mut saw_audio = false;
    for _ in 0..120 {
        let frame = core.run_frame(Default::default()).unwrap();
        saw_video |= frame.video.is_some();
        saw_audio |= !frame.audio.is_empty();
    }
    assert!(saw_video, "core did not emit a video frame");
    assert!(saw_audio, "diagnostic ROM did not produce audio samples");
    assert_eq!(
        core.read_save_ram().unwrap()[0],
        0x5a,
        "diagnostic save marker"
    );
    let state = core.serialize().expect("core serializes state");
    assert!(!state.is_empty());
    core.unserialize(&state)
        .expect("core restores serialized state");
    drop(core);
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "local GBA core smoke: set VYNTRA_GBA_CORE_SMOKE_PATH and VYNTRA_GBA_CORE_SMOKE_ROM"]
fn configured_local_core_runs_generated_gba_diagnostic_rom() {
    let core_path =
        std::env::var_os("VYNTRA_GBA_CORE_SMOKE_PATH").expect("VYNTRA_GBA_CORE_SMOKE_PATH");
    let rom_path =
        std::env::var_os("VYNTRA_GBA_CORE_SMOKE_ROM").expect("VYNTRA_GBA_CORE_SMOKE_ROM");
    let core_path = Path::new(&core_path);
    let rom_path = Path::new(&rom_path);
    let root = std::env::temp_dir().join(format!("vyntra-gba-core-smoke-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let save_dir = root.join("saves");
    let system_dir = root.join("system");
    fs::create_dir_all(&save_dir).unwrap();
    fs::create_dir_all(&system_dir).unwrap();
    println!(
        "gba_core_hash={:x}",
        sha2::Sha256::digest(fs::read(core_path).unwrap())
    );
    println!(
        "gba_rom_hash={:x}",
        sha2::Sha256::digest(fs::read(rom_path).unwrap())
    );

    let mut core = LibretroCore::load(
        core_path,
        CorePaths {
            save_dir,
            system_dir,
            panel_width: 640,
            panel_height: 480,
        },
    )
    .expect("the Windows core DLL loads");
    let info = core.system_info();
    assert!(
        info.valid_extensions
            .as_deref()
            .unwrap_or_default()
            .split('|')
            .any(|extension| extension.eq_ignore_ascii_case("gba")),
        "the core must advertise GBA ROMs"
    );
    let av = core
        .load_game(rom_path)
        .expect("the generated GBA diagnostic ROM loads");
    assert!(av.geometry.base_width >= 240);
    assert!(av.geometry.base_height >= 160);

    let mut saw_video = false;
    for _ in 0..120 {
        let frame = core.run_frame(Default::default()).unwrap();
        saw_video |= frame.video.is_some();
    }
    assert!(saw_video, "the GBA core did not emit video");
    let state = core.serialize().expect("the GBA core serializes state");
    assert!(!state.is_empty());
    core.unserialize(&state)
        .expect("the GBA core restores serialized state");
    drop(core);
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "GBA core input smoke: set VYNTRA_GBA_CORE_SMOKE_PATH"]
fn configured_local_gba_core_receives_a_button_from_libretro_input() {
    let core_path = std::env::var_os("VYNTRA_GBA_CORE_SMOKE_PATH").expect("core path");
    let root = std::env::temp_dir().join(format!("vyntra-gba-input-smoke-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let rom_path = root.join("diagnostic-input.gba");
    let generator =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../linux/make_diagnostic_gba_input.py");
    let generated = Command::new("python")
        .arg(generator)
        .arg(&rom_path)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let save_dir = root.join("saves");
    let system_dir = root.join("system");
    fs::create_dir_all(&save_dir).unwrap();
    fs::create_dir_all(&system_dir).unwrap();
    let mut core = LibretroCore::load(
        Path::new(&core_path),
        CorePaths {
            save_dir,
            system_dir,
            panel_width: 640,
            panel_height: 480,
        },
    )
    .expect("GBA core loads");
    core.load_game(&rom_path)
        .expect("input diagnostic ROM loads");
    let mut released = None;
    for _ in 0..4 {
        released = core.run_frame(JoypadState::default()).unwrap().video;
    }
    let released = released.expect("released frame exists");
    let mut pressed = JoypadState::default();
    pressed.set(JoypadButton::A, true);
    let mut pressed_video = None;
    for _ in 0..4 {
        pressed_video = core.run_frame(pressed).unwrap().video;
    }
    let pressed_video = pressed_video.expect("pressed frame exists");
    assert_eq!(
        &released.pixels[..2],
        &[0, 248],
        "released A must render the red diagnostic screen"
    );
    assert_eq!(
        &pressed_video.pixels[..2],
        &[192, 7],
        "pressed A must render the green diagnostic screen"
    );
    assert_eq!(
        core.read_save_ram().unwrap().first(),
        Some(&0xA5),
        "pressed A must write the diagnostic battery marker"
    );
    drop(core);
    fs::remove_dir_all(root).unwrap();
}
