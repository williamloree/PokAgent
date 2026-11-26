//! Calculateur de limites. Claude Code passe `rate_limits` (fenêtre de 5 h et
//! semaine) à sa statusline : `pokagent.exe statusline` les recopie dans
//! usage.json, puis relaie l'entrée à la statusline d'origine (ccstatusline…).
//! Au passage on apprend combien de % hebdo coûte 1 % de fenêtre 5 h.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};

const H5: f64 = 5.0 * 3600.0;
const WEEK: f64 = 7.0 * 86400.0;

fn dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("PokAgent")
}

fn usage_path() -> PathBuf {
    dir().join("usage.json")
}

/// Statusline d'origine, appelée après nous.
fn next_path() -> PathBuf {
    dir().join("statusline_next.txt")
}

pub fn set_next(cmd: &str) -> Result<(), String> {
    let _ = std::fs::create_dir_all(dir());
    std::fs::write(next_path(), cmd).map_err(|e| e.to_string())
}

#[derive(Clone, Default)]
pub struct Usage {
    pub five: f64,
    pub five_reset: f64,
    pub week: f64,
    pub week_reset: f64,
    /// Hausses cumulées (% hebdo, % 5 h) observées dans les mêmes fenêtres.
    acc_w: f64,
    acc_f: f64,
    /// % hebdo au début de la fenêtre 5 h en cours.
    w_start: f64,
    /// Coût réel (% hebdo) des dernières fenêtres 5 h terminées, pleines ou non.
    sessions: Vec<f64>,
}

const KEEP: usize = 20;

impl Usage {
    fn from_json(v: &Value) -> Option<Usage> {
        let f = |k: &str| v.get(k).and_then(Value::as_f64);
        Some(Usage {
            five: f("five")?,
            five_reset: f("five_reset")?,
            week: f("week")?,
            week_reset: f("week_reset")?,
            acc_w: f("acc_w").unwrap_or(0.0),
            acc_f: f("acc_f").unwrap_or(0.0),
            w_start: f("w_start").unwrap_or(f("week")?),
            sessions: v.get("sessions").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default(),
        })
    }

    fn from_rate_limits(r: &Value) -> Option<Usage> {
        let f = |w: &str, k: &str| r.get(w)?.get(k)?.as_f64();
        Some(Usage {
            five: f("five_hour", "used_percentage")?,
            five_reset: f("five_hour", "resets_at")?,
            week: f("seven_day", "used_percentage")?,
            week_reset: f("seven_day", "resets_at")?,
            ..Default::default()
        })
    }

    pub fn load() -> Option<Usage> {
        let b = std::fs::read(usage_path()).ok()?;
        Usage::from_json(&serde_json::from_slice(&b).ok()?)
    }

    fn save(&self) {
        let _ = std::fs::create_dir_all(dir());
        let v = json!({ "five": self.five, "five_reset": self.five_reset, "week": self.week, "week_reset": self.week_reset, "acc_w": self.acc_w, "acc_f": self.acc_f, "w_start": self.w_start, "sessions": self.sessions });
        let _ = std::fs::write(usage_path(), v.to_string());
    }

    /// Reprend l'historique précédent. Hausse dans les mêmes fenêtres : alimente
    /// le ratio. Nouvelle fenêtre 5 h : la précédente est terminée, on note son coût.
    fn absorb(mut self, prev: Option<Usage>) -> Usage {
        self.w_start = self.week;
        let Some(p) = prev else { return self };
        self.acc_w = p.acc_w;
        self.acc_f = p.acc_f;
        self.sessions = p.sessions;
        let same_five = (p.five_reset - self.five_reset).abs() < 120.0;
        let same_week = (p.week_reset - self.week_reset).abs() < 120.0;
        if same_five && same_week {
            self.w_start = p.w_start;
            if self.five > p.five && self.week >= p.week {
                self.acc_w += self.week - p.week;
                self.acc_f += self.five - p.five;
            }
        } else if !same_five {
            if same_week {
                // rien n'est consommé entre deux fenêtres : la nouvelle part de là
                self.w_start = p.week;
            }
            // shortcut: une fenêtre à cheval sur le reset hebdo est ignorée
            if same_week && p.week >= p.w_start && p.five > 0.0 {
                self.sessions.push(p.week - p.w_start);
                if self.sessions.len() > KEEP {
                    self.sessions.remove(0);
                }
            }
        }
        self
    }

    /// % hebdo consommé par une fenêtre 5 h pleine ; None tant qu'on a trop peu observé.
    fn per_session(&self) -> Option<f64> {
        (self.acc_f >= 10.0 && self.acc_w > 0.0).then(|| self.acc_w / self.acc_f * 100.0)
    }

    /// Coût moyen réel d'une de tes sessions 5 h, en % hebdo.
    fn typical(&self) -> Option<f64> {
        let s: Vec<f64> = self.sessions.iter().copied().filter(|c| *c > 0.0).collect();
        (!s.is_empty()).then(|| s.iter().sum::<f64>() / s.len() as f64)
    }

    pub fn short(&self, now: f64) -> String {
        let five = if now >= self.five_reset { 0.0 } else { self.five };
        let week = if now >= self.week_reset { 0.0 } else { self.week };
        format!("5h {five:.0}% · sem. {week:.0}%")
    }

