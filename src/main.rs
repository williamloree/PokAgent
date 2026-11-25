// PokAgent : les sessions et agents Claude Code en pixel art, dans le quart
// haut-gauche du fond d'écran ; les questions de Claude dans le quart bas-droit.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod gfx;
mod hook;
mod scene;
#[cfg(test)]
mod shots;
mod sound;
mod sprites;
mod themes;
mod updater;
mod watcher;

use gfx::Canvas;
use scene::{draw_popup, Demo, Scene, NH, NW, PH, PW};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};
use themes::Theme;
use watcher::{Model, Watcher};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_TRAY: u32 = WM_APP + 1;
const WM_RESTART: u32 = WM_APP + 2;
const ID_THEME: usize = 100;
const ID_FPS: usize = 200;
const FPS: [u32; 3] = [2, 8, 20];
const ID_SOUND: usize = 300;
const ID_HOOK: usize = 400;
const ID_AUTOSTART: usize = 500;
const ID_AUTOUPDATE: usize = 600;
const ID_CHECK: usize = 601;
const ID_QUIT: usize = 999;
const UPDATE_EVERY: Duration = Duration::from_secs(6 * 3600);

// ------------------------------------------------------------ réglages
struct Config {
    theme: Theme,
    fps: u32,
    sound: bool,
    /// None tant que l'utilisateur n'a rien choisi : activé au premier lancement.
    autostart: Option<bool>,
    auto_update: bool,
}

fn config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("PokAgent").join("config.json")
}

impl Config {
    fn load() -> Config {
        let v: Value = std::fs::read(config_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
        Config {
            theme: Theme::from_id(v.get("theme").and_then(Value::as_str).unwrap_or("labo")),
            fps: v.get("fps").and_then(Value::as_u64).map(|f| f as u32).filter(|f| FPS.contains(f)).unwrap_or(8),
            sound: v.get("sound").and_then(Value::as_bool).unwrap_or(true),
            autostart: v.get("autostart").and_then(Value::as_bool),
            auto_update: v.get("auto_update").and_then(Value::as_bool).unwrap_or(true),
        }
    }
    fn save(&self) {
        let p = config_path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let v = json!({ "theme": self.theme.id(), "fps": self.fps, "sound": self.sound, "autostart": self.autostart, "auto_update": self.auto_update });
        let _ = std::fs::write(p, serde_json::to_string_pretty(&v).unwrap());
    }
}

// ------------------------------------------------------------ fenêtres transparentes
/// Fenêtre « layered » et son tampon DIB (BGRA prémultiplié).
struct Surface {
    hwnd: HWND,
    memdc: HDC,
    bits: *mut u32,
    w: i32,
    h: i32,
}

impl Surface {
    unsafe fn new(hwnd: HWND, w: i32, h: i32) -> Surface {
        let screen = GetDC(None);
        let memdc = CreateCompatibleDC(screen);
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let bmp = CreateDIBSection(screen, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).expect("CreateDIBSection");
        SelectObject(memdc, bmp);
        ReleaseDC(None, screen);
        Surface { hwnd, memdc, bits: bits as *mut u32, w, h }
    }

    /// Recopie (×scale, sans lissage) les seuls pixels natifs qui diffèrent de
    /// l'image précédente, puis n'envoie à Windows que la zone modifiée.
    unsafe fn present(&self, cv: &Canvas, old: &Canvas, scale: i32) {
        let bits = std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize);
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, -1, -1);
        for ny in 0..cv.h {
            for nx in 0..cv.w {
                let i = (ny * cv.w + nx) as usize;
                let c = cv.px[i];
                if c == old.px[i] {
                    continue;
                }
                (x0, y0, x1, y1) = (x0.min(nx), y0.min(ny), x1.max(nx), y1.max(ny));
                let a = c.3 as u32;
                let v = (a << 24) | ((c.0 as u32 * a / 255) << 16) | ((c.1 as u32 * a / 255) << 8) | (c.2 as u32 * a / 255);
                for dy in 0..scale {
                    let row = ((ny * scale + dy) * self.w + nx * scale) as usize;
                    bits[row..row + scale as usize].fill(v);
                }
            }
        }
        if x1 < 0 {
            return;
        }
        let size = SIZE { cx: self.w, cy: self.h };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
        let dirty = RECT { left: x0 * scale, top: y0 * scale, right: (x1 + 1) * scale, bottom: (y1 + 1) * scale };
        let info = UPDATELAYEREDWINDOWINFO {
            cbSize: std::mem::size_of::<UPDATELAYEREDWINDOWINFO>() as u32,
            psize: &size,
            hdcSrc: self.memdc,
            pptSrc: &src,
            pblend: &blend,
            dwFlags: ULW_ALPHA,
            prcDirty: &dirty,
            ..Default::default()
        };
        let _ = UpdateLayeredWindowIndirect(self.hwnd, &info);
    }
}

