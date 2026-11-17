#[cfg(windows)]
#[test]
fn core_probe_child_reports_loadable_fixture_and_system_mismatch() {
    use std::process::Command;

    let compiler = "x86_64-w64-mingw32-gcc";
    if Command::new(compiler).arg("--version").output().is_err() {
        eprintln!("Skipping loadable DLL fixture: MinGW compiler is unavailable");
        return;
    }
    let root = std::env::temp_dir().join(format!("vyntra-probe-child-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core_probe_fixture.c");
    let dll = root.join("vyntra-probe-fixture.dll");
    let built = Command::new(compiler)
        .arg("-shared")
        .arg("-O2")
        .arg("-o")
        .arg(&dll)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let launcher = env!("CARGO_BIN_EXE_vyntra-desktop");
    for (system, expected_status, expected_match) in
        [("GBA", "loadable", true), ("DS", "load_failed", false)]
    {
        let output = Command::new(launcher)
            .arg("--probe-core")
            .arg(&dll)
            .arg(system)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["status"], expected_status);
        assert_eq!(report["system_match"], expected_match);
        assert_eq!(report["library_name"], "Vyntra Probe Fixture");
    }

    let dependency_source = root.join("dependency.c");
    let dependency = root.join("dependency.dll");
    std::fs::write(
        &dependency_source,
        "__declspec(dllexport) void vyntra_probe_dependency(void) {}\n",
    )
    .unwrap();
    let dependency_build = Command::new(compiler)
        .arg("-shared")
        .arg("-O2")
        .arg("-o")
        .arg(&dependency)
        .arg(&dependency_source)
        .output()
        .unwrap();
    assert!(
        dependency_build.status.success(),
        "{}",
        String::from_utf8_lossy(&dependency_build.stderr)
    );
    let dependent_dll = root.join("vyntra-dependent-fixture.dll");
    let dependent_build = Command::new(compiler)
        .arg("-shared")
        .arg("-O2")
        .arg("-DPROBE_MISSING_DEPENDENCY")
        .arg("-L")
        .arg(&root)
        .arg("-o")
        .arg(&dependent_dll)
        .arg(&source)
        .arg("-ldependency")
        .output()
        .unwrap();
    assert!(
        dependent_build.status.success(),
        "{}",
        String::from_utf8_lossy(&dependent_build.stderr)
    );
    std::fs::remove_file(&dependency).unwrap();
    let output = Command::new(launcher)
        .arg("--probe-core")
        .arg(&dependent_dll)
        .arg("GBA")
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "load_failed");
    assert!(
        report["reason"]
            .as_str()
            .is_some_and(|reason| !reason.is_empty())
    );
    let _ = std::fs::remove_dir_all(root);
}
