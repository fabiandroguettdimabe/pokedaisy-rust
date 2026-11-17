//! Static, non-executing Windows PE inspection for candidate libretro cores.
use std::fs;
use std::path::Path;

const REQUIRED_EXPORTS: &[&str] = &[
    "retro_set_environment",
    "retro_set_video_refresh",
    "retro_set_audio_sample",
    "retro_set_audio_sample_batch",
    "retro_set_input_poll",
    "retro_set_input_state",
    "retro_api_version",
    "retro_init",
    "retro_deinit",
    "retro_get_system_info",
    "retro_load_game",
    "retro_unload_game",
    "retro_run",
    "retro_get_system_av_info",
    "retro_serialize_size",
    "retro_serialize",
    "retro_unserialize",
    "retro_get_memory_data",
    "retro_get_memory_size",
    "retro_set_controller_port_device",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeReport {
    pub architecture: String,
    pub exports: Vec<String>,
    pub missing_exports: Vec<String>,
}

pub fn inspect_file(path: &Path) -> Result<PeReport, String> {
    let metadata = fs::metadata(path).map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
    if metadata.len() > 512 * 1024 * 1024 {
        return Err("La DLL supera el límite de inspección de 512 MiB".into());
    }
    let bytes = fs::read(path).map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
    inspect_bytes(&bytes)
}

pub fn inspect_bytes(bytes: &[u8]) -> Result<PeReport, String> {
    if bytes.get(0..2) != Some(b"MZ") {
        return Err("Falta la cabecera DOS MZ".into());
    }
    let pe = read_u32(bytes, 0x3c)? as usize;
    if bytes.get(pe..pe.checked_add(4).ok_or("Offset PE inválido")?) != Some(b"PE\0\0") {
        return Err("Firma PE inválida".into());
    }
    let machine = read_u16(bytes, pe + 4)?;
    let architecture = match machine {
        0x8664 => "x86_64",
        0x014c => "x86",
        0xaa64 => "aarch64",
        _ => return Err(format!("Arquitectura PE no admitida: 0x{machine:04x}")),
    };
    let section_count = read_u16(bytes, pe + 6)? as usize;
    if section_count == 0 || section_count > 96 {
        return Err("Cantidad de secciones PE inválida".into());
    }
    let optional_size = read_u16(bytes, pe + 20)? as usize;
    let optional = pe.checked_add(24).ok_or("Cabecera PE inválida")?;
    let magic = read_u16(bytes, optional)?;
    let (directory_count_offset, export_directory_offset) = match magic {
        0x10b => (92, 96),
        0x20b => (108, 112),
        _ => return Err("Formato PE32/PE32+ desconocido".into()),
    };
    if optional_size < export_directory_offset + 8
        || read_u32(bytes, optional + directory_count_offset)? == 0
    {
        return Err("La DLL no declara una tabla de exports".into());
    }
    let export_rva = read_u32(bytes, optional + export_directory_offset)?;
    if export_rva == 0 {
        return Err("La DLL no declara exports".into());
    }
    let section_table = optional
        .checked_add(optional_size)
        .ok_or("Tabla de secciones inválida")?;
    let sections = (0..section_count)
        .map(|i| {
            let at = section_table
                .checked_add(i.checked_mul(40).ok_or("Tabla de secciones inválida")?)
                .ok_or("Tabla de secciones inválida")?;
            Ok((
                read_u32(bytes, at + 8)?,
                read_u32(bytes, at + 12)?,
                read_u32(bytes, at + 16)?,
                read_u32(bytes, at + 20)?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let export_offset = rva_to_offset(bytes, export_rva, &sections)?;
    let names_count = read_u32(bytes, export_offset + 24)? as usize;
    if names_count > 65536 {
        return Err("La tabla PE excede el límite de nombres".into());
    }
    let names_rva = read_u32(bytes, export_offset + 32)?;
    let names_offset = rva_to_offset(bytes, names_rva, &sections)?;
    let mut exports = Vec::with_capacity(names_count);
    for i in 0..names_count {
        let name_rva = read_u32(
            bytes,
            names_offset
                .checked_add(i.checked_mul(4).ok_or("Tabla export inválida")?)
                .ok_or("Tabla export inválida")?,
        )?;
        exports.push(read_c_string(
            bytes,
            rva_to_offset(bytes, name_rva, &sections)?,
        )?);
    }
    exports.sort();
    exports.dedup();
    let missing_exports = REQUIRED_EXPORTS
        .iter()
        .filter(|name| !exports.iter().any(|export| export == **name))
        .map(|name| (*name).to_string())
        .collect();
    Ok(PeReport {
        architecture: architecture.into(),
        exports,
        missing_exports,
    })
}

fn rva_to_offset(
    bytes: &[u8],
    rva: u32,
    sections: &[(u32, u32, u32, u32)],
) -> Result<usize, String> {
    for (virtual_size, virtual_address, raw_size, raw_pointer) in sections {
        let span = (*virtual_size).max(*raw_size);
        let end = virtual_address
            .checked_add(span)
            .ok_or("RVA PE desbordado")?;
        if rva >= *virtual_address && rva < end {
            let delta = rva - virtual_address;
            if delta >= *raw_size {
                return Err("RVA apunta a datos no almacenados en el archivo".into());
            }
            let offset = raw_pointer
                .checked_add(delta)
                .ok_or("Offset PE desbordado")? as usize;
            if offset < bytes.len() {
                return Ok(offset);
            }
        }
    }
    Err("RVA de export fuera de las secciones del archivo".into())
}

fn read_c_string(bytes: &[u8], offset: usize) -> Result<String, String> {
    let tail = bytes
        .get(offset..)
        .ok_or("Nombre export fuera del archivo")?;
    let end = tail
        .iter()
        .take(512)
        .position(|b| *b == 0)
        .ok_or("Nombre export sin terminador")?;
    let name = std::str::from_utf8(&tail[..end]).map_err(|_| "Nombre export no UTF-8")?;
    if name.is_empty() {
        return Err("Nombre export vacío".into());
    }
    Ok(name.to_string())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset.checked_add(2).ok_or("Offset PE desbordado")?)
            .ok_or("Cabecera PE truncada")?
            .try_into()
            .unwrap(),
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset.checked_add(4).ok_or("Offset PE desbordado")?)
            .ok_or("Cabecera PE truncada")?
            .try_into()
            .unwrap(),
    ))
}