unsafe fn work_area() -> RECT {
    let mut r = RECT::default();
    let _ = SystemParametersInfoW(SPI_GETWORKAREA, 0, Some(&mut r as *mut RECT as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
    r
}

/// Une application plein écran au premier plan : inutile de dessiner.
unsafe fn fullscreen_app(sw: i32, sh: i32) -> bool {
    let fg = GetForegroundWindow();
    if fg.is_invalid() {
        return false;
    }
    let mut cls = [0u16; 64];
    let n = GetClassNameW(fg, &mut cls);
    let name = String::from_utf16_lossy(&cls[..n.max(0) as usize]);
    if name == "Progman" || name == "WorkerW" {
        return false;
    }
    let mut r = RECT::default();
    GetWindowRect(fg, &mut r).is_ok() && r.left <= 0 && r.top <= 0 && r.right >= sw && r.bottom >= sh
}

// ------------------------------------------------------------ application
struct App {
    cfg: Config,
    scene: Scene,
    demo: Option<Demo>,
    rx: Option<Receiver<Model>>,
    view: Surface,
    popup: Surface,
    ctl: HWND,
    scale: i32,
    canvas: Canvas,
    prev: Canvas,
    pcanvas: Canvas,
    pprev: Canvas,
    popup_key: Option<String>,
    popup_since: f64,
    dismissed: HashSet<String>,
    seen: HashSet<String>,
    jingle: Vec<u8>,
    start: Instant,
    last_t: f64,
    screen: (i32, i32),
    tray: NOTIFYICONDATAW,
    auto_update: Arc<AtomicBool>,
}

static mut MUTEX: HANDLE = HANDLE(std::ptr::null_mut());

/// Cherche une mise à jour ; si elle est installée, demande la relance.
/// `manual` : l'utilisateur a demandé, on lui répond dans tous les cas.
fn run_update_check(ctl: isize, manual: bool) {
    match updater::check_and_install(manual) {
        Ok(updater::Outcome::Installed(tag)) => unsafe {
            if manual {
                message(&format!("PokAgent {tag} est installé. Le programme redémarre."));
            }
            let _ = PostMessageW(HWND(ctl as *mut _), WM_RESTART, WPARAM(0), LPARAM(0));
        },
        Ok(updater::Outcome::UpToDate) if manual => message(&format!("PokAgent est à jour (v{}).", updater::VERSION)),
        Err(e) if manual => message(&format!("Impossible de vérifier les mises à jour : {e}")),
        _ => {}
    }
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

impl App {
    fn tick(&mut self) {
        let t = self.start.elapsed().as_secs_f64();
        let dt = (t - self.last_t).min(0.5) as f32;
        self.last_t = t;
        if let Some(rx) = &self.rx {
            let mut latest = None;
            while let Ok(m) = rx.try_recv() {
                latest = Some(m);
            }
            if let Some(m) = latest {
                self.scene.sync(&m, t);
            }
        }
        if let Some(d) = &mut self.demo {
            if let Some(m) = d.step(t) {
                let m = m.clone();
                self.scene.sync(&m, t);
            }
        }
        unsafe {
            if fullscreen_app(self.screen.0, self.screen.1) {
                return;
            }
        }
        self.scene.update(dt);
        self.scene.draw(&mut self.canvas, t);
        if self.canvas.px != self.prev.px {
            unsafe { self.view.present(&self.canvas, &self.prev, self.scale) };
            std::mem::swap(&mut self.canvas, &mut self.prev);
        }
        self.update_popup(t);
    }

    fn update_popup(&mut self, t: f64) {
        let active = self.scene.question.as_ref().filter(|q| !self.dismissed.contains(&q.key)).map(|q| q.key.clone());
        if active != self.popup_key {
            unsafe {
                match &active {
                    Some(k) => {
                        self.popup_since = t;
                        let _ = SetWindowPos(self.popup.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
                        if self.cfg.sound && self.seen.insert(k.clone()) {
                            let _ = PlaySoundW(PCWSTR(self.jingle.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
                        }
                    }
                    None => {
                        let _ = ShowWindow(self.popup.hwnd, SW_HIDE);
                    }
                }
            }
            self.popup_key = active;
            self.pprev.invalidate();
        }
        if self.popup_key.is_none() {
            return;
        }
        let slide = (((t - self.popup_since) / 0.4).min(1.0) * 5.0).floor() as f32 / 5.0;
        let portrait = self.scene.portrait();
        if let Some(q) = &self.scene.question {
            draw_popup(&mut self.pcanvas, self.scene.theme, portrait.as_ref(), q, t, slide);
        }
        if self.pcanvas.px != self.pprev.px {
            unsafe { self.popup.present(&self.pcanvas, &self.pprev, self.scale) };
            std::mem::swap(&mut self.pcanvas, &mut self.pprev);
        }
    }

    /// Clic sur la bulle : on la ferme et on ramène la fenêtre de la session
    /// (terminal, VS Code…) si le hook l'a fournie.
    fn dismiss(&mut self) {
        if let Some(k) = self.popup_key.clone() {
            self.dismissed.insert(k);
        }
        let focus = self.scene.question.as_ref().and_then(|q| q.focus);
        if let Some(h) = focus {
            unsafe {
                let h = HWND(h as *mut _);
                if IsWindow(h).as_bool() {
                    if IsIconic(h).as_bool() {
                        let _ = ShowWindow(h, SW_RESTORE);
                    }
                    let _ = SetForegroundWindow(h);
                }
            }
        }
    }

    fn set_theme(&mut self, th: Theme) {
        self.cfg.theme = th;
        self.scene.theme = th;
        self.scene.drop_other_looks();
        self.jingle = sound::jingle(th);
        self.prev.invalidate();
        self.pprev.invalidate();
        self.cfg.save();
    }

    fn set_fps(&mut self, fps: u32) {
        self.cfg.fps = fps;
        unsafe { SetTimer(self.ctl, 1, 1000 / fps, None) };
        self.cfg.save();
    }
}

// ------------------------------------------------------------ procédures de fenêtre
extern "system" fn view_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCHITTEST {
            return LRESULT(HTTRANSPARENT as isize);
        }
        // Toujours tout en bas de la pile : juste au-dessus du fond d'écran.
        if msg == WM_WINDOWPOSCHANGING {
            let pos = &mut *(lp.0 as *mut WINDOWPOS);
            pos.hwndInsertAfter = HWND_BOTTOM;
            pos.flags &= !SWP_NOZORDER;
        }
        DefWindowProcW(h, msg, wp, lp)
    }
}

extern "system" fn popup_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_LBUTTONUP | WM_RBUTTONUP => {
                with_app(|a| a.dismiss());
                LRESULT(0)
            }
            WM_SETCURSOR => {
                if let Ok(c) = LoadCursorW(None, IDC_HAND) {
                    SetCursor(c);
                }
                LRESULT(1)
            }
            _ => DefWindowProcW(h, msg, wp, lp),
        }
    }
}

static mut TASKBAR_CREATED: u32 = 0;

unsafe fn tray_menu(ctl: HWND) {
    let Some((theme, fps, snd, upd, demo)) = with_app(|a| (a.cfg.theme, a.cfg.fps, a.cfg.sound, a.cfg.auto_update, a.demo.is_some())) else { return };
    let auto = autostart::is_on();
    let Ok(menu) = CreatePopupMenu() else { return };
    let item = |id: usize, text: &str, checked: bool| {
        let t: Vec<u16> = text.encode_utf16().chain([0]).collect();
        let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
        let _ = AppendMenuW(menu, flags, id, PCWSTR(t.as_ptr()));
    };
    let title: Vec<u16> = format!("PokAgent v{}{}", updater::VERSION, if demo { " (démo)" } else { "" }).encode_utf16().chain([0]).collect();
    let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, PCWSTR(title.as_ptr()));
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
    for (i, th) in Theme::ALL.iter().enumerate() {
        item(ID_THEME + i, th.label(), *th == theme);
    }
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
    for (i, f) in FPS.iter().enumerate() {
        item(ID_FPS + i, &format!("{f} images par seconde"), *f == fps);
    }
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
    item(ID_SOUND, "Son des questions", snd);
    item(ID_HOOK, "Installer le hook des questions…", false);
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
    item(ID_AUTOSTART, "Lancer avec Windows", auto);
    item(ID_AUTOUPDATE, "Mises à jour automatiques", upd);
    item(ID_CHECK, "Rechercher une mise à jour", false);
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
    item(ID_QUIT, "Quitter PokAgent", false);
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let _ = SetForegroundWindow(ctl);
    let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, pt.x, pt.y, 0, ctl, None).0 as usize;
    let _ = DestroyMenu(menu);
    match cmd {
        c if (ID_THEME..ID_THEME + 3).contains(&c) => {
            with_app(|a| a.set_theme(Theme::ALL[c - ID_THEME]));
        }
        c if (ID_FPS..ID_FPS + 3).contains(&c) => {
            with_app(|a| a.set_fps(FPS[c - ID_FPS]));
        }
        ID_SOUND => {
            with_app(|a| {
                a.cfg.sound = !a.cfg.sound;
                a.cfg.save();
            });
        }
        ID_HOOK => {
            let ok = MessageBoxW(
                None,
                w!("PokAgent va ajouter un hook « Notification » dans %USERPROFILE%\\.claude\\settings.json (une copie settings.json.bak est faite avant). Il permet de savoir quand une session attend ta réponse.\n\nContinuer ?"),
                w!("PokAgent"),
                MB_OKCANCEL | MB_ICONQUESTION,
            );
            if ok == IDOK {
                let msg = match hook::install_hook() {
                    Ok(m) => m,
                    Err(e) => format!("Échec : {e}"),
                };
                message(&msg);
            }
        }
        ID_AUTOSTART => {
            if let Err(e) = autostart::set(!auto) {
                message(&format!("Échec : {e}"));
            }
            with_app(|a| {
                a.cfg.autostart = Some(!auto);
                a.cfg.save();
            });
        }
        ID_AUTOUPDATE => {
            with_app(|a| {
                a.cfg.auto_update = !upd;
                a.auto_update.store(!upd, Ordering::Relaxed);
                a.cfg.save();
            });
        }
        ID_CHECK => {
            let ctl = ctl.0 as isize;
            std::thread::spawn(move || run_update_check(ctl, true));
        }
        ID_QUIT => PostQuitMessage(0),
        _ => {}
    }
}

