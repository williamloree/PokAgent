//! Personnages 16×24 (dessinés dans un cadre 18×26 pour le contour), construits
//! une fois par agent et par univers : 4 directions × 3 images de marche.
//! L'apparence de chaque agent est tirée de son identifiant : peau, coiffure,
//! couvre-chef, lunettes, écharpe, couleurs, espèce…

use crate::gfx::{hex, Canvas, Rng, C};

#[derive(Clone, Copy, PartialEq)]
pub enum Ear {
    Round,
    Pointy,
    Long,
    Floppy,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Hat {
    Cap,
    Beanie,
    Bandana,
    Bare,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Hair {
    Short,
    Long,
    Spiky,
}

#[derive(Clone)]
pub struct Pal {
    pub cap: C,
    pub shirt: C,
    pub pants: C,
    pub hair: C,
    pub skin: C,
    pub skin_d: C,
    pub fur: C,
    pub muzzle: Option<C>,
    pub ear: Ear,
    pub hat: Hat,
    pub style: Hair,
    pub glasses: bool,
    pub scarf: Option<C>,
    pub metal: C,
    pub coat: bool,
    pub claude: bool,
}

/// Couvre-chef imposé par l'univers.
#[derive(Clone, Copy)]
pub enum Hw {
    Own,
    Helm,
    Hood,
}

pub const DOWN: usize = 0;
pub const UP: usize = 1;
pub const RIGHT: usize = 2;
pub const LEFT: usize = 3;

pub type Atlas = Vec<Vec<Canvas>>; // [direction][image]

pub fn claude_pal() -> Pal {
    Pal {
        cap: hex("#d97757"),
        shirt: hex("#f4f2ec"),
        pants: hex("#4a4458"),
        hair: hex("#5a3a28"),
        skin: hex("#f6c9a0"),
        skin_d: hex("#dca07a"),
        fur: hex("#e08850"),
        muzzle: Some(hex("#fff3e6")),
        ear: Ear::Pointy,
        hat: Hat::Cap,
        style: Hair::Short,
        glasses: false,
        scarf: None,
        metal: hex("#b8c0cc"),
        coat: true,
        claude: true,
    }
}

const CAPS: [&str; 14] = [
    "#e24848", "#43a85a", "#8a5ad0", "#f08a2a", "#e86aa8", "#3a98e0", "#2fb3a0", "#f0c030", "#5a6ad8", "#c04878", "#78b830", "#e0603a", "#30a8c8", "#a87038",
];
const SHIRTS: [&str; 14] = [
    "#3a6fd0", "#f0c43a", "#3ab0a6", "#6e7088", "#2f4a86", "#d85050", "#58a848", "#e8e4d8", "#9a4ac0", "#f07848", "#2c2c3a", "#c8a070", "#48c0e0", "#a83850",
];
const PANTS: [&str; 7] = ["#3a3f5c", "#4a4050", "#353a55", "#2e3248", "#5a4a38", "#3a5a48", "#6a6a78"];
const HAIRS: [&str; 9] = ["#3a2a20", "#6a4020", "#202028", "#c08840", "#8a5a2a", "#d8d0c0", "#a83a2a", "#e8c060", "#5a6ab0"];
const SKINS: [(&str, &str); 4] = [("#f6c9a0", "#dca07a"), ("#e8b48a", "#c88e64"), ("#c68a5e", "#a46c44"), ("#8d5a3b", "#6e4429")];
const FURS: [(&str, Option<&str>); 10] = [
    ("#c8a078", None),
    ("#f2f0ec", Some("#ffe8ec")),
    ("#9a9aaa", None),
    ("#e8c070", None),
    ("#f0c0c8", None),
    ("#6a5a50", None),
    ("#e08848", Some("#fff3e6")),
    ("#4a4a58", Some("#d8d8e0")),
    ("#d8b890", None),
    ("#a8c8e8", None),
];
const METALS: [&str; 4] = ["#b8c0cc", "#e0c070", "#8a8f9c", "#c89070"];

/// Apparence d'un agent, tirée de façon stable de sa graine.
pub fn agent_pal(seed: u32) -> Pal {
    let mut r = Rng::new(seed.wrapping_mul(2654435761).wrapping_add(17));
    for _ in 0..3 {
        r.f();
    }
    let mut pick = |n: usize| r.i(n as i32) as usize;
    let cap = hex(CAPS[pick(CAPS.len())]);
    let shirt = hex(SHIRTS[pick(SHIRTS.len())]);
    let pants = hex(PANTS[pick(PANTS.len())]);
    let hair = hex(HAIRS[pick(HAIRS.len())]);
    let (sk, skd) = SKINS[pick(SKINS.len())];
    let (fur, muzzle) = FURS[pick(FURS.len())];
    let ear = [Ear::Round, Ear::Pointy, Ear::Long, Ear::Floppy][pick(4)];
    let hat = [Hat::Cap, Hat::Cap, Hat::Cap, Hat::Beanie, Hat::Beanie, Hat::Bandana, Hat::Bare, Hat::Bare][pick(8)];
    let style = [Hair::Short, Hair::Short, Hair::Long, Hair::Spiky][pick(4)];
    let glasses = pick(5) == 0;
    let scarf = if pick(5) == 0 { Some(hex(CAPS[pick(CAPS.len())])) } else { None };
    let metal = hex(METALS[pick(METALS.len())]);
    Pal {
        cap,
        shirt,
        pants,
        hair,
        skin: hex(sk),
        skin_d: hex(skd),
        fur: hex(fur),
        muzzle: muzzle.map(hex),
        ear,
        hat,
        style,
        glasses,
        scarf,
        metal,
        coat: false,
        claude: false,
    }
}

pub fn human(p: &Pal, dir: usize, f: usize, hw: Hw) -> Canvas {
    let mut cv = Canvas::new(18, 26);
    macro_rules! r {
        ($x:expr, $y:expr, $w:expr, $h:expr, $c:expr) => {
            cv.fill($x + 1, $y + 1, $w, $h, $c)
        };
    }
    let (skin, skin_d) = (p.skin, p.skin_d);
    let (eye, blush, shoe, white) = (hex("#2a2a38"), hex("#f0a8a0"), hex("#3a3038"), hex("#ffffff"));
    let (lens, frame) = (hex("#c8e4f0"), hex("#303848"));
    let (cap_d, cap_h) = (p.cap.sh(-0.3), p.cap.sh(0.35));
    let (sh_d, sh_h, pa_d) = (p.shirt.sh(-0.25), p.shirt.sh(0.25), p.pants.sh(-0.3));
    let (hair_d, hair_h) = (p.hair.sh(-0.3), p.hair.sh(0.25));
    let (m, md, mh) = (p.metal, p.metal.sh(-0.3), p.metal.sh(0.45));
    // couvre-chef effectif : celui de l'univers, sinon celui de l'agent
    let hat = match hw {
        Hw::Own => Some(p.hat),
        _ => None,
    };
    let bare = hat == Some(Hat::Bare) || hat == Some(Hat::Bandana);
    let fi = f as i32;
    if dir == DOWN || dir == UP {
        let lu = if fi == 1 { 1 } else { 0 };
        let ru = if fi == 2 { 1 } else { 0 };
        if dir == UP && p.style == Hair::Long && !matches!(hw, Hw::Helm | Hw::Hood) {
            r!(3, 10, 10, 4, p.hair);
        }
        r!(4, 17, 3, 4 - lu, p.pants);
        r!(9, 17, 3, 4 - ru, p.pants);
        r!(6, 17, 1, 4 - lu, pa_d);
        r!(11, 17, 1, 4 - ru, pa_d);
        r!(4, 21 - lu, 3, 2, shoe);
        r!(9, 21 - ru, 3, 2, shoe);
        r!(3, 11, 10, 7, p.shirt);
        r!(3, 11, 1, 6, sh_h);
        r!(11, 11, 2, 7, sh_d);
        r!(3, 17, 10, 1, sh_d);
        if p.coat {
            r!(3, 17, 10, 2, p.shirt);
            r!(3, 18, 10, 1, sh_d);
            r!(11, 17, 2, 2, sh_d);
        }
        if dir == UP && p.style == Hair::Long && !matches!(hw, Hw::Helm | Hw::Hood) {
            r!(4, 11, 8, 3, p.hair);
            r!(4, 13, 8, 1, hair_d);
        }
        if dir == DOWN {
            r!(6, 11, 4, 1, white);
            if p.coat {
                r!(7, 12, 2, 4, hex("#d97757"));
                r!(8, 12, 1, 4, hex("#b05a40"));
            }
            if let Hw::Helm = hw {
                r!(3, 13, 10, 1, hex("#8a6a40"));
                r!(7, 13, 2, 1, hex("#e8c060"));
            }
        }
        let la = match fi {
            1 => -1,
            2 => 1,
            _ => 0,
        };
        r!(1, 12 + la, 2, 4, p.shirt);
        r!(1, 15 + la, 2, 1, sh_d);
        r!(1, 16 + la, 2, 2, skin);
        r!(13, 12 - la, 2, 4, sh_d);
        r!(13, 16 - la, 2, 2, skin_d);
        r!(3, 2, 10, 9, skin);
        r!(2, 3, 12, 7, skin);
        r!(12, 3, 2, 7, skin_d);
        r!(4, 10, 8, 1, skin_d);
        if dir == DOWN {
            r!(2, 4, 2, 5, p.hair);
            r!(12, 4, 2, 5, hair_d);
            r!(4, 5, 8, 1, hair_d);
            if p.style == Hair::Long && !matches!(hw, Hw::Helm | Hw::Hood) {
                r!(1, 4, 2, 9, p.hair);
                r!(13, 4, 2, 9, hair_d);
            }
            if p.glasses {
                r!(4, 7, 3, 2, lens);
                r!(9, 7, 3, 2, lens);
                r!(7, 7, 2, 1, frame);
            }
            r!(5, 7, 1, 2, eye);
            r!(10, 7, 1, 2, eye);
            r!(4, 9, 1, 1, blush);
            r!(11, 9, 1, 1, blush);
            r!(7, 9, 2, 1, skin_d);
        } else {
            r!(2, 3, 12, 7, p.hair);
            r!(3, 9, 10, 1, hair_d);
            r!(12, 3, 2, 7, hair_d);
            r!(5, 10, 6, 1, skin_d);
        }
        if let Some(s) = p.scarf {
            r!(3, 10, 10, 2, s);
            r!(3, 11, 10, 1, s.sh(-0.2));
            if dir == DOWN {
                r!(9, 12, 2, 3, s);
            }
        }
        match hw {
            Hw::Own => match p.hat {
                Hat::Cap => {
                    r!(3, 0, 10, 1, p.cap);
                    r!(2, 1, 12, 4, p.cap);
                    r!(4, 1, 3, 1, cap_h);
                    r!(3, 2, 1, 1, cap_h);
                    r!(12, 1, 2, 4, cap_d);
                    if dir == DOWN {
                        r!(2, 5, 12, 1, cap_d);
                        r!(6, 2, 4, 2, white);
                        r!(7, 2, 2, 1, p.cap);
                    } else {
                        r!(2, 5, 12, 1, p.cap);
                    }
                }
                Hat::Beanie => {
                    r!(7, 0, 2, 1, cap_h);
                    r!(3, 1, 10, 1, p.cap);
                    r!(2, 2, 12, 3, p.cap);
                    r!(4, 2, 2, 1, cap_h);
                    r!(12, 2, 2, 3, cap_d);
                    r!(2, 5, 12, 1, cap_d);
                    for k in 0..6 {
                        r!(2 + k * 2, 5, 1, 1, p.cap);
                    }
                }
                Hat::Bandana | Hat::Bare => {}
            },
            Hw::Helm => {
                if dir == DOWN {
                    r!(3, 1, 10, 1, m);
                    r!(2, 2, 12, 4, m);
                    r!(4, 2, 3, 1, mh);
                    r!(12, 2, 2, 4, md);
                    r!(2, 6, 12, 1, md);
                    r!(2, 6, 2, 4, m);
                    r!(12, 6, 2, 4, md);
                } else {
                    r!(3, 1, 10, 1, m);
                    r!(2, 2, 12, 8, m);
                    r!(4, 2, 3, 1, mh);
                    r!(12, 2, 2, 8, md);
                    r!(3, 9, 10, 1, md);
                }
                r!(6, 0, 4, 2, p.cap);
            }
            Hw::Hood => {
                if dir == DOWN {
                    r!(3, 0, 10, 1, p.cap);
                    r!(2, 1, 12, 4, p.cap);
                    r!(4, 1, 3, 1, cap_h);
                    r!(1, 4, 2, 8, p.cap);
                    r!(13, 4, 2, 8, cap_d);
                } else {
                    r!(3, 0, 10, 1, p.cap);
                    r!(1, 1, 14, 11, p.cap);
                    r!(13, 1, 2, 11, cap_d);
                    r!(2, 11, 12, 1, cap_d);
                }
            }
        }
        if bare {
            // cheveux sur le dessus de la tête
            r!(3, 1, 10, 1, p.hair);
            r!(2, 2, 12, 3, p.hair);
            r!(4, 2, 3, 1, hair_h);
            r!(12, 2, 2, 3, hair_d);
            if p.style == Hair::Spiky {
                for k in 0..4 {
                    r!(3 + k * 3, 0, 2, 1, p.hair);
                }
            }
            if hat == Some(Hat::Bandana) {
                r!(2, 3, 12, 2, p.cap);
                r!(3, 3, 3, 1, cap_h);
                if dir == UP {
                    r!(6, 5, 4, 2, p.cap);
                }
            }
        }
    } else {
        if fi == 0 {
            r!(6, 17, 4, 4, p.pants);
            r!(9, 17, 1, 4, pa_d);
            r!(6, 21, 5, 2, shoe);
        } else {
            let a = fi == 1;
            r!(4, 17, 3, 4, if a { p.pants } else { pa_d });
            r!(9, 17, 3, 4, if a { pa_d } else { p.pants });
            r!(3, 21, 4, 2, shoe);
            r!(9, 21, 4, 2, shoe);
        }
        r!(4, 11, 8, 7, p.shirt);
        r!(4, 11, 1, 6, sh_h);
        r!(10, 11, 2, 7, sh_d);
        r!(4, 17, 8, 1, sh_d);
        if p.coat {
            r!(4, 17, 8, 2, p.shirt);
            r!(4, 18, 8, 1, sh_d);
        }
        let ax = match fi {
            1 => 1,
            2 => -1,
            _ => 0,
        };
        r!(7 + ax, 12, 3, 4, sh_d);
        r!(7 + ax, 16, 3, 2, skin);
        r!(4, 2, 9, 9, skin);
        r!(3, 3, 11, 7, skin);
        r!(4, 10, 7, 1, skin_d);
        r!(3, 3, 5, 7, p.hair);
        r!(3, 9, 4, 1, hair_d);
        if p.style == Hair::Long && !matches!(hw, Hw::Helm | Hw::Hood) {
            r!(2, 4, 4, 9, p.hair);
            r!(2, 12, 4, 1, hair_d);
        }
        r!(7, 6, 1, 2, skin_d);
        if p.glasses {
            r!(10, 7, 3, 2, lens);
            r!(8, 7, 2, 1, frame);
        }
        r!(11, 7, 1, 2, eye);
        r!(13, 8, 1, 1, skin_d);
        r!(11, 9, 1, 1, blush);
        if let Some(s) = p.scarf {
            r!(4, 10, 8, 2, s);
            r!(4, 11, 8, 1, s.sh(-0.2));
            r!(3, 12, 2, 3, s);
        }
        match hw {
            Hw::Own => match p.hat {
                Hat::Cap => {
                    r!(4, 0, 8, 1, p.cap);
                    r!(3, 1, 10, 4, p.cap);
                    r!(5, 1, 3, 1, cap_h);
                    r!(3, 4, 4, 1, cap_d);
                    r!(10, 4, 5, 1, p.cap);
                    r!(9, 5, 6, 1, cap_d);
                }
                Hat::Beanie => {
                    r!(7, 0, 2, 1, cap_h);
                    r!(4, 1, 8, 1, p.cap);
                    r!(3, 2, 10, 3, p.cap);
                    r!(5, 2, 2, 1, cap_h);
                    r!(3, 5, 10, 1, cap_d);
                }
                Hat::Bandana | Hat::Bare => {
                    r!(4, 1, 8, 1, p.hair);
                    r!(3, 2, 10, 3, p.hair);
                    r!(5, 2, 3, 1, hair_h);
                    if p.style == Hair::Spiky {
                        for k in 0..3 {
                            r!(4 + k * 3, 0, 2, 1, p.hair);
                        }
                    }
                    if p.hat == Hat::Bandana {
                        r!(3, 3, 10, 2, p.cap);
                        r!(1, 4, 2, 2, p.cap);
                    }
                }
            },
            Hw::Helm => {
                r!(4, 1, 8, 1, m);
                r!(3, 2, 10, 4, m);
                r!(5, 2, 3, 1, mh);
                r!(3, 6, 4, 4, m);
                r!(3, 9, 4, 1, md);
                r!(5, 0, 4, 2, p.cap);
                r!(2, 1, 2, 2, p.cap);
            }
            Hw::Hood => {
                r!(4, 0, 8, 1, p.cap);
                r!(3, 1, 10, 4, p.cap);
                r!(5, 1, 3, 1, cap_h);
                r!(2, 4, 5, 8, p.cap);
                r!(2, 11, 5, 1, cap_d);
            }
        }
    }
    cv.outline(hex("#2b2633"));
    cv
}

pub fn animal(p: &Pal, dir: usize, f: usize) -> Canvas {
    let mut cv = Canvas::new(18, 26);
    macro_rules! r {
        ($x:expr, $y:expr, $w:expr, $h:expr, $c:expr) => {
            cv.fill($x + 1, $y + 1, $w, $h, $c)
        };
    }
    let (eye, blush, nose, white) = (hex("#2a2a38"), hex("#f0a8a0"), hex("#3a2a2a"), hex("#ffffff"));
    let (lens, frame) = (hex("#c8e4f0"), hex("#303848"));
    let fur = p.fur;
    let (fur_d, fur_h) = (fur.sh(-0.22), fur.sh(0.3));
    let mz = p.muzzle.unwrap_or(fur.sh(0.55));
    let (sh_d, sh_h, pink) = (p.shirt.sh(-0.25), p.shirt.sh(0.25), hex("#f0a0a8"));
    let side = dir == RIGHT;
    let fi = f as i32;
    if !side {
        r!(4, 19, 3, 3 - if fi == 1 { 1 } else { 0 }, fur_d);
        r!(9, 19, 3, 3 - if fi == 2 { 1 } else { 0 }, fur_d);
    } else if fi == 0 {
        r!(6, 19, 4, 3, fur_d);
    } else {
        r!(4, 19, 3, 3, fur_d);
        r!(9, 19, 3, 3, fur_d);
    }
    r!(4, 14, 8, 6, p.shirt);
    r!(4, 14, 1, 5, sh_h);
    r!(10, 14, 2, 6, sh_d);
    r!(4, 19, 8, 1, sh_d);
    if p.coat && dir == DOWN {
        r!(7, 15, 2, 4, hex("#d97757"));
    }
    if !side {
        let la = match fi {
            1 => -1,
            2 => 1,
            _ => 0,
        };
        r!(2, 15 + la, 2, 3, p.shirt);
        r!(2, 18 + la, 2, 1, fur);
        r!(12, 15 - la, 2, 3, sh_d);
        r!(12, 18 - la, 2, 1, fur_d);
    } else {
        let ax = match fi {
            1 => 1,
            2 => -1,
            _ => 0,
        };
        r!(7 + ax, 15, 3, 3, sh_d);
        r!(7 + ax, 18, 3, 1, fur);
    }
    let ears: &[i32] = if side { &[4] } else { &[2, 11] };
    for &ex in ears {
        let inner = if dir == UP { fur_d } else { pink };
        match p.ear {
            Ear::Long => {
                r!(ex, 0, 3, 5, fur);
                r!(ex + 1, 1, 1, 3, inner);
            }
            Ear::Pointy => {
                r!(ex + 1, 0, 1, 1, fur);
                r!(ex, 1, 3, 3, fur);
                r!(ex + 1, 2, 1, 1, inner);
            }
            Ear::Round => {
                r!(ex, 1, 3, 3, fur);
                r!(ex + 1, 2, 1, 1, inner);
            }
            Ear::Floppy => {}
        }
    }
    r!(3, 3, 10, 11, fur);
    r!(2, 4, 12, 9, fur);
    r!(1, 6, 14, 5, fur);
    r!(12, 4, 2, 9, fur_d);
    r!(3, 13, 10, 1, fur_d);
    r!(3, 4, 2, 2, fur_h);
    if p.ear == Ear::Floppy {
        if side {
            r!(3, 4, 3, 7, fur_d);
        } else {
            r!(0, 4, 3, 7, fur_d);
            r!(13, 4, 3, 7, fur_d);
        }
    }
    if dir == DOWN {
        if p.glasses {
            r!(3, 6, 4, 3, lens);
            r!(9, 6, 4, 3, lens);
            r!(7, 7, 2, 1, frame);
        }
        r!(4, 7, 2, 2, eye);
        r!(10, 7, 2, 2, eye);
        r!(4, 7, 1, 1, white);
        r!(10, 7, 1, 1, white);
        r!(6, 9, 4, 3, mz);
        r!(7, 9, 2, 1, nose);
        r!(7, 11, 2, 1, mz.sh(-0.2));
        r!(3, 10, 2, 1, blush);
        r!(11, 10, 2, 1, blush);
    } else if dir == UP {
        r!(3, 11, 10, 2, fur_d);
    } else {
        r!(12, 8, 3, 3, mz);
        r!(14, 8, 1, 1, nose);
        if p.glasses {
            r!(9, 5, 4, 3, lens);
        }
        r!(10, 6, 2, 2, eye);
        r!(10, 6, 1, 1, white);
        r!(9, 10, 2, 1, blush);
        r!(2, 6, 2, 5, fur_d);
    }
    if let Some(s) = p.scarf {
        r!(3, 13, 10, 2, s);
        r!(3, 14, 10, 1, s.sh(-0.2));
        if dir == DOWN {
            r!(9, 15, 2, 3, s);
        }
    }
    cv.outline(hex("#3a2a28"));
    cv
}

pub fn atlas(build: impl Fn(usize, usize) -> Canvas) -> Atlas {
    let mut a: Atlas = [DOWN, UP, RIGHT].iter().map(|&d| (0..3).map(|f| build(d, f)).collect()).collect();
    let left = a[RIGHT].iter().map(|c| c.flipped()).collect();
    a.push(left);
    a
}

/// Bulle « ! » au-dessus d'un personnage qui attend une réponse.
pub fn bubble() -> Canvas {
    let mut c = Canvas::new(13, 15);
    let (w, red, dark) = (hex("#ffffff"), hex("#e83838"), hex("#a82020"));
    c.fill(2, 1, 9, 11, w);
    c.fill(1, 2, 11, 9, w);
    c.fill(5, 12, 3, 1, w);
    c.fill(5, 13, 1, 1, w);
    c.fill(2, 9, 9, 2, hex("#e4e6f0"));
    c.fill(5, 3, 3, 5, red);
    c.fill(7, 3, 1, 5, dark);
    c.fill(5, 9, 3, 2, red);
    c.fill(7, 9, 1, 2, dark);
    c.outline(hex("#2b2633"));
    c
}
