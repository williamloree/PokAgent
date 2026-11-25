//! Mise à jour automatique depuis les releases GitHub du dépôt : si la dernière
//! release porte une version plus récente, on télécharge pokagent.exe, on vérifie
//! son empreinte SHA-256 (fichier pokagent.exe.sha256 de la release), on remplace
//! l'exécutable et on relance. HTTP via WinHTTP, hachage via BCrypt : aucune
//! dépendance en plus.

use serde_json::Value;
use std::path::PathBuf;
use windows::core::{w, PCWSTR};
use windows::Win32::Networking::WinHttp::*;
use windows::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};

pub const REPO: &str = "williamloree/PokAgent";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

struct Handle(*mut core::ffi::c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

/// GET HTTPS ; les redirections (téléchargement des fichiers d'une release) sont suivies.
fn get(url: &str) -> Result<Vec<u8>, String> {
    let rest = url.strip_prefix("https://").ok_or("adresse non HTTPS")?;
    let (host, path) = match rest.split_once('/') {
        Some((h, p)) => (h, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (whost, wpath) = (wide(host), wide(&path));
    unsafe {
        let s = Handle(WinHttpOpen(w!("PokAgent-updater"), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0));
        if s.0.is_null() {
            return Err("WinHttpOpen".into());
        }
        let c = Handle(WinHttpConnect(s.0, PCWSTR(whost.as_ptr()), INTERNET_DEFAULT_HTTPS_PORT as u16, 0));
        if c.0.is_null() {
            return Err("connexion impossible".into());
        }
        let r = Handle(WinHttpOpenRequest(c.0, w!("GET"), PCWSTR(wpath.as_ptr()), PCWSTR::null(), PCWSTR::null(), std::ptr::null(), WINHTTP_FLAG_SECURE));
        if r.0.is_null() {
            return Err("requête impossible".into());
        }
        WinHttpSendRequest(r.0, None, None, 0, 0, 0).map_err(|e| e.to_string())?;
        WinHttpReceiveResponse(r.0, std::ptr::null_mut()).map_err(|e| e.to_string())?;
        let mut code: u32 = 0;
        let mut len = 4u32;
        WinHttpQueryHeaders(
            r.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut code as *mut u32 as *mut _),
            &mut len,
            std::ptr::null_mut(),
        )
        .map_err(|e| e.to_string())?;
        if code != 200 {
            return Err(format!("HTTP {code}"));
        }
        let mut out = Vec::new();
        loop {
            let mut avail = 0u32;
            WinHttpQueryDataAvailable(r.0, &mut avail).map_err(|e| e.to_string())?;
            if avail == 0 {
                break;
            }
            let start = out.len();
            out.resize(start + avail as usize, 0);
            let mut read = 0u32;
            WinHttpReadData(r.0, out[start..].as_mut_ptr() as *mut _, avail, &mut read).map_err(|e| e.to_string())?;
            out.truncate(start + read as usize);
        }
        Ok(out)
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let mut out = [0u8; 32];
    unsafe {
        let _ = BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, data, &mut out);
    }
    out.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_ver(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn newer(remote: &str, local: &str) -> bool {
    parse_ver(remote) > parse_ver(local)
}

/// Dernière version déjà tentée : évite de boucler si une release est mal versionnée.
fn tried_file() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("PokAgent").join("update_tried.txt")
}

pub enum Outcome {
    UpToDate,
    Installed(String),
}

/// Cherche une version plus récente et l'installe à la place de l'exécutable.
/// Il reste à relancer le programme.
pub fn check_and_install(manual: bool) -> Result<Outcome, String> {
    let body = get(&format!("https://api.github.com/repos/{REPO}/releases/latest"))?;
    let rel: Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let tag = rel.get("tag_name").and_then(Value::as_str).ok_or("release sans tag")?.to_string();
    if !newer(&tag, VERSION) {
        return Ok(Outcome::UpToDate);
    }
    if !manual && std::fs::read_to_string(tried_file()).ok().as_deref() == Some(tag.as_str()) {
        return Ok(Outcome::UpToDate);
    }
    let asset = |name: &str| -> Option<String> {
        rel.get("assets")?
            .as_array()?
            .iter()
            .find(|a| a.get("name").and_then(Value::as_str) == Some(name))?
            .get("browser_download_url")?
            .as_str()
            .map(str::to_string)
    };
    let exe_url = asset("pokagent.exe").ok_or("pokagent.exe absent de la release")?;
    let sha_url = asset("pokagent.exe.sha256").ok_or("pokagent.exe.sha256 absent de la release")?;
    let expected: String = String::from_utf8_lossy(&get(&sha_url)?).chars().take(64).collect::<String>().to_lowercase();
    let exe = get(&exe_url)?;
    if sha256_hex(&exe) != expected {
        return Err("empreinte SHA-256 différente : téléchargement refusé".into());
    }
    let cur = std::env::current_exe().map_err(|e| e.to_string())?;
    let new = cur.with_extension("exe.new");
    let old = cur.with_extension("exe.old");
    std::fs::write(&new, &exe).map_err(|e| format!("écriture impossible : {e}"))?;
    let _ = std::fs::remove_file(&old);
    // Windows autorise à renommer un exécutable en cours d'exécution.
    std::fs::rename(&cur, &old).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&new, &cur) {
        let _ = std::fs::rename(&old, &cur);
        return Err(e.to_string());
    }
    let _ = std::fs::create_dir_all(tried_file().parent().unwrap());
    let _ = std::fs::write(tried_file(), &tag);
    Ok(Outcome::Installed(tag))
}

/// Supprime l'ancien exécutable laissé par la dernière mise à jour.
pub fn cleanup() {
    if let Ok(cur) = std::env::current_exe() {
        let _ = std::fs::remove_file(cur.with_extension("exe.old"));
        let _ = std::fs::remove_file(cur.with_extension("exe.new"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert!(newer("v0.2.0", "0.1.9"));
        assert!(newer("v1.0.0", "0.9.9"));
        assert!(!newer("v0.1.0", "0.1.0"));
        assert!(!newer("v0.0.9", "0.1.0"));
    }
    /// Réseau : cargo test http -- --ignored
    #[test]
    #[ignore]
    fn http() {
        let body = get(&format!("https://api.github.com/repos/{REPO}")).unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["full_name"], REPO);
    }

    #[test]
    fn sha() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
