//! Les trois univers. Chacun fournit : le décor d'une salle (calculé une fois),
//! l'indicateur de statut (redessiné à chaque image), la porte entre deux salles
//! d'une même session, les personnages et quelques réglages.

use crate::gfx::{hex, rgba, Canvas, Rng, C};
use crate::sprites::{animal, human, Hw, Pal};

pub const RW: i32 = 120;
pub const RH: i32 = 88;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Labo,
    Village,
    Donjon,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Run,
    Idle,
    Done,
    Failed,
}

pub fn status_color(m: Mode) -> C {
    match m {
        Mode::Run => hex("#58a8f8"),
        Mode::Idle => hex("#68c8a0"),
        Mode::Done => hex("#f0c040"),
        Mode::Failed => hex("#e04848"),
    }
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Labo, Theme::Village, Theme::Donjon];

    pub fn index(self) -> usize {
        self as usize
    }
    pub fn id(self) -> &'static str {
        match self {
            Theme::Labo => "labo",
            Theme::Village => "village",
            Theme::Donjon => "donjon",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Theme::Labo => "Labo (dresseurs, style DS)",
            Theme::Village => "Village (île, habitants animaux)",
            Theme::Donjon => "Donjon (aventure, chevaliers)",
        }
    }
    pub fn from_id(s: &str) -> Theme {
        Theme::ALL.into_iter().find(|t| t.id() == s).unwrap_or(Theme::Labo)
    }
    /// Zone où le personnage se promène : x0, x1, y0, y1.
    pub fn walk(self) -> [f32; 4] {
        match self {
            Theme::Labo => [36.0, 78.0, 30.0, 58.0],
            Theme::Village => [10.0, 80.0, 36.0, 58.0],
            Theme::Donjon => [46.0, 80.0, 30.0, 58.0],
        }
    }
    /// Endroit où il va « travailler » (PC, porte de la maison, autel).
    pub fn spot(self) -> [f32; 2] {
        match self {
            Theme::Labo => [14.0, 26.0],
            Theme::Village => [47.0, 14.0],
            Theme::Donjon => [53.0, 22.0],
        }
    }
    pub fn plate(self) -> (C, C) {
        match self {
            Theme::Labo => (rgba(20, 16, 30, 0.78), hex("#f8f0d8")),
            Theme::Village => (rgba(110, 74, 40, 0.9), hex("#fff6dc")),
            Theme::Donjon => (rgba(10, 8, 14, 0.85), hex("#f0c860")),
        }
    }
    pub fn character(self, p: &Pal, dir: usize, f: usize) -> Canvas {
        match self {
            Theme::Labo => human(p, dir, f, Hw::Own),
            Theme::Village => animal(p, dir, f),
            Theme::Donjon => human(p, dir, f, if p.claude { Hw::Hood } else { Hw::Helm }),
        }
    }
    pub fn bg(self, hub: bool, seed: u32, p: &Pal) -> Canvas {
        match self {
            Theme::Labo => labo_bg(hub, seed),
            Theme::Village => village_bg(hub, seed, p),
            Theme::Donjon => donjon_bg(hub, seed, p),
        }
    }
    pub fn status(self, c: &mut Canvas, m: Mode, x: i32, y: i32, t: f64, hub: bool, seed: u32) {
        match self {
            Theme::Labo => labo_status(c, m, x, y, t),
            Theme::Village => village_status(c, m, x, y, t, hub),
            Theme::Donjon => donjon_status(c, m, x, y, t, hub, seed),
        }
    }
    /// Porte entre deux salles voisines ; x est la frontière.
    pub fn door(self, c: &mut Canvas, x: i32, y: i32) {
        match self {
            Theme::Labo => {
                // paillasson : les deux salles n'ont pas forcément le même parquet
                c.fill(x - 3, y + 50, 6, 16, hex("#7a5a3e"));
                for yy in [52, 55, 58, 61, 64] {
                    c.fill(x - 3, y + yy, 6, 1, hex("#5e4430"));
                }
                c.fill(x - 3, y + 46, 6, 4, hex("#2a2433"));
                c.fill(x - 3, y + 49, 6, 1, rgba(0, 0, 0, 0.35));
            }
            Theme::Village => {}
            Theme::Donjon => {
                c.fill(x - 4, y + 48, 8, 18, hex("#3a3848"));
                c.fill(x - 4, y + 48, 8, 1, hex("#1e1c26"));
                c.fill(x - 4, y + 46, 8, 2, hex("#1e1c26"));
            }
        }
    }
}