    /// Lignes du menu de l'icône et du panneau.
    pub fn report(&self, now: f64) -> Vec<String> {
        let mut out = vec![];
        if now >= self.five_reset {
            out.push("Session 5 h : 0 % · fenêtre libre".into());
        } else {
            out.push(format!("Session 5 h : {:.0} % · reset dans {} ({})", self.five, dur(self.five_reset - now), clock(self.five_reset, false)));
        }
        if now >= self.week_reset {
            out.push("Semaine : 0 % · nouvelle semaine".into());
            return out;
        }
        let left = self.week_reset - now;
        out.push(format!("Semaine : {:.0} % · reset dans {} ({})", self.week, dur(left), clock(self.week_reset, true)));
        let elapsed = WEEK - left;
        if elapsed > 6.0 * 3600.0 {
            out.push(format!("Au rythme actuel : ~{:.0} % à la fin de la semaine", (self.week * WEEK / elapsed).min(999.0)));
        }
        let rest = (100.0 - self.week).max(0.0);
        let windows = (left / H5).ceil();
        let per = self.per_session();
        match per {
            Some(per) if now < self.five_reset => out.push(format!("5 h pleine = {per:.0} % hebdo · reste ~{:.0} % dans celle-ci", (100.0 - self.five) * per / 100.0)),
            Some(per) => out.push(format!("1 session 5 h pleine = {per:.0} % de la semaine")),
            None => out.push("Coût d'une 5 h pleine : en apprentissage".into()),
        }
        match self.typical() {
            Some(c) => {
                let n = self.sessions.iter().filter(|c| **c > 0.0).count();
                out.push(format!("Ta session type = {c:.0} % hebdo (sur {n})"));
                let mine = format!("Reste {rest:.0} % = ~{:.1} sessions à ton rythme", rest / c);
                out.push(match per {
                    Some(p) => format!("{mine}, {:.1} pleines", rest / p),
                    None => mine,
                });
            }
            None => {
                out.push("Ta session type : connue à la fin de cette fenêtre".into());
                if let Some(p) = per {
                    out.push(format!("Reste {rest:.0} % = {:.1} sessions 5 h pleines", rest / p));
                }
            }
        }
        match per {
            Some(p) if rest / p > windows => out.push(format!("{windows:.0} fenêtres 5 h restantes : max ~{:.0} % à fond", self.week + windows * p)),
            Some(p) => out.push(format!("{windows:.0} fenêtres 5 h restantes · {:.1} pleine/jour pour 100 %", rest / p / (left / 86400.0).max(1.0 / 24.0))),
            None => out.push(format!("{windows:.0} fenêtres 5 h restantes avant le reset")),
        }
        out
    }
}

fn dur(s: f64) -> String {
    let m = (s / 60.0).round() as i64;
    match m {
        m if m >= 1440 => format!("{} j {} h", m / 1440, m % 1440 / 60),
        m if m >= 60 => format!("{} h {:02}", m / 60, m % 60),
        m => format!("{m} min"),
    }
}

/// Heure locale d'un horodatage Unix (avec le jour pour le reset hebdo).
fn clock(ts: f64, day: bool) -> String {
    let ft = ((ts as i64 + 11_644_473_600) * 10_000_000) as u64;
    let ft = FILETIME { dwLowDateTime: ft as u32, dwHighDateTime: (ft >> 32) as u32 };
    unsafe {
        let (mut utc, mut loc) = Default::default();
        if FileTimeToSystemTime(&ft, &mut utc).is_err() || SystemTimeToTzSpecificLocalTime(None, &utc, &mut loc).is_err() {
            return String::new();
        }
        let hm = format!("{}:{:02}", loc.wHour, loc.wMinute);
        if day {
            let d = ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."][loc.wDayOfWeek as usize % 7];
            format!("{d} {hm}")
        } else {
            hm
        }
    }
}

pub fn now() -> f64 {
    crate::watcher::now_ms() / 1000.0
}

/// `pokagent.exe statusline` : enregistre rate_limits puis passe la main.
pub fn run_statusline() {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let v: Value = serde_json::from_str(&input).unwrap_or(Value::Null);
    let mut own = String::new();
    if let Some(u) = v.get("rate_limits").and_then(Usage::from_rate_limits) {
        // shortcut: plusieurs sessions écrivent sans verrou ; au pire un échantillon perdu
        let u = u.absorb(Usage::load());
        u.save();
        own = u.short(now());
    }
    let next = std::fs::read_to_string(next_path()).unwrap_or_default();
    let next = next.trim();
    if next.is_empty() {
        println!("{own}");
        return;
    }
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let child = Command::new("cmd").arg("/C").raw_arg(next).creation_flags(CREATE_NO_WINDOW).stdin(Stdio::piped()).spawn();
    if let Ok(mut c) = child {
        if let Some(mut si) = c.stdin.take() {
            let _ = si.write_all(input.as_bytes());
        }
        let _ = c.wait();
    } else {
        println!("{own}");
    }
}

pub const UW: i32 = 380; // panneau bas-gauche, pixels natifs
pub const UH: i32 = 110;

/// Panneau du bas-gauche : un titre et les lignes du rapport, calé en bas.
pub fn draw(cv: &mut crate::gfx::Canvas, lines: &[String]) {
    use crate::gfx::{fit, hex};
    use embedded_graphics::mono_font::iso_8859_1::FONT_6X10;
    cv.clear();
    let h = 16 + lines.len() as i32 * 11;
    let y0 = UH - h;
    cv.fill(0, y0, UW, h, hex("#1c1a24").a(210));
    cv.text(6, y0 + 4, "LIMITES CLAUDE", hex("#f0c860"), &FONT_6X10);
    for (i, l) in lines.iter().enumerate() {
        // police ISO-8859-1 : pas de flèche ni de ≈
        let l = l.replace('→', "->").replace('≈', "~");
        cv.text(6, y0 + 15 + i as i32 * 11, &fit(&l, UW - 12, &FONT_6X10), hex("#ece2c8"), &FONT_6X10);
    }
}
