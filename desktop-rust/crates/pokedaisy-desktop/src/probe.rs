//! Isolated libretro core verification; the launcher invokes this mode in a child process.
use crate::probe_pe::{self, PeReport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use vyntra_libretro::{CorePaths, LibretroCore};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeReport {
    pub path: String,
    pub expected_system: String,
    pub system_match: Option<bool>,
    pub sha256: Option<String>,
    pub status: String,
    pub architecture: Option<String>,
    pub exports: Vec<String>,
    pub missing_exports: Vec<String>,
    pub api_version: Option<u32>,
    pub library_name: Option<String>,
    pub library_version: Option<String>,
    pub valid_extensions: Option<String>,
    pub need_fullpath: Option<bool>,
    pub block_extract: Option<bool>,
    pub duration_ms: u128,
    pub reason: Option<String>,
}

impl ProbeReport {
    fn base(path: &Path, expected_system: &str) -> Self {
        Self {
            path: path.to_string_lossy().into_owned(),
            expected_system: expected_system.to_string(),
            system_match: None,
            sha256: crate::identity::sha256_file(path).ok(),
            status: "unverified".into(),
            architecture: None,
            exports: Vec::new(),
            missing_exports: Vec::new(),
            api_version: None,
            library_name: None,
            library_version: None,
            valid_extensions: None,
            need_fullpath: None,
            block_extract: None,
            duration_ms: 0,
            reason: None,
        }
    }
}

pub fn run_probe(path: &Path, expected_system: &str) -> ProbeReport {
    let start = Instant::now();
    let mut report = ProbeReport::base(path, expected_system);
    let pe = match probe_pe::inspect_file(path) {
        Ok(pe) => pe,
        Err(error) => return finish(report, start, "invalid_pe", error),
    };
    copy_pe(&mut report, &pe);
    if pe.architecture != std::env::consts::ARCH {
        return finish(
            report,
            start,
            "wrong_architecture",
            format!(
                "Se requiere {}, la DLL es {}",
                std::env::consts::ARCH,
                pe.architecture
            ),
        );
    }
    if !pe.missing_exports.is_empty() {
        return finish(
            report,
            start,
            "missing_exports",
            "La DLL no exporta todas las funciones libretro requeridas".into(),
        );
    }
    let root = std::env::temp_dir().join(format!("vyntra-core-probe-{}", std::process::id()));
    let save_dir = root.join("saves");
    let system_dir = root.join("system");
    if let Err(error) =
        std::fs::create_dir_all(&save_dir).and_then(|_| std::fs::create_dir_all(&system_dir))
    {
        return finish(report, start, "probe_setup_failed", error.to_string());
    }
    let core = LibretroCore::load(
        path,
        CorePaths {
            save_dir,
            system_dir,
            panel_width: 640,
            panel_height: 480,
        },
    );
    let result = match core {
        Ok(core) => {
            report.api_version = Some(core.api_version());
            let info = core.system_info();
            report.library_name = info.library_name;
            report.library_version = info.library_version;
            report.valid_extensions = info.valid_extensions;
            report.need_fullpath = Some(info.need_fullpath);
            report.block_extract = Some(info.block_extract);
            report.system_match = Some(extensions_match(
                expected_system,
                report.valid_extensions.as_deref(),
            ));
            if report.library_name.is_none() {
                Err("retro_get_system_info no devolvió un nombre de núcleo".into())
            } else if report.system_match != Some(true) {
                Err(format!(
                    "El núcleo no declara extensiones compatibles con {expected_system}"
                ))
            } else {
                Ok(())
            }
        }
        Err(error) => Err(error.to_string()),
    };
    let _ = std::fs::remove_dir_all(root);
    match result {
        Ok(()) => finish(report, start, "loadable", String::new()),
        Err(error) => finish(report, start, "load_failed", error),
    }
}

fn copy_pe(report: &mut ProbeReport, pe: &PeReport) {
    report.architecture = Some(pe.architecture.clone());
    report.exports = pe.exports.clone();
    report.missing_exports = pe.missing_exports.clone();
}

fn finish(mut report: ProbeReport, start: Instant, status: &str, reason: String) -> ProbeReport {
    report.status = status.into();
    report.duration_ms = start.elapsed().as_millis();
    report.reason = (!reason.is_empty()).then_some(reason);
    report
}

pub fn run_child() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(2)
        .map(PathBuf::from)
        .ok_or("Uso interno: --probe-core <ruta-dll>")?;
    let expected_system = std::env::args().nth(3).ok_or("Falta el sistema esperado")?;
    let report = run_probe(&path, &expected_system);
    println!(
        "{}",
        serde_json::to_string(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}

pub fn probe_child(
    executable: &Path,
    core_path: &Path,
    expected_system: &str,
    timeout: Duration,
) -> Result<Value, String> {
    let mut command = Command::new(executable);
    command
        .arg("--probe-core")
        .arg(core_path)
        .arg(expected_system)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let child = command
        .spawn()
        .map_err(|e| format!("No se pudo iniciar el proceso de diagnóstico: {e}"))?;
    let output = wait_for_child(child, timeout)?;
    serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Reporte de diagnóstico inválido: {e}"))
}

fn wait_for_child(
    mut child: std::process::Child,
    timeout: Duration,
) -> Result<std::process::Output, String> {
    let start = Instant::now();
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => {
                let output = child.wait_with_output().map_err(|e| e.to_string())?;
                if !status.success() {
                    return Err(format!(
                        "El proceso de diagnóstico terminó sin reporte ({status})"
                    ));
                }
                return Ok(output);
            }
            None if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("El núcleo no respondió dentro del límite de diagnóstico".into());
            }
            None => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}

pub fn extensions_match(system: &str, extensions: Option<&str>) -> bool {
    let expected: &[&str] = match system.to_ascii_uppercase().as_str() {
        "GB" => &["gb"],
        "GBC" => &["gb", "gbc"],
        "GBA" => &["gba"],
        "DS" | "NDS" => &["nds"],
        _ => return false,
    };
    extensions.is_some_and(|values| {
        values
            .split('|')
            .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
            .any(|value| expected.contains(&value.as_str()))
    })
}

#[cfg(all(test, windows))]
mod tests {
    use super::wait_for_child;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    #[test]
    fn probe_supervisor_kills_a_child_that_exceeds_its_deadline() {
        let child = Command::new("cmd.exe")
            .args(["/C", "ping", "127.0.0.1", "-n", "30"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        let error = wait_for_child(child, Duration::from_millis(100)).unwrap_err();
        assert!(error.contains("límite de diagnóstico"));
    }
}
