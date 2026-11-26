//! La scène : une salle par session (Claude) puis une par agent, rangées dans une
//! grille 4 × 3. Les personnages se promènent ; décors et atlas sont mis en cache.

use crate::gfx::{fit, hex, rgba, text_width, wrap, Canvas, Rng, C};
use crate::sprites::{agent_pal, atlas, bubble, claude_pal, Atlas, Pal, DOWN, LEFT, RIGHT, UP};
use crate::themes::{Mode, Theme, RH, RW};
use crate::watcher::{MAgent, MSession, Model, St};
use embedded_graphics::mono_font::iso_8859_1::{FONT_4X6, FONT_6X10};

pub const COLS: i32 = 4;
pub const ROWS: i32 = 3;
pub const NW: i32 = RW * COLS; // 480
pub const NH: i32 = 270;
pub const PW: i32 = 300; // popup, pixels natifs
pub const PH: i32 = 72;

struct Look {
    bg: Canvas,
    spr: Atlas,
}

struct Walker {
    x: f32,
    y: f32,
    tx: f32,
    ty: f32,
    dir: usize,
    dist: f32,
    wait: f32,
}

struct Room {
    key: String,
    hub: bool,
    name: String,
    mode: Mode,
    question: bool,
    pal: Pal,
    seed: u32,
    looks: [Option<Look>; 3],
    w: Walker,
    born: f64,
    focus: Option<isize>,
}

/// Une ligne de la liste des agents (menu de l'icône).
pub struct RosterItem {
    pub hub: bool,
    pub name: String,
    pub mode: Mode,
    pub question: bool,
    /// Tête et buste du personnage, pour reconnaître sa salle.
    pub face: Canvas,
    /// false si la salle est « hors cadre » (au-delà de 12).
    pub shown: bool,
    pub focus: Option<isize>,
}

pub struct Question {
    pub key: String,
    pub project: String,
    pub text: String,
    pub focus: Option<isize>,
}

pub struct Scene {
    pub theme: Theme,
    rooms: Vec<Room>,
    rng: Rng,
    bubble: Canvas,
    pub question: Option<Question>,
}

fn hash(s: &str) -> u32 {
    s.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))
}

impl Scene {
    pub fn new(theme: Theme) -> Self {
        Scene { theme, rooms: vec![], rng: Rng::new(12345), bubble: bubble(), question: None }
    }

    fn walker(&mut self) -> Walker {
        let [x0, x1, y0, y1] = self.theme.walk();
        let x = x0 + self.rng.f() * (x1 - x0);
        let y = y0 + self.rng.f() * (y1 - y0);
        Walker { x: x.round(), y: y.round(), tx: x, ty: y, dir: DOWN, dist: 0.0, wait: 0.3 + self.rng.f() * 1.7 }
    }

    /// Met la scène en accord avec l'état lu dans les journaux.
    pub fn sync(&mut self, m: &Model, t: f64) {
        let mut old: Vec<Room> = std::mem::take(&mut self.rooms);
        let mut take = |key: &str| old.iter().position(|r| r.key == key).map(|i| old.swap_remove(i));
        let mut rooms = vec![];
        for s in m {
            let key = format!("s:{}", s.id);
            let mut r = match take(&key) {
                Some(r) => r,
                None => {
                    let w = self.walker();
                    Room { key, hub: true, name: String::new(), mode: Mode::Idle, question: false, pal: claude_pal(), seed: hash(&s.id), looks: [None, None, None], w, born: t, focus: None }
                }
            };
            r.name = s.name.clone();
            let running = s.agents.iter().any(|a| a.status == St::Running);
            r.mode = if s.busy || running { Mode::Run } else { Mode::Idle };
            r.question = s.question.is_some();
            r.focus = s.focus;
            rooms.push(r);
            for a in &s.agents {
                let mut r = match take(&a.id) {
                    Some(r) => r,
                    None => {
                        let seed = hash(&a.id);
                        let w = self.walker();
                        Room { key: a.id.clone(), hub: false, name: String::new(), mode: Mode::Run, question: false, pal: agent_pal(seed), seed, looks: [None, None, None], w, born: t, focus: None }
                    }
                };
                r.name = a.name.clone();
                r.focus = s.focus;
                r.mode = match a.status {
                    St::Running => Mode::Run,
                    St::Failed => Mode::Failed,
                    _ => Mode::Done,
                };
                rooms.push(r);
            }
        }
        self.rooms = rooms;
        self.question = m.iter().find_map(|s| {
            s.question.as_ref().map(|q| Question { key: format!("{}|{}", s.id, q), project: s.name.clone(), text: q.clone(), focus: s.focus })
        });
    }