// ---------------------------------------------------------------- Labo
const WALLS: [&str; 9] = ["#e9e2cf", "#dde6dc", "#ebdccb", "#dce2ee", "#f0d8d0", "#e4dcf0", "#d8e8e8", "#f0e6c4", "#e0d0c0"];
const FLOORS: [&str; 5] = ["#c9935c", "#b07a4a", "#d8b07a", "#9a8a7a", "#a8684a"];
// tapis : bordure sombre, fond, liseré, cœur, motif, médaillon
const RUGS: [[&str; 6]; 6] = [
    ["#6e2632", "#a8414a", "#e0b56a", "#b84a52", "#e8c47a", "#3a6a8a"],
    ["#22345e", "#3a5a9a", "#e0c890", "#4a6ab0", "#f0e0b0", "#c84848"],
    ["#2a4a2e", "#3e7a48", "#e8d8a0", "#4e8a56", "#f0e8c0", "#c87830"],
    ["#3e2a5a", "#6a4a90", "#e8c8e0", "#7a5aa0", "#f0d8f0", "#e0b040"],
    ["#1e4a4a", "#2e7a78", "#f0d8a8", "#3a8a88", "#f8e8c8", "#d06048"],
    ["#5a4a30", "#8a7450", "#f0e0c0", "#9a8460", "#fff0d0", "#4a6a8a"],
];