/// Relance l'exécutable (fraîchement mis à jour) puis quitte.
unsafe fn restart() {
    let _ = CloseHandle(MUTEX);
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
    PostQuitMessage(0);
}

unsafe fn add_tray(nid: &NOTIFYICONDATAW) {
    let _ = Shell_NotifyIconW(NIM_ADD, nid);
}

extern "system" fn ctl_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_TIMER => {
                with_app(|a| a.tick());
                LRESULT(0)
            }
            WM_TRAY => {
                let ev = (lp.0 & 0xffff) as u32;
                if ev == WM_RBUTTONUP || ev == WM_LBUTTONUP {
                    tray_menu(h);
                }
                LRESULT(0)
            }
            WM_RESTART => {
                restart();
                LRESULT(0)
            }
            m if m != 0 && m == TASKBAR_CREATED => {
                // L'explorateur a redémarré : l'icône de notification est à refaire.
                with_app(|a| add_tray(&a.tray));
                LRESULT(0)
            }
            _ => DefWindowProcW(h, msg, wp, lp),
        }
    }
}

fn message(text: &str) {
    let t: Vec<u16> = text.encode_utf16().chain([0]).collect();
    unsafe {
        MessageBoxW(None, PCWSTR(t.as_ptr()), w!("PokAgent"), MB_OK);
    }
}