    pub fn update(&mut self, dt: f32) {
        let [x0, x1, y0, y1] = self.theme.walk();
        let [sx, sy] = self.theme.spot();
        for r in &mut self.rooms {
            let c = &mut r.w;
            if r.question || (!r.hub && r.mode != Mode::Run) {
                c.dir = DOWN;
                c.wait = 0.5;
                continue;
            }
            if c.wait > 0.0 {
                c.wait -= dt;
                continue;
            }
            let (dx, dy, step) = (c.tx - c.x, c.ty - c.y, 24.0 * dt);
            if dx.abs() > 0.5 {
                let m = dx.signum() * dx.abs().min(step);
                c.x += m;
                c.dist += m.abs();
                c.dir = if dx > 0.0 { RIGHT } else { LEFT };
            } else if dy.abs() > 0.5 {
                let m = dy.signum() * dy.abs().min(step);
                c.y += m;
                c.dist += m.abs();
                c.dir = if dy > 0.0 { DOWN } else { UP };
            } else {
                if c.tx == sx && c.ty == sy {
                    c.dir = UP;
                }
                c.wait = 1.0 + self.rng.f() * 2.5;
                if self.rng.f() < 0.3 {
                    (c.tx, c.ty) = (sx, sy);
                } else {
                    c.tx = (x0 + self.rng.f() * (x1 - x0)).round();
                    c.ty = (y0 + self.rng.f() * (y1 - y0)).round();
                }
            }
        }
    }

    fn look(theme: Theme, r: &mut Room) -> &Look {
        let i = theme.index();
        if r.looks[i].is_none() {
            let pal = r.pal.clone();
            r.looks[i] = Some(Look { bg: theme.bg(r.hub, r.seed, &r.pal), spr: atlas(|d, f| theme.character(&pal, d, f)) });
        }
        r.looks[i].as_ref().unwrap()
    }

    /// Libère les décors des autres univers (après un changement d'univers).
    pub fn drop_other_looks(&mut self) {
        let i = self.theme.index();
        for r in &mut self.rooms {
            for (j, l) in r.looks.iter_mut().enumerate() {
                if j != i {
                    *l = None;
                }
            }
        }
    }

    /// Sessions et agents dans l'ordre des salles, avec la tête de chaque personnage.
    pub fn roster(&mut self) -> Vec<RosterItem> {
        let theme = self.theme;
        let cap = (COLS * ROWS) as usize;
        self.rooms
            .iter_mut()
            .enumerate()
            .map(|(i, r)| {
                let spr = &Self::look(theme, r).spr[DOWN][0];
                let mut face = Canvas::new(spr.w, 18);
                face.blit(spr, 0, 0);
                RosterItem { hub: r.hub, name: r.name.clone(), mode: r.mode, question: r.question, face, shown: i < cap, focus: r.focus }
            })
            .collect()
    }

    /// Bilan court pour l'infobulle de l'icône.
    pub fn summary(&self) -> String {
        let sessions = self.rooms.iter().filter(|r| r.hub).count();
        let running = self.rooms.iter().filter(|r| !r.hub && r.mode == Mode::Run).count();
        let questions = self.rooms.iter().filter(|r| r.hub && r.question).count();
        let mut s = format!("PokAgent · {sessions} session{} · {running} agent{} en cours", if sessions > 1 { "s" } else { "" }, if running > 1 { "s" } else { "" });
        if questions > 0 {
            s.push_str(&format!(" · {questions} question{}", if questions > 1 { "s" } else { "" }));
        }
        s
    }

    pub fn portrait(&mut self) -> Option<Canvas> {
        let theme = self.theme;
        let r = self.rooms.iter_mut().find(|r| r.hub && r.question)?;
        Some(Self::look(theme, r).spr[DOWN][0].clone())
    }