fn labo_bg(hub: bool, seed: u32) -> Canvas {
    let mut c = Canvas::new(RW, RH);
    let mut rnd = Rng::new(seed.wrapping_mul(7919).wrapping_add(13));
    let mut pick = Rng::new(seed.wrapping_mul(2246822519).wrapping_add(3));
    pick.f();
    let wall = hex(WALLS[pick.i(WALLS.len() as i32) as usize]);
    let floor = hex(FLOORS[pick.i(FLOORS.len() as i32) as usize]);
    let rug = RUGS[pick.i(RUGS.len() as i32) as usize].map(hex);
    let decor = pick.i(4);
    let cactus = pick.i(3) == 0;
    c.fill(0, 0, RW, RH, hex("#1c1822"));
    // papier peint
    c.fill(3, 4, RW - 6, 22, wall);
    let mut x = 3;
    while x < RW - 3 {
        c.fill(x, 4, 8, 22, wall.sh(-0.035));
        x += 16;
    }
    let (mut x, mut k) = (3, 0);
    while x < RW - 3 {
        let y = if k % 2 == 1 { 9 } else { 18 };
        let m = wall.sh(-0.12);
        c.fill(x + 3, y, 2, 1, m);
        c.fill(x + 2, y + 1, 4, 1, m);
        c.fill(x + 3, y + 2, 2, 1, m);
        x += 8;
        k += 1;
    }
    c.grad_v(3, 4, RW - 6, 22, 4.0, 26.0, rgba(40, 20, 30, 0.22), rgba(40, 20, 30, 0.0));
    // lambris + plinthe
    c.fill(3, 26, RW - 6, 7, hex("#b5865a"));
    c.fill(3, 26, RW - 6, 1, hex("#d8aa78"));
    c.fill(3, 27, RW - 6, 1, hex("#8f6640"));
    let mut x = 3;
    while x < RW - 3 {
        c.fill(x, 28, 1, 5, hex("#8f6640"));
        c.fill(x + 1, 28, 1, 5, hex("#c89868"));
        x += 12;
    }
    c.fill(3, 33, RW - 6, 2, hex("#5e4030"));
    // parquet en lames (dessiné à part puis recopié dans la zone du sol)
    let mut fl = Canvas::new(RW, RH);
    let gap = floor.sh(-0.35);
    let mut y = 35;
    while y < RH - 3 {
        let mut x = 3 - rnd.i(30);
        while x < RW - 3 {
            let len = 22 + rnd.i(22);
            let col = floor.sh((rnd.f() - 0.5) * 0.14);
            fl.fill(x, y, len, 5, col);
            fl.fill(x, y, len, 1, col.sh(0.14));
            fl.fill(x, y + 5, len, 1, gap);
            fl.fill(x + len - 1, y, 1, 5, gap);
            for _ in 0..2 {
                let gx = x + 3 + rnd.i(len - 6);
                let gy = y + 2 + rnd.i(2);
                fl.fill(gx, gy, 3, 1, col.sh(-0.08));
            }
            x += len;
        }
        y += 6;
    }
    for yy in 35..RH - 3 {
        for xx in 3..RW - 3 {
            let p = fl.px[(yy * RW + xx) as usize];
            c.put(xx, yy, p);
        }
    }
    c.grad_r(3, 35, RW - 6, RH - 38, RW as f32 / 2.0, 58.0, 8.0, 76.0, rgba(255, 240, 200, 0.10), rgba(30, 10, 20, 0.18));
    c.grad_v(3, 35, RW - 6, 6, 35.0, 41.0, rgba(30, 15, 10, 0.30), rgba(30, 15, 10, 0.0));
    // murs latéraux
    let (wd, wl) = (hex("#2a2433"), hex("#4a4058"));
    c.fill(0, 0, RW, 4, wd);
    c.fill(0, 0, 3, RH, wd);
    c.fill(RW - 3, 0, 3, RH, wd);
    c.fill(0, RH - 3, RW, 3, wd);
    c.fill(2, 4, 1, RH - 7, wl);
    c.fill(RW - 3, 4, 1, RH - 7, wl);
    c.fill(3, RH - 3, RW - 6, 1, wl);

    let window_at = |c: &mut Canvas, wx: i32| {
        let wxf = wx as f32;
        c.poly(
            &[(wxf + 2.0, 35.0), (wxf + 24.0, 35.0), (wxf + 40.0, (RH - 6) as f32), (wxf + 14.0, (RH - 6) as f32)],
            rgba(255, 248, 215, 0.13),
        );
        let frame = hex("#f2ece2");
        c.fill(wx, 6, 26, 19, frame);
        c.fill(wx + 24, 6, 2, 19, hex("#c4bcae"));
        c.fill(wx, 23, 26, 2, hex("#c4bcae"));
        c.grad_v(wx + 2, 8, 22, 14, 8.0, 22.0, hex("#6fb8ec"), hex("#d4f0ff"));
        c.fill(wx + 4, 10, 5, 1, hex("#ffffff"));
        c.fill(wx + 3, 11, 8, 1, hex("#f4fbff"));
        c.fill(wx + 15, 15, 6, 1, hex("#ffffff"));
        c.fill(wx + 12, 8, 2, 14, frame);
        c.fill(wx + 2, 14, 22, 1, frame);
        c.fill(wx - 2, 25, 30, 2, hex("#e0d6c6"));
        c.fill(wx - 2, 27, 30, 1, rgba(0, 0, 0, 0.18));
        c.fill(wx - 5, 4, 36, 1, hex("#7a5232"));
        for cx in [wx - 4, wx + 25] {
            c.fill(cx, 5, 5, 22, hex("#c95a4e"));
            c.fill(cx + 1, 5, 1, 22, hex("#e07a6c"));
            c.fill(cx + 3, 5, 1, 22, hex("#a8443c"));
            c.fill(cx, 26, 5, 1, hex("#8e3630"));
        }
    };
    // bureau + PC
    c.fill(7, 46, 30, 3, rgba(20, 10, 10, 0.25));
    c.fill(6, 35, 30, 11, hex("#9a6a40"));
    c.fill(6, 35, 3, 11, hex("#86592f"));
    c.fill(6, 45, 30, 1, hex("#6e4a2c"));
    c.fill(20, 37, 14, 7, hex("#a87648"));
    c.fill(20, 37, 14, 1, hex("#c08a58"));
    c.fill(20, 43, 14, 1, hex("#7a5232"));
    c.fill(26, 40, 2, 1, hex("#e8c070"));
    c.fill(6, 30, 30, 5, hex("#c8905a"));
    c.fill(6, 30, 30, 1, hex("#e0aa70"));
    c.fill(6, 34, 30, 1, hex("#a87548"));
    c.fill(12, 31, 18, 1, rgba(0, 0, 0, 0.2));
    c.fill(19, 29, 4, 2, hex("#9898a6"));
    c.fill(11, 16, 20, 14, hex("#e2e2ea"));
    c.fill(29, 16, 2, 14, hex("#b0b0bc"));
    c.fill(11, 28, 20, 2, hex("#b0b0bc"));
    c.fill(11, 16, 20, 1, hex("#f8f8fc"));
    c.fill(13, 18, 15, 10, hex("#1a2030"));
    c.fill(14, 32, 12, 2, hex("#f0f0f4"));
    c.fill(14, 33, 12, 1, hex("#c8c8d0"));
    c.fill(28, 32, 2, 2, hex("#f0f0f4"));
    // plante (ou cactus)
    let px = RW - 22;
    let pot = hex(["#b8643a", "#4a7ab8", "#e8e0d0", "#6a8a4a"][(seed % 4) as usize]);
    c.fill(px + 1, 50, 16, 3, rgba(20, 10, 10, 0.22));
    c.fill(px + 3, 40, 12, 11, pot);
    c.fill(px + 12, 41, 3, 10, pot.sh(-0.25));
    c.fill(px + 5, 42, 1, 7, pot.sh(0.2));
    c.fill(px + 2, 38, 14, 3, pot.sh(0.1));
    c.fill(px + 2, 38, 14, 1, pot.sh(0.3));
    c.fill(px + 4, 39, 10, 1, hex("#4a3020"));
    if cactus {
        let (g, gd, gl) = (hex("#5a9a50"), hex("#3a6a38"), hex("#8ac878"));
        c.blob(px + 6, 16, 6, 23, gd);
        c.blob(px + 6, 15, 6, 23, g);
        c.fill(px + 7, 17, 1, 20, gl);
        c.blob(px + 1, 24, 4, 8, g);
        c.fill(px + 3, 30, 4, 2, g);
        c.blob(px + 13, 21, 4, 8, g);
        c.fill(px + 11, 26, 3, 2, g);
        c.fill(px + 8, 13, 2, 2, hex("#f070a0"));
    } else {
        for (dx, y, w, h) in [(0, 30, 8, 9), (9, 28, 9, 9), (4, 22, 9, 9), (1, 18, 7, 8), (10, 19, 7, 8), (6, 14, 6, 7)] {
            c.blob(px + dx, y + 1, w, h, hex("#2f6b34"));
            c.blob(px + dx, y, w, h - 1, hex("#4d9a45"));
            c.blob(px + dx + 1, y, w - 3, h - 4, hex("#7cc35e"));
            c.fill(px + dx + 2, y + 1, 1, 1, hex("#b8e890"));
        }
    }
    // tapis
    let (rx, ry, rw, rh) = (42, 52, 46, 22);
    let mut y = ry + 1;
    while y < ry + rh {
        c.fill(rx - 2, y, 2, 1, hex("#efe6d0"));
        c.fill(rx + rw, y, 2, 1, hex("#efe6d0"));
        y += 2;
    }
    c.fill(rx, ry, rw, rh, rug[0]);
    c.fill(rx + 1, ry + 1, rw - 2, rh - 2, rug[1]);
    c.fill(rx + 3, ry + 3, rw - 6, rh - 6, rug[2]);
    c.fill(rx + 4, ry + 4, rw - 8, rh - 8, rug[3]);
    let gold = rug[4];
    let mut x = rx + 7;
    while x < rx + rw - 6 {
        for y in [ry + 6, ry + rh - 9] {
            c.fill(x + 1, y, 1, 3, gold);
            c.fill(x, y + 1, 3, 1, gold);
        }
        x += 8;
    }
    let (mx, my) = (rx + rw / 2 - 5, ry + rh / 2 - 4);
    c.fill(mx, my, 10, 8, gold);
    c.fill(mx + 1, my + 1, 8, 6, rug[5]);
    c.fill(mx + 3, my + 2, 4, 4, gold);
    c.fill(rx, ry + rh, rw, 1, rgba(0, 0, 0, 0.25));

    let mut shelf = |c: &mut Canvas, bx: i32, top: i32, w: i32| {
        let h = 42 - top;
        c.fill(bx + 1, 42, w, 3, rgba(20, 10, 10, 0.25));
        c.fill(bx, top, w, h, hex("#7e5130"));
        c.fill(bx, top, w, 2, hex("#b07a48"));
        c.fill(bx + w - 2, top, 2, h, hex("#5a3820"));
        c.fill(bx + 2, top + 3, w - 5, h - 5, hex("#4e3020"));
        let bk = ["#d84848", "#4878d8", "#e8c040", "#48a858", "#8858c8", "#e08838", "#f0ece0"];
        let mut sy = top + 3;
        while sy + 9 <= top + h - 2 {
            let mut x = bx + 2;
            while x < bx + w - 4 {
                let bw = 2 + rnd.i(2);
                let bh = 6 + rnd.i(3);
                let col = hex(bk[rnd.i(bk.len() as i32) as usize]);
                c.fill(x, sy + 9 - bh, bw, bh, col);
                c.fill(x, sy + 9 - bh, bw, 1, col.sh(0.3));
                c.fill(x + bw - 1, sy + 9 - bh, 1, bh, col.sh(-0.25));
                x += bw;
            }
            c.fill(bx + 2, sy + 9, w - 4, 1, hex("#a06c40"));
            sy += 10;
        }
    };
    if hub {
        window_at(&mut c, 42);
        shelf(&mut c, 74, 6, 22);
    } else if decor == 3 {
        // télévision sur un meuble bas
        c.fill(47, 43, 26, 3, rgba(20, 10, 10, 0.25));
        c.fill(46, 30, 28, 13, hex("#7e5130"));
        c.fill(46, 30, 28, 2, hex("#b07a48"));
        c.fill(48, 34, 11, 7, hex("#5a3820"));
        c.fill(61, 34, 11, 7, hex("#5a3820"));
        c.fill(49, 12, 22, 17, hex("#2a2a34"));
        c.fill(51, 14, 18, 12, hex("#3a6aa0"));
        c.fill(52, 15, 6, 2, hex("#a8d0f0"));
        c.fill(58, 29, 4, 1, hex("#2a2a34"));
        c.fill(66, 8, 1, 4, hex("#2a2a34"));
        c.fill(68, 6, 1, 6, hex("#2a2a34"));
    } else if decor == 0 {
        c.fill(46, 8, 24, 16, hex("#9a7030"));
        c.fill(47, 9, 22, 14, hex("#d8b060"));
        c.fill(47, 9, 22, 1, hex("#f0d088"));
        c.fill(49, 11, 18, 10, hex("#8cc8e8"));
        c.fill(49, 16, 18, 5, hex("#5aa050"));
        c.fill(52, 14, 6, 3, hex("#6ab060"));
        c.fill(61, 12, 3, 2, hex("#f8f0c0"));
        c.fill(47, 24, 22, 1, rgba(0, 0, 0, 0.18));
    } else if decor == 1 {
        window_at(&mut c, 50);
    } else {
        shelf(&mut c, 56, 14, 20);
        c.fill(84, 8, 10, 10, hex("#5a4030"));
        c.fill(85, 9, 8, 8, hex("#f8f4ea"));
        c.fill(86, 8, 6, 1, hex("#5a4030"));
        c.fill(89, 10, 1, 4, hex("#303040"));
        c.fill(89, 13, 3, 1, hex("#303040"));
    }
    c
}