unsafe fn register(name: PCWSTR, proc_: WNDPROC, hinst: HINSTANCE) {
    let wc = WNDCLASSW { lpfnWndProc: proc_, hInstance: hinst, lpszClassName: name, hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(), ..Default::default() };
    RegisterClassW(&wc);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("hook") => return hook::run_hook(),
        // Mise à jour à la demande. Code de sortie : 0 installée, 1 déjà à jour, 2 erreur.
        Some("update") => {
            let quiet = args.iter().any(|a| a == "--quiet");
            let (code, text) = match updater::check_and_install(true) {
                Ok(updater::Outcome::Installed(tag)) => (0, format!("PokAgent {tag} est installé. Relance PokAgent pour l'utiliser.")),
                Ok(updater::Outcome::UpToDate) => (1, format!("PokAgent est à jour (v{}).", updater::VERSION)),
                Err(e) => (2, format!("Impossible de mettre à jour : {e}")),
            };
            if !quiet {
                message(&text);
            }
            std::process::exit(code);
        }
        Some("install-hook") => {
            match hook::install_hook() {
                Ok(m) => message(&m),
                Err(e) => message(&format!("Échec : {e}")),
            }
            return;
        }
        _ => {}
    }
    let demo = args.iter().any(|a| a == "--demo");
    let mut cfg = Config::load();

    unsafe {
        // Une seule instance. Juste après une mise à jour, l'ancienne instance
        // peut encore tenir le verrou un instant : on patiente un peu.
        let mut tries = 0;
        loop {
            match CreateMutexW(None, true, w!("Local\\PokAgentSingleton")) {
                Ok(h) if GetLastError() != ERROR_ALREADY_EXISTS => {
                    MUTEX = h;
                    break;
                }
                Ok(h) => {
                    let _ = CloseHandle(h);
                }
                Err(_) => {}
            }
            tries += 1;
            if tries > 20 {
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        updater::cleanup();
        if !demo && cfg.autostart.is_none() {
            cfg.autostart = Some(autostart::set(true).is_ok());
            cfg.save();
        }
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        TASKBAR_CREATED = RegisterWindowMessageW(w!("TaskbarCreated"));
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        register(w!("PokAgentView"), Some(view_proc), hinst);
        register(w!("PokAgentPopup"), Some(popup_proc), hinst);
        register(w!("PokAgentCtl"), Some(ctl_proc), hinst);

        // taille : le quart de l'écran principal, en multiple entier des pixels natifs
        let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        let scale = ((sw / 2) / NW).min((sh / 2) / NH).max(1);
        let (vw, vh) = (NW * scale, NH * scale);
        let wa = work_area();

        let ctl = CreateWindowExW(WINDOW_EX_STYLE(0), w!("PokAgentCtl"), w!("PokAgent"), WINDOW_STYLE(0), 0, 0, 0, 0, HWND_MESSAGE, None, hinst, None).unwrap();
        let view_hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("PokAgentView"),
            w!("PokAgent"),
            WS_POPUP,
            wa.left,
            wa.top,
            vw,
            vh,
            None,
            None,
            hinst,
            None,
        )
        .unwrap();
        let (pw, ph) = (PW * scale, PH * scale);
        let popup_hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("PokAgentPopup"),
            w!("PokAgent — question"),
            WS_POPUP,
            wa.right - pw - 16,
            wa.bottom - ph - 16,
            pw,
            ph,
            None,
            None,
            hinst,
            None,
        )
        .unwrap();

        // Sous toutes les fenêtres, au-dessus du fond d'écran (voir view_proc).
        let _ = SetWindowPos(view_hwnd, HWND_BOTTOM, wa.left, wa.top, vw, vh, SWP_NOACTIVATE | SWP_SHOWWINDOW);

        let mut tray = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: ctl,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
            ..Default::default()
        };
        for (i, c) in "PokAgent".encode_utf16().enumerate() {
            tray.szTip[i] = c;
        }
        add_tray(&tray);

        let (rx, demo) = if demo {
            (None, Some(Demo::new(0.0)))
        } else {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let mut w = Watcher::new();
                let mut last: Option<Model> = None;
                loop {
                    w.poll();
                    let m = w.snapshot();
                    if last.as_ref() != Some(&m) {
                        if tx.send(m.clone()).is_err() {
                            break;
                        }
                        last = Some(m);
                    }
                    std::thread::sleep(Duration::from_millis(1000));
                }
            });
            (Some(rx), None)
        };

        // Mises à jour : une minute après le lancement, puis toutes les 6 h.
        let auto_update = Arc::new(AtomicBool::new(cfg.auto_update));
        if demo.is_none() && !cfg!(debug_assertions) {
            let flag = auto_update.clone();
            let ctl_id = ctl.0 as isize;
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(60));
                loop {
                    if flag.load(Ordering::Relaxed) {
                        run_update_check(ctl_id, false);
                    }
                    std::thread::sleep(UPDATE_EVERY);
                }
            });
        }

        let mut scene = Scene::new(cfg.theme);
        if let Some(d) = &demo {
            scene.sync(d.model(), -10.0);
        }
        let app = App {
            jingle: sound::jingle(cfg.theme),
            scene,
            demo,
            rx,
            view: Surface::new(view_hwnd, vw, vh),
            popup: Surface::new(popup_hwnd, pw, ph),
            ctl,
            scale,
            canvas: Canvas::new(NW, NH),
            prev: Canvas::new(NW, NH),
            pcanvas: Canvas::new(PW, PH),
            pprev: Canvas::new(PW, PH),
            popup_key: None,
            popup_since: 0.0,
            dismissed: HashSet::new(),
            seen: HashSet::new(),
            start: Instant::now(),
            last_t: 0.0,
            screen: (sw, sh),
            tray,
            auto_update,
            cfg,
        };
        let fps = app.cfg.fps;
        APP.with(|a| *a.borrow_mut() = Some(app));
        SetTimer(ctl, 1, 1000 / fps, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        with_app(|a| {
            let _ = Shell_NotifyIconW(NIM_DELETE, &a.tray);
            let _ = DestroyWindow(a.view.hwnd);
            let _ = DestroyWindow(a.popup.hwnd);
        });
    }
}
