//! Le hook Notification de Claude Code lance `pokagent.exe hook` et lui passe un
//! JSON sur l'entrée standard. On le recopie dans un petit fichier par session,
//! que le widget surveille. `install-hook` ajoute ce hook à ~/.claude/settings.json.

use crate::watcher::{now_ms, questions_dir};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, TRUE};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER};

pub fn run_hook() {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let Ok(v) = serde_json::from_str::<Value>(&input) else { return };
    let kind = v.get("notification_type").and_then(Value::as_str).unwrap_or("");
    let msg = v.get("message").and_then(Value::as_str).unwrap_or("");
    // « Claude attend ta saisie » arrive à chaque fin de tour : ce n'est pas une question.
    if kind == "idle_prompt" || msg.contains("waiting for your input") {
        return;
    }
    let Some(sid) = v.get("session_id").and_then(Value::as_str) else { return };
    if sid.is_empty() || sid.contains(['/', '\\', '.']) {
        return;
    }
    let dir = questions_dir();
    let _ = std::fs::create_dir_all(&dir);
    let text = if msg.is_empty() { "Claude a besoin de toi." } else { msg };
    let body = json!({ "message": text, "type": kind, "ts": now_ms(), "hwnd": owner_window() });
    let _ = std::fs::write(dir.join(format!("{sid}.json")), body.to_string());
}

unsafe extern "system" fn collect(h: HWND, l: LPARAM) -> BOOL {
    let map = &mut *(l.0 as *mut HashMap<u32, isize>);
    let owned = GetWindow(h, GW_OWNER).is_ok_and(|o| !o.is_invalid());
    if IsWindowVisible(h).as_bool() && !owned && GetWindowTextLengthW(h) > 0 {
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        map.entry(pid).or_insert(h.0 as isize);
    }
    TRUE
}

/// Fenêtre qui héberge la session (Windows Terminal, VS Code…) : premier
/// processus parent du hook qui possède une fenêtre visible.
fn owner_window() -> Option<isize> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut parent = HashMap::new();
        let mut pe = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        if Process32FirstW(snap, &mut pe).is_ok() {
            loop {
                parent.insert(pe.th32ProcessID, pe.th32ParentProcessID);
                if Process32NextW(snap, &mut pe).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
        let mut wins: HashMap<u32, isize> = HashMap::new();
        let _ = EnumWindows(Some(collect), LPARAM(&mut wins as *mut _ as isize));
        let mut pid = GetCurrentProcessId();
        for _ in 0..16 {
            pid = *parent.get(&pid)?;
            if pid == 0 {
                break;
            }
            if let Some(h) = wins.get(&pid) {
                return Some(*h);
            }
        }
        None
    }
}

fn settings_path() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    home.join(".claude").join("settings.json")
}

/// Ajoute le hook (avec copie de sauvegarde settings.json.bak). Renvoie un message.
pub fn install_hook() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let command = format!("\"{}\" hook", exe.display());
    let path = settings_path();
    let mut root: Value = match std::fs::read(&path) {
        Ok(b) => {
            std::fs::write(path.with_extension("json.bak"), &b).map_err(|e| e.to_string())?;
            serde_json::from_slice(&b).map_err(|e| format!("settings.json illisible : {e}"))?
        }
        Err(_) => json!({}),
    };
    let obj = root.as_object_mut().ok_or("settings.json n'est pas un objet JSON")?;
    let hooks = obj.entry("hooks").or_insert_with(|| json!({}));
    let list = hooks
        .as_object_mut()
        .ok_or("« hooks » n'est pas un objet")?
        .entry("Notification")
        .or_insert_with(|| json!([]));
    let arr = list.as_array_mut().ok_or("« hooks.Notification » n'est pas une liste")?;
    let already = arr.iter().any(|e| e.to_string().contains("pokagent"));
    if already {
        return Ok("Le hook PokAgent est déjà installé.".into());
    }
    arr.push(json!({ "matcher": "", "hooks": [{ "type": "command", "command": command }] }));
    let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(format!("Hook ajouté dans {}.\nCopie de l'ancien fichier : settings.json.bak.\nRelance tes sessions Claude Code pour qu'il soit pris en compte.", path.display()))
}
