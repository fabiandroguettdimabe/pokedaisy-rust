#![cfg(windows)]

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
