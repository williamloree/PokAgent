//! Personnages 16×24 (dessinés dans un cadre 18×26 pour le contour), construits
//! une fois par agent et par univers : 4 directions × 3 images de marche.

use crate::gfx::{hex, Canvas, C};

#[derive(Clone, Copy, PartialEq)]
pub enum Ear {
    Round,
    Pointy,
    Long,
}

#[derive(Clone)]
pub struct Pal {
    pub cap: C,
    pub shirt: C,
    pub pants: C,
    pub hair: C,
    pub fur: C,
    pub muzzle: Option<C>,
    pub ear: Ear,
    pub coat: bool,
    pub claude: bool,
}

#[derive(Clone, Copy)]
pub enum Hw {
    Cap,
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
        fur: hex("#e08850"),
        muzzle: Some(hex("#fff3e6")),
        ear: Ear::Pointy,
        coat: true,
        claude: true,
    }
}

pub fn agent_pal(i: u32) -> Pal {
    let t: [(&str, &str, &str, &str, &str, Option<&str>, Ear); 6] = [
        ("#e24848", "#3a6fd0", "#3a3f5c", "#3a2a20", "#c8a078", None, Ear::Round),
        ("#43a85a", "#f0c43a", "#4a4050", "#6a4020", "#f2f0ec", Some("#ffe8ec"), Ear::Long),
        ("#8a5ad0", "#3ab0a6", "#353a55", "#202028", "#9a9aaa", None, Ear::Pointy),
        ("#f08a2a", "#6e7088", "#2e3248", "#8a5a2a", "#e8c070", None, Ear::Round),
        ("#e86aa8", "#2f4a86", "#3a3040", "#c08840", "#f0c0c8", None, Ear::Long),
        ("#3a98e0", "#d85050", "#2e3a4a", "#4a2e1a", "#6a5a50", None, Ear::Pointy),
    ];
    let p = &t[(i % 6) as usize];
    Pal {
        cap: hex(p.0),
        shirt: hex(p.1),
        pants: hex(p.2),
        hair: hex(p.3),
        fur: hex(p.4),
        muzzle: p.5.map(hex),
        ear: p.6,
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
    let (skin, skin_d, eye, blush, shoe) = (hex("#f6c9a0"), hex("#dca07a"), hex("#2a2a38"), hex("#f0a8a0"), hex("#3a3038"));
    let white = hex("#ffffff");
    let (cap_d, cap_h) = (p.cap.sh(-0.3), p.cap.sh(0.35));
    let (sh_d, sh_h, pa_d, hair_d) = (p.shirt.sh(-0.25), p.shirt.sh(0.25), p.pants.sh(-0.3), p.hair.sh(-0.3));
    let (m, md, mh) = (hex("#b8c0cc"), hex("#7a8290"), hex("#eef2f6"));
    let fi = f as i32;
    if dir == DOWN || dir == UP {
        let lu = if fi == 1 { 1 } else { 0 };
        let ru = if fi == 2 { 1 } else { 0 };
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
        match hw {
            Hw::Cap => {
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
        r!(7, 6, 1, 2, skin_d);
        r!(11, 7, 1, 2, eye);
        r!(13, 8, 1, 1, skin_d);
        r!(11, 9, 1, 1, blush);
        match hw {
            Hw::Cap => {
                r!(4, 0, 8, 1, p.cap);
                r!(3, 1, 10, 4, p.cap);
                r!(5, 1, 3, 1, cap_h);
                r!(3, 4, 4, 1, cap_d);
                r!(10, 4, 5, 1, p.cap);
                r!(9, 5, 6, 1, cap_d);
            }
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
        }
    }
    r!(3, 3, 10, 11, fur);
    r!(2, 4, 12, 9, fur);
    r!(1, 6, 14, 5, fur);
    r!(12, 4, 2, 9, fur_d);
    r!(3, 13, 10, 1, fur_d);
    r!(3, 4, 2, 2, fur_h);
    if dir == DOWN {
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
        r!(10, 6, 2, 2, eye);
        r!(10, 6, 1, 1, white);
        r!(9, 10, 2, 1, blush);
        r!(2, 6, 2, 5, fur_d);
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
