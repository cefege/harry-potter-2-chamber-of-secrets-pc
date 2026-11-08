//! Checked filesystem text conversions shared by canonical Core and Engine owners.
use crate::core_access::{RuntimeError, RuntimeResult};
use std::path::{Path, PathBuf};

pub fn to_utf16(units: &[u32]) -> RuntimeResult<Vec<u16>> {
    let mut output = Vec::with_capacity(units.len());
    for &unit in units.iter().take_while(|&&unit| unit != 0) {
        let character = char::from_u32(unit)
            .ok_or_else(|| RuntimeError::new("Canonical path contains invalid TCHAR data"))?;
        let mut buffer = [0_u16; 2];
        output.extend_from_slice(character.encode_utf16(&mut buffer));
    }
    Ok(output)
}

pub fn from_utf16_units(units: &[u16]) -> RuntimeResult<Vec<u32>> {
    let mut output = Vec::with_capacity(units.len() + 1);
    for character in char::decode_utf16(units.iter().copied().take_while(|&unit| unit != 0)) {
        output.push(u32::from(character.map_err(|_| {
            RuntimeError::new("Canonical path contains invalid UTF-16 data")
        })?));
    }
    Ok(output)
}

pub fn from_utf32(units: &[u32]) -> RuntimeResult<String> {
    let mut output = String::new();
    for &unit in units.iter().take_while(|&&unit| unit != 0) {
        output.push(char::from_u32(unit).ok_or_else(|| {
            RuntimeError::new("Canonical search path contains invalid TCHAR data")
        })?);
    }
    Ok(output)
}

pub fn path_from_units(units: &[u32]) -> RuntimeResult<PathBuf> {
    Ok(PathBuf::from(from_utf32(units)?))
}

pub fn path_from_utf16(units: &[u16]) -> RuntimeResult<PathBuf> {
    let end = units
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(units.len());
    String::from_utf16(&units[..end])
        .map(PathBuf::from)
        .map_err(|_| RuntimeError::new("Canonical path contains invalid UTF-16 data"))
}

pub fn path_to_units(path: &Path) -> RuntimeResult<Vec<u32>> {
    let text = path
        .to_str()
        .ok_or_else(|| RuntimeError::new("Canonical search path is not valid UTF-8"))?;
    Ok(text.chars().map(u32::from).collect())
}