    pub fn draw(&mut self, cv: &mut Canvas, t: f64) {
        let theme = self.theme;
        cv.clear();
        let cap = (COLS * ROWS) as usize;
        let hidden = self.rooms.len().saturating_sub(cap);
        let pos = |i: usize| ((i as i32 % COLS) * RW, (i as i32 / COLS) * RH);
        let blink = (t * 2.0) as i64 % 2 == 0;
        let n = self.rooms.len().min(cap);
        for i in 0..n {
            let (x, y) = pos(i);
            let r = &mut self.rooms[i];
            let (hub, seed, mode) = (r.hub, r.seed, r.mode);
            cv.blit(&Self::look(theme, r).bg, x, y);
            theme.status(cv, mode, x, y, t, hub, seed);
        }
        for i in 0..n.saturating_sub(1) {
            let same = !self.rooms[i + 1].hub;
            if same && (i as i32 + 1) % COLS != 0 {
                let (x, y) = pos(i);
                theme.door(cv, x + RW, y);
            }
        }
        let (plate_bg, plate_fg) = theme.plate();
        for i in 0..n {
            let (rx, ry) = pos(i);
            let r = &mut self.rooms[i];
            let still = r.w.wait > 0.0 || r.question || (!r.hub && r.mode != Mode::Run);
            let f = if still { 0 } else { [1, 0, 2, 0][(r.w.dist / 5.0) as usize % 4] };
            let (wx, wy, dir) = (r.w.x.round() as i32, r.w.y.round() as i32, r.w.dir);
            let x = rx + wx - 1;
            let y = ry + wy - 1 - if f > 0 { 1 } else { 0 };
            let fy = ry + wy + 21;
            cv.fill(x + 4, fy, 10, 3, rgba(20, 10, 10, 0.28));
            cv.fill(x + 3, fy + 1, 12, 1, rgba(20, 10, 10, 0.28));
            let (question, hub, born) = (r.question, r.hub, r.born);
            let label = if hub { format!("CLAUDE · {}", r.name) } else { r.name.clone() };
            cv.blit(&Self::look(theme, r).spr[dir][f], x, y);
            if question && blink {
                cv.blit(&self.bubble, x + 3, y - 15);
            }
            let label = fit(&label, RW - 16, &FONT_4X6);
            let tw = text_width(&label, &FONT_4X6);
            cv.fill(rx + 5, ry + RH - 13, tw + 6, 9, plate_bg);
            cv.fill(rx + 5, ry + RH - 13, tw + 6, 1, rgba(255, 255, 255, 0.15));
            cv.text(rx + 8, ry + RH - 11, &label, plate_fg, &FONT_4X6);
            let age = t - born;
            if age < 0.6 {
                let a = 1.0 - (age / 0.15).floor() as f32 * 0.25;
                cv.fill(rx, ry, RW, RH, rgba(8, 8, 16, a));
            }
        }
        if hidden > 0 {
            let (x, y) = pos(cap - 1);
            let txt = format!("+{hidden} hors cadre");
            let tw = text_width(&txt, &FONT_4X6);
            cv.fill(x + RW - tw - 12, y + 4, tw + 8, 9, hex("#e04848"));
            cv.text(x + RW - tw - 8, y + 6, &txt, hex("#ffffff"), &FONT_4X6);
        }
    }
}

/// Boîte de dialogue de question, au style de l'univers. `slide` va de 0 à 1.
pub fn draw_popup(cv: &mut Canvas, theme: Theme, portrait: Option<&Canvas>, q: &Question, t: f64, slide: f32) {
    cv.clear();
    let off = ((1.0 - slide) * PH as f32).round() as i32;
    let mut c = Canvas::new(PW, PH);
    // portrait ×2
    if let Some(p) = portrait {
        for y in 0..p.h {
            for x in 0..p.w {
                let col = p.px[(y * p.w + x) as usize];
                if col.3 > 0 {
                    c.fill(4 + x * 2, PH - 2 - p.h * 2 + y * 2, 2, 2, col);
                }
            }
        }
    }
    let (bx, by, bw, bh) = (46, 4, PW - 50, PH - 8);
    let (bg, fg, title, shadow): (C, C, C, Option<C>) = match theme {
        Theme::Labo => {
            c.blob(bx, by, bw, bh, hex("#585868"));
            c.blob(bx + 2, by + 2, bw - 4, bh - 4, hex("#ffffff"));
            c.blob(bx + 4, by + 4, bw - 8, bh - 8, hex("#d8dce8"));
            c.blob(bx + 5, by + 5, bw - 10, bh - 10, hex("#ffffff"));
            (hex("#ffffff"), hex("#505058"), hex("#e05048"), Some(hex("#d0d0dc")))
        }
        Theme::Village => {
            c.blob(bx, by, bw, bh, hex("#eadbb4"));
            c.fill(bx + 3, by + 1, bw - 6, bh - 2, hex("#fff8e6"));
            c.fill(bx + 1, by + 3, bw - 2, bh - 6, hex("#fff8e6"));
            c.fill(bx + 2, by + 2, bw - 4, bh - 4, hex("#fff8e6"));
            (hex("#fff8e6"), hex("#6a4a30"), hex("#e07a30"), None)
        }
        Theme::Donjon => {
            c.fill(bx, by, bw, bh, hex("#c8a050"));
            c.fill(bx + 2, by + 2, bw - 4, bh - 4, hex("#1c1a24"));
            c.fill(bx + 3, by + 3, bw - 6, bh - 6, hex("#6a5a38"));
            c.grad_v(bx + 4, by + 4, bw - 8, bh - 8, (by + 4) as f32, (by + bh - 4) as f32, hex("#2c2836"), hex("#1c1a24"));
            (hex("#2c2836"), hex("#ece2c8"), hex("#f0c860"), Some(hex("#000000")))
        }
    };
    let _ = bg;
    let write = |c: &mut Canvas, x: i32, y: i32, s: &str, col: C| {
        if let Some(sh) = shadow {
            c.text(x + 1, y + 1, s, sh, &FONT_6X10);
        }
        c.text(x, y, s, col, &FONT_6X10);
    };
    let tx = bx + 9;
    let cols = ((bw - 18) / 6) as usize;
    write(&mut c, tx, by + 7, &fit(&format!("CLAUDE · {}", q.project), bw - 18, &FONT_6X10), title);
    for (i, line) in wrap(&q.text, cols).iter().take(4).enumerate() {
        write(&mut c, tx, by + 19 + i as i32 * 10, line, fg);
    }
    if (t * 2.5) as i64 % 2 == 0 {
        let (ax, ay) = (bx + bw - 12, by + bh - 9);
        for k in 0..3 {
            c.fill(ax + k, ay + k, 5 - 2 * k, 1, title);
        }
    }
    for y in 0..PH - off {
        for x in 0..PW {
            let p = c.px[(y * PW + x) as usize];
            if p.3 > 0 {
                cv.put(x, y + off, p);
            }
        }
    }
}

