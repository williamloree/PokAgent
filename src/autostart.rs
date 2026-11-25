//! Lancement avec Windows : valeur « PokAgent » dans
//! HKCU\Software\Microsoft\Windows\CurrentVersion\Run.

use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::*;

const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const NAME: PCWSTR = w!("PokAgent");

fn command() -> Option<String> {
    std::env::current_exe().ok().map(|p| format!("\"{}\"", p.display()))
}

pub fn is_on() -> bool {
    let mut buf = [0u16; 1024];
    let mut len = (buf.len() * 2) as u32;
    let ok = unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN, NAME, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut len)) }.is_ok();
    let val = String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]);
    ok && command().is_some_and(|c| c.eq_ignore_ascii_case(&val))
}

pub fn set(on: bool) -> Result<(), String> {
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(HKEY_CURRENT_USER, RUN, 0, PCWSTR::null(), REG_OPTION_NON_VOLATILE, KEY_SET_VALUE, None, &mut key, None)
            .ok()
            .map_err(|e| e.to_string())?;
        let res = if on {
            let cmd = command().ok_or("chemin de l'exécutable introuvable")?;
            let data: Vec<u8> = cmd.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
            RegSetValueExW(key, NAME, 0, REG_SZ, Some(&data))
        } else {
            match RegDeleteValueW(key, NAME) {
                e if e == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND => windows::Win32::Foundation::ERROR_SUCCESS,
                e => e,
            }
        };
        let _ = RegCloseKey(key);
        res.ok().map_err(|e| e.to_string())
    }
}