fn labo_status(c: &mut Canvas, m: Mode, x: i32, y: i32, t: f64) {
    let (bg, fg) = match m {
        Mode::Run => ("#143a6e", "#8fd0ff"),
        Mode::Idle => ("#163a30", "#88e0b0"),
        Mode::Done => ("#3a2e10", "#f8d050"),
        Mode::Failed => ("#4a1010", "#ff7070"),
    };
    let fg = hex(fg);
    c.fill(x + 13, y + 18, 15, 10, hex(bg));
    match m {
        Mode::Run | Mode::Idle => {
            let off = (t * 4.0) as i32;
            for i in 0..4 {
                c.fill(x + 14, y + 19 + i * 2, 3 + ((i + off) * 7) % 10, 1, fg);
            }
        }
        Mode::Done => {
            for (dx, dy) in [(17, 23), (19, 24), (21, 22), (23, 20)] {
                c.fill(x + dx, y + dy, 2, 2, fg);
            }
        }
        Mode::Failed => {
            for i in 0..6 {
                c.fill(x + 17 + i, y + 20 + i, 2, 1, fg);
                c.fill(x + 22 - i, y + 20 + i, 2, 1, fg);
            }
        }
    }
    c.fill(x + 13, y + 18, 15, 1, rgba(255, 255, 255, 0.18));
}