// ------------------------------------------------------------ démonstration
/// Sessions et agents fictifs, pour essayer les univers sans Claude.
pub struct Demo {
    model: Model,
    rng: Rng,
    next: f64,
    uid: u32,
}

impl Demo {
    pub fn new(t: f64) -> Self {
        let mut d = Demo { model: vec![], rng: Rng::new(777), next: t + 2.5, uid: 0 };
        d.add_session("ClaudeIsles");
        d.add_agent(0);
        d.add_agent(0);
        d.model[0].agents[1].status = St::Done;
        d.add_session("PokAgent");
        d.add_agent(1);
        d.model[1].question = Some("Claude a besoin de ta permission pour utiliser Bash.".into());
        d
    }
    fn add_session(&mut self, name: &str) {
        self.uid += 1;
        self.model.push(MSession { id: format!("demo{}", self.uid), name: name.into(), started: self.uid as f64, busy: true, question: None, focus: None, agents: vec![] });
    }
    fn add_agent(&mut self, s: usize) {
        const NAMES: [&str; 10] = ["explore-api", "refacto-ui", "tests-e2e", "doc-readme", "fix-watcher", "build-rust", "audit-deps", "sprites-16px", "port-sse", "bench-cpu"];
        self.uid += 1;
        let name = NAMES[self.uid as usize % NAMES.len()].to_string();
        self.model[s].agents.push(MAgent { id: format!("a{}", self.uid), name, status: St::Running, started: self.uid as f64 });
    }
    /// Fait avancer la démo ; renvoie le modèle s'il a changé.
    pub fn step(&mut self, t: f64) -> Option<&Model> {
        if t < self.next {
            return None;
        }
        self.next = t + 3.5;
        let x = self.rng.f();
        let n: usize = self.model.iter().map(|s| s.agents.len()).sum();
        let running: Vec<(usize, usize)> = self
            .model
            .iter()
            .enumerate()
            .flat_map(|(i, s)| s.agents.iter().enumerate().filter(|(_, a)| a.status == St::Running).map(move |(j, _)| (i, j)))
            .collect();
        for s in &mut self.model {
            s.agents.retain(|a| a.status == St::Running || self.rng.f() < 0.75);
        }
        if x < 0.4 && n < 12 {
            let s = self.rng.i(self.model.len() as i32) as usize;
            self.add_agent(s);
        } else if x < 0.62 && !running.is_empty() {
            let (i, j) = running[self.rng.i(running.len() as i32) as usize];
            if let Some(a) = self.model[i].agents.get_mut(j) {
                a.status = St::Done;
            }
        } else if x < 0.7 && !running.is_empty() {
            let (i, j) = running[self.rng.i(running.len() as i32) as usize];
            if let Some(a) = self.model[i].agents.get_mut(j) {
                a.status = St::Failed;
            }
        } else if x < 0.84 {
            let s = self.rng.i(self.model.len() as i32) as usize;
            let qs = ["Je peux lancer npm install dans ce dossier ?", "Claude a besoin de ta permission pour utiliser Bash.", "Deux pistes pour le watcher : notify ou sondage. Laquelle ?"];
            let has = self.model.iter().any(|s| s.question.is_some());
            for m in &mut self.model {
                m.question = None;
            }
            if !has {
                self.model[s].question = Some(qs[self.rng.i(3) as usize].to_string());
            }
        } else if x < 0.9 && self.model.len() < 3 {
            self.add_session("MyLife");
            let s = self.model.len() - 1;
            self.add_agent(s);
        }
        Some(&self.model)
    }
    pub fn model(&self) -> &Model {
        &self.model
    }
}