// ---------------------------------------------------------------- Village
const GRASS: &str = "#86c95a";
const PATH: &str = "#e6d29c";

fn village_bg(hub: bool, seed: u32, p: &Pal) -> Canvas {
    let mut c = Canvas::new(RW, RH);
    let mut rnd = Rng::new(seed.wrapping_mul(104729).wrapping_add(7));
    let grass = hex(GRASS);
    let mut cy = 0;
    while cy < RH {
        let mut cx = 0;
        while cx < RW {
            c.fill(cx, cy, 8, 8, grass.sh((rnd.f() - 0.5) * 0.06));
            cx += 8;
        }
        cy += 8;
    }
    let tuft = hex("#5fa844");
    for _ in 0..40 {
        let (x, y) = (rnd.i(RW), rnd.i(RH));
        c.fill(x, y, 1, 2, tuft);
        c.fill(x + 2, y, 1, 2, tuft);
        c.fill(x + 1, y + 1, 1, 2, tuft);
    }
    for _ in 0..30 {
        let (x, y) = (rnd.i(RW), rnd.i(RH));
        c.fill(x, y, 1, 1, hex("#b4e486"));
    }
    // haies
    let (h, hl) = (hex("#3f8a3a"), hex("#5aa848"));
    c.fill(0, 0, RW, 3, h);
    c.fill(0, RH - 3, RW, 3, h);
    c.fill(0, 0, 3, RH, h);
    c.fill(RW - 3, 0, 3, RH, h);
    let mut i = 2;
    while i < RW {
        c.fill(i, 3, 3, 1, h);
        c.fill(i, RH - 4, 3, 1, h);
        c.fill(i + 1, 1, 1, 1, hl);
        i += 6;
    }
    let mut i = 2;
    while i < RH {
        c.fill(3, i, 1, 3, h);
        c.fill(RW - 4, i, 1, 3, h);
        c.fill(1, i + 1, 1, 1, hl);
        i += 6;
    }
    // chemins
    let (path, edge) = (hex(PATH), hex("#c8b078"));
    c.fill(0, 54, RW, 8, path);
    c.fill(0, 53, RW, 1, edge);
    c.fill(0, 62, RW, 1, edge);
    c.fill(47, 36, 8, 18, path);
    c.fill(46, 36, 1, 18, edge);
    c.fill(55, 36, 1, 18, edge);
    for _ in 0..12 {
        let (x, y) = (rnd.i(RW), 55 + rnd.i(6));
        c.fill(x, y, 1, 1, hex("#d0b880"));
    }
    // fleurs
    let fl = ["#f06080", "#f8f0f0", "#f8d040", "#a070e0", "#f09050"];
    for _ in 0..9 {
        let x = 6 + rnd.i(RW - 14);
        let y = if rnd.f() < 0.5 { 40 + rnd.i(10) } else { 66 + rnd.i(14) };
        let col = hex(fl[rnd.i(fl.len() as i32) as usize]);
        if x > 40 && x < 60 && y < 54 {
            continue;
        }
        c.fill(x, y + 1, 3, 1, col);
        c.fill(x + 1, y, 1, 3, col);
        c.fill(x + 1, y + 1, 1, 1, hex("#f8c020"));
    }
    // mare
    let mut pick = Rng::new(seed.wrapping_mul(3266489917).wrapping_add(11));
    pick.f();
    let facade = hex(["#f4ead2", "#dceaf4", "#f6dce0", "#dcf0e0", "#e8e0f4", "#f8e8c8"][pick.i(6) as usize]);
    let tree = pick.i(4);
    if !hub && pick.i(3) == 0 {
        let (px, py) = (78, 66);
        c.blob(px - 1, py - 1, 30, 14, hex("#6a9a50"));
        c.blob(px, py, 28, 12, hex("#4a90c8"));
        c.blob(px + 2, py + 2, 24, 8, hex("#5aa8e0"));
        c.fill(px + 5, py + 3, 6, 1, hex("#bfe4f8"));
        c.fill(px + 16, py + 6, 4, 1, hex("#bfe4f8"));
        c.blob(px + 19, py + 2, 5, 3, hex("#4aa040"));
    }
    // maison
    let (x0, w) = if hub { (24, 56) } else { (30, 42) };
    let roof = if hub { hex("#d97757") } else { p.cap };
    let dx = x0 + w / 2 - 4;
    c.fill(x0 + 2, 36, w - 4, 2, rgba(30, 60, 20, 0.3));
    c.fill(x0 + 2, 20, w - 4, 16, facade);
    c.fill(x0 + w - 6, 20, 4, 16, facade.sh(-0.12));
    c.fill(x0 + 2, 35, w - 4, 1, hex("#b8a888"));
    for i in 0..16 {
        let inset = (8 - i / 2).max(0);
        c.fill(x0 + inset, 4 + i, w - 2 * inset, 1, if i % 4 == 3 { roof.sh(-0.15) } else { roof });
    }
    c.fill(x0 + 8, 4, w - 16, 1, roof.sh(0.3));
    c.fill(x0, 19, w, 2, roof.sh(-0.3));
    if hub {
        let mut i = 0;
        while i < w - 4 {
            c.fill(x0 + 2 + i, 21, 4, 3, hex(if (i / 4) % 2 == 1 { "#f8f4ec" } else { "#e88a60" }));
            i += 4;
        }
        c.fill(x0 + 2, 24, w - 4, 1, rgba(0, 0, 0, 0.15));
        c.fill(dx - 6, 7, 20, 8, hex("#7a5232"));
        c.fill(dx - 5, 8, 18, 6, hex("#f4e4c0"));
        c.fill(dx + 2, 10, 4, 2, hex("#d97757"));
    }
    c.fill(dx, 25, 8, 11, hex("#8a5a3a"));
    c.fill(dx + 1, 26, 6, 10, hex("#a8704a"));
    c.fill(dx + 5, 31, 1, 1, hex("#f0d070"));
    c.fill(dx - 1, 36, 10, 2, hex("#c8b898"));
    c.fill(x0 + 6, 24, 9, 7, hex("#7a5232"));
    c.fill(x0 + 7, 25, 7, 5, hex("#5a7a90"));
    if !hub {
        c.fill(x0 + w - 15, 24, 9, 7, hex("#7a5232"));
        c.fill(x0 + w - 14, 25, 7, 5, hex("#9ad0e8"));
    }
    // boîte aux lettres
    let mx = if hub { 84 } else { 76 };
    c.fill(mx + 3, 28, 2, 9, hex("#7a5232"));
    c.fill(mx, 23, 8, 6, hex("#e8e0d0"));
    c.fill(mx, 28, 8, 1, hex("#b8b0a0"));
    c.fill(mx + 1, 24, 6, 1, hex("#ffffff"));
    c.fill(mx + 1, 37, 6, 1, rgba(30, 60, 20, 0.3));
    if hub {
        c.fill(88, 40, 26, 12, hex("#8a6040"));
        c.fill(89, 41, 24, 10, hex("#6a4a30"));
        for i in 0..6 {
            let col = hex(fl[i as usize % fl.len()]);
            c.fill(91 + i * 4, 43 + (i % 2) * 3, 3, 1, col);
            c.fill(92 + i * 4, 42 + (i % 2) * 3, 1, 3, col);
        }
    } else {
        c.fill(94, 41, 18, 3, rgba(30, 60, 20, 0.3));
        c.fill(100, 30, 5, 11, hex("#8a5a3a"));
        c.fill(103, 30, 2, 11, hex("#6a4228"));
        if tree == 3 {
            // sapin
            for i in 0..26 {
                let half = 2 + i * 9 / 26;
                c.fill(102 - half, 6 + i, half * 2 + 1, 1, if i % 6 == 5 { hex("#1e5a34") } else { hex("#2e7a44") });
                c.fill(102 - half, 6 + i, 2, 1, hex("#4a9a5a"));
            }
        } else {
            // pommier, cerisier en fleurs ou arbre d'automne
            let [d, m, l, hi] = [
                ["#3f8a3a", "#5aa848", "#7cc35e", "#b4e486"],
                ["#c86890", "#e890b0", "#f8b8d0", "#fff0f6"],
                ["#b05a20", "#e08030", "#f0a848", "#f8d080"],
            ][tree as usize]
                .map(hex);
            c.blob(92, 10, 22, 21, d);
            c.blob(93, 9, 20, 19, m);
            c.blob(95, 10, 12, 10, l);
            c.fill(97, 11, 2, 1, hi);
            if tree == 0 {
                let fruit = hex(["#e84040", "#f8a030", "#f8c8c8", "#e8d040"][(seed % 4) as usize]);
                for (fx, fy) in [(96, 18), (104, 14), (108, 22), (99, 25)] {
                    c.fill(fx, fy, 2, 2, fruit);
                    c.fill(fx, fy, 1, 1, hex("#ffffff"));
                }
            } else if tree == 1 {
                for (fx, fy) in [(94, 34), (110, 38), (88, 40)] {
                    c.fill(fx, fy, 2, 1, hex("#f8b8d0"));
                }
            } else {
                for (fx, fy) in [(92, 36), (109, 39), (98, 44)] {
                    c.fill(fx, fy, 2, 1, hex("#e08030"));
                }
            }
        }
    }
    c
}

fn village_status(c: &mut Canvas, m: Mode, x: i32, y: i32, t: f64, hub: bool) {
    let (x0, mx) = if hub { (24, 84) } else { (30, 76) };
    let lit = matches!(m, Mode::Run | Mode::Idle);
    let glass = if !lit {
        hex("#5a7a90")
    } else if (t * 2.0) as i64 % 2 == 1 {
        hex("#ffe08a")
    } else {
        hex("#ffd060")
    };
    c.fill(x + x0 + 7, y + 25, 7, 5, glass);
    if lit {
        c.fill(x + x0 + 7, y + 25, 3, 1, hex("#fff6d0"));
    }
    c.fill(x + mx + 8, y + 19, 1, 9, hex("#555560"));
    c.fill(x + mx + 9, y + 19, 5, 4, status_color(m));
    c.fill(x + mx + 9, y + 22, 5, 1, rgba(0, 0, 0, 0.25));
}

// ---------------------------------------------------------------- Donjon

fn donjon_bg(hub: bool, seed: u32, p: &Pal) -> Canvas {
    let mut c = Canvas::new(RW, RH);
    let mut rnd = Rng::new(seed.wrapping_mul(15485863).wrapping_add(3));
    let mut pick = Rng::new(seed.wrapping_mul(374761393).wrapping_add(5));
    pick.f();
    // pierre bleutée, grès, pierre moussue, pierre violette
    let stone = hex(["#5a5a72", "#7a6a52", "#506a5a", "#5e4a6e"][pick.i(4) as usize]);
    let slab = stone.sh(-0.15);
    let runner = if hub { None } else { [None, Some("#7a2030"), Some("#203a7a"), Some("#2a5a30")][pick.i(4) as usize].map(hex) };
    c.fill(0, 0, RW, RH, hex("#2e2c3a"));
    let (mut row, mut y) = (0, 6);
    while y < 34 {
        let mut x = if row % 2 == 1 { -6 } else { 0 };
        while x < RW {
            let col = stone.sh((rnd.f() - 0.5) * 0.18);
            c.fill(x + 1, y + 1, 11, 5, col);
            c.fill(x + 1, y + 1, 11, 1, col.sh(0.15));
            x += 12;
        }
        row += 1;
        y += 6;
    }
    c.fill(0, 0, RW, 6, hex("#3a3848"));
    c.fill(0, 5, RW, 1, hex("#4e4c60"));
    c.fill(0, 33, RW, 2, hex("#1e1c26"));
    c.fill(0, 35, RW, RH - 35, hex("#2a2834"));
    let (mut row, mut y) = (0, 35);
    while y < RH {
        let mut x = if row % 2 == 1 { -6 } else { 0 };
        while x < RW {
            let col = slab.sh((rnd.f() - 0.5) * 0.16);
            c.fill(x + 1, y + 1, 11, 9, col);
            c.fill(x + 1, y + 1, 11, 1, col.sh(0.14));
            if rnd.f() < 0.18 {
                let (cx, cy) = (x + 3 + rnd.i(6), y + 3);
                let k = hex("#2a2834");
                c.fill(cx, cy, 1, 1, k);
                c.fill(cx + 1, cy + 1, 1, 1, k);
                c.fill(cx + 1, cy + 2, 2, 1, k);
            }
            if rnd.f() < 0.12 {
                let mx = x + 2 + rnd.i(8);
                c.fill(mx, y + 7, 2, 1, hex("#4a6a40"));
            }
            x += 12;
        }
        row += 1;
        y += 10;
    }
    if let Some(rc) = runner {
        let gold = hex("#c8a050");
        c.fill(58, 35, 24, RH - 38, rc.sh(-0.3));
        c.fill(59, 35, 22, RH - 38, rc);
        c.fill(61, 35, 1, RH - 38, gold);
        c.fill(78, 35, 1, RH - 38, gold);
        let mut y = 40;
        while y < RH - 6 {
            c.fill(69, y, 2, 2, gold);
            y += 8;
        }
    }
    c.grad_v(0, 35, RW, 7, 35.0, 42.0, rgba(0, 0, 0, 0.45), rgba(0, 0, 0, 0.0));
    for tx in [14, RW - 22] {
        c.fill(tx, 13, 8, 3, hex("#8a7058"));
        c.fill(tx + 1, 16, 6, 2, hex("#6a5040"));
        c.fill(tx + 3, 18, 2, 5, hex("#4a3828"));
    }
    if hub {
        let (o, od) = (hex("#d97757"), hex("#a8553a"));
        for bx in [34, 78] {
            c.fill(bx - 2, 7, 12, 1, hex("#8a7058"));
            c.fill(bx, 8, 8, 20, o);
            c.fill(bx + 6, 8, 2, 20, od);
            c.fill(bx, 28, 3, 2, o);
            c.fill(bx + 5, 28, 3, 2, od);
            c.fill(bx + 3, 13, 2, 4, hex("#f8e8c8"));
        }
        c.fill(46, 44, 28, 3, rgba(0, 0, 0, 0.45));
        c.fill(48, 32, 24, 12, hex("#6a6a80"));
        c.fill(48, 30, 24, 3, hex("#8a8aa0"));
        c.fill(68, 32, 4, 12, hex("#4a4a5c"));
        c.fill(50, 36, 16, 1, hex("#5a5a70"));
    } else {
        let d = p.cap.sh(-0.3);
        c.fill(52, 7, 16, 1, hex("#8a7058"));
        c.fill(54, 8, 12, 22, p.cap);
        c.fill(63, 8, 3, 22, d);
        c.fill(54, 30, 4, 2, p.cap);
        c.fill(62, 30, 4, 2, d);
        c.fill(58, 14, 4, 4, hex("#f0d070"));
        c.fill(59, 15, 2, 2, p.cap.sh(-0.2));
        c.fill(30, 44, 15, 2, rgba(0, 0, 0, 0.45));
    }
    for (px, py) in [(RW - 18, 38), (RW - 12, 44)] {
        c.fill(px, py + 7, 9, 2, rgba(0, 0, 0, 0.4));
        c.fill(px, py, 8, 8, hex("#8a5a40"));
        c.fill(px + 1, py - 1, 6, 1, hex("#a87050"));
        c.fill(px + 6, py, 2, 8, hex("#6a4030"));
        c.fill(px + 2, py + 2, 1, 3, hex("#b88060"));
    }
    let (wd, we) = (hex("#3a3848"), hex("#1e1c26"));
    c.fill(0, 0, 4, RH, wd);
    c.fill(RW - 4, 0, 4, RH, wd);
    c.fill(0, RH - 3, RW, 3, wd);
    c.fill(4, 6, 1, RH - 9, we);
    c.fill(RW - 5, 6, 1, RH - 9, we);
    c.fill(4, RH - 4, RW - 8, 1, we);
    c.grad_r(0, 0, RW, RH, RW as f32 / 2.0, 52.0, 14.0, 82.0, rgba(0, 0, 0, 0.0), rgba(0, 0, 0, 0.55));
    c
}

fn donjon_status(c: &mut Canvas, m: Mode, x: i32, y: i32, t: f64, hub: bool, seed: u32) {
    let f = match m {
        Mode::Run => ["#2a6cf0", "#7ab8ff", "#e0f4ff"],
        Mode::Idle => ["#2a9a70", "#78e0b0", "#e0fff0"],
        Mode::Done => ["#e08a20", "#f8c840", "#fff4c0"],
        Mode::Failed => ["#a01818", "#f04040", "#ffc0a0"],
    }
    .map(hex);
    for (i, tx) in [14, RW - 22].into_iter().enumerate() {
        let (fx, fy) = (x + tx + 1, y + 13);
        let k = if (t * 9.0 + i as f64 * 2.0 + seed as f64).sin() > 0.0 { 1 } else { 0 };
        c.grad_r(fx - 20, fy - 20, 46, 46, fx as f32 + 3.0, fy as f32, 1.0, 22.0, f[1].a(0x55), f[1].a(0));
        c.fill(fx, fy - 4, 6, 4, f[0]);
        c.fill(fx + 1, fy - 7 - k, 4, 4, f[1]);
        c.fill(fx + 2, fy - 9 - k, 2, 3, f[1]);
        c.fill(fx + 2, fy - 4, 2, 3, f[2]);
    }
    if hub {
        let b = ((t * 3.0).sin() * 1.5).round() as i32;
        let (cx, cy) = (x + 57, y + 18 + b);
        c.grad_r(cx - 14, cy - 12, 34, 34, cx as f32 + 3.0, cy as f32 + 5.0, 1.0, 16.0, hex("#ff9a6a").a(0x66), hex("#ff9a6a").a(0));
        c.fill(cx + 2, cy, 2, 1, hex("#ffd0b0"));
        c.fill(cx + 1, cy + 1, 4, 2, hex("#f0a070"));
        c.fill(cx, cy + 3, 6, 4, hex("#d97757"));
        c.fill(cx + 1, cy + 7, 4, 2, hex("#a8553a"));
        c.fill(cx + 2, cy + 9, 2, 1, hex("#a8553a"));
        c.fill(cx + 1, cy + 3, 1, 3, hex("#ffd0b0"));
    } else {
        let (cx, cy) = (x + 30, y + 34);
        if m == Mode::Done {
            c.fill(cx, cy - 5, 14, 5, hex("#6a4020"));
            c.fill(cx, cy - 5, 14, 1, hex("#8a5a30"));
            c.fill(cx, cy, 14, 10, hex("#8a5a30"));
            c.fill(cx + 1, cy, 12, 3, hex("#3a2010"));
            c.fill(cx + 2, cy, 10, 2, hex("#f8d040"));
            c.fill(cx + 3, cy, 2, 1, hex("#fff4c0"));
            if (t * 3.0) as i64 % 2 == 1 {
                c.fill(cx + 9, cy - 3, 1, 1, hex("#ffffff"));
            }
        } else {
            c.fill(cx, cy, 14, 10, hex("#8a5a30"));
            c.fill(cx, cy - 2, 14, 4, hex("#a87040"));
            c.fill(cx, cy - 2, 14, 1, hex("#c8905a"));
            c.fill(cx + 3, cy - 2, 2, 12, hex("#b8b8c8"));
            c.fill(cx + 9, cy - 2, 2, 12, hex("#b8b8c8"));
            c.fill(cx + 6, cy + 2, 2, 2, hex(if m == Mode::Failed { "#e04848" } else { "#f0d070" }));
        }
    }
}
