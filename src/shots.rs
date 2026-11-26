//! Captures du README, rendues par le vrai moteur (sans écran ni fenêtre) :
//! cargo test shots -- --ignored   ->  assets/screenshots/<univers>.png

use crate::gfx::Canvas;
use crate::scene::{draw_popup, Scene, NW, PH, PW};
use crate::themes::Theme;
use crate::watcher::{MAgent, MSession, St};

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

/// PNG non compressé (blocs « stored ») : suffisant pour un test.
fn png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::new();
    for y in 0..h as usize {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * w as usize * 4..(y + 1) * w as usize * 4]);
    }
    let mut z = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, ch) in chunks.iter().enumerate() {
        z.push((i == chunks.len() - 1) as u8);
        let l = ch.len() as u16;
        z.extend_from_slice(&l.to_le_bytes());
        z.extend_from_slice(&(!l).to_le_bytes());
        z.extend_from_slice(ch);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut c = kind.to_vec();
        c.extend_from_slice(data);
        out.extend_from_slice(&c);
        out.extend_from_slice(&crc32(&c).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    out
}

/// Logo 16×16 (le même que assets/logo.svg).
fn logo() -> Canvas {
    use crate::gfx::hex;
    let mut c = Canvas::new(16, 16);
    c.blob(0, 0, 16, 16, hex("#2b2633"));
    c.fill(1, 1, 14, 14, hex("#2b2633"));
    for (x, y, w, h, col) in [
        (5, 3, 6, 1, "#d97757"),
        (4, 4, 8, 2, "#d97757"),
        (7, 4, 2, 1, "#ffffff"),
        (4, 6, 8, 1, "#a8553a"),
        (4, 7, 8, 4, "#f6c9a0"),
        (4, 7, 1, 2, "#5a3a28"),
        (11, 7, 1, 2, "#5a3a28"),
        (6, 8, 1, 2, "#2a2a38"),
        (9, 8, 1, 2, "#2a2a38"),
        (5, 11, 6, 3, "#f4f2ec"),
        (7, 11, 2, 3, "#d97757"),
        (12, 1, 3, 6, "#ffffff"),
        (13, 2, 1, 2, "#e83838"),
        (13, 5, 1, 1, "#e83838"),
    ] {
        c.fill(x, y, w, h, hex(col));
    }
    c
}

/// Icône Windows (entrées PNG) : cargo test icon -- --ignored  ->  assets/pokagent.ico
#[test]
#[ignore]
fn icon() {
    let src = logo();
    let sizes = [16u32, 20, 24, 32, 40, 48, 64];
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&s| {
            let n = s as usize;
            let mut px = vec![0u8; n * n * 4];
            for y in 0..n {
                for x in 0..n {
                    let c = src.px[(y * 16 / n) * 16 + x * 16 / n];
                    // BMP : lignes de bas en haut, ordre BGRA
                    px[((n - 1 - y) * n + x) * 4..][..4].copy_from_slice(&[c.2, c.1, c.0, c.3]);
                }
            }
            let mut bmp = Vec::new();
            for v in [40u32, s, s * 2] {
                bmp.extend_from_slice(&v.to_le_bytes());
            }
            bmp.extend_from_slice(&1u16.to_le_bytes());
            bmp.extend_from_slice(&32u16.to_le_bytes());
            bmp.extend_from_slice(&[0u8; 24]);
            bmp.extend_from_slice(&px);
            // masque ET (1 bit par pixel, lignes alignées sur 4 octets) : vide, l'alpha suffit
            let mask_row = n.div_ceil(32) * 4;
            bmp.extend(std::iter::repeat_n(0u8, mask_row * n));
            bmp
        })
        .collect();
    let mut ico = vec![0, 0, 1, 0];
    ico.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (s, img) in sizes.iter().zip(&images) {
        let b = if *s >= 256 { 0 } else { *s as u8 };
        ico.extend_from_slice(&[b, b, 0, 0, 1, 0, 32, 0]);
        ico.extend_from_slice(&(img.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in &images {
        ico.extend_from_slice(img);
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("pokagent.ico");
    std::fs::write(path, ico).unwrap();
}

fn model() -> Vec<MSession> {
    let a = |id: &str, name: &str, st: St| MAgent { id: id.into(), name: name.into(), status: st, started: 0.0 };
    let s = |id: &str, name: &str, q: Option<&str>, agents: Vec<MAgent>| MSession {
        id: id.into(),
        name: name.into(),
        started: 0.0,
        busy: true,
        question: q.map(str::to_string),
        focus: None,
        agents,
    };
    vec![
        s("s1", "ClaudeIsles", None, vec![a("a1", "explore-api", St::Running), a("a2", "refacto-ui", St::Done), a("a3", "tests-e2e", St::Failed)]),
        s("s2", "PokAgent", Some("Claude a besoin de ta permission pour utiliser Bash."), vec![a("a4", "build-rust", St::Running), a("a5", "sprites-16px", St::Running)]),
        s("s3", "MyLife", None, vec![a("a6", "doc-readme", St::Running)]),
    ]
}

#[test]
#[ignore]
fn shots() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    let (s, w, h) = (2, 1000, 768);
    for theme in Theme::ALL {
        let mut scene = Scene::new(theme);
        scene.sync(&model(), -10.0);
        for _ in 0..30 {
            scene.update(0.125);
        }
        let mut cv = Canvas::new(NW, 270);
        scene.draw(&mut cv, 1.0);
        let mut pc = Canvas::new(PW, PH);
        let portrait = scene.portrait();
        draw_popup(&mut pc, theme, portrait.as_ref(), scene.question.as_ref().unwrap(), 0.0, 1.0);
        // fond : un dégradé façon fond d'écran, pour montrer la transparence
        let mut out = vec![0u8; w * h * 4];
        for y in 0..h {
            let t = y as f32 / h as f32;
            for x in 0..w {
                let i = (y * w + x) * 4;
                out[i] = (38.0 + 30.0 * t) as u8;
                out[i + 1] = (32.0 + 18.0 * t) as u8;
                out[i + 2] = (70.0 + 40.0 * t + 20.0 * x as f32 / w as f32) as u8;
                out[i + 3] = 255;
            }
        }
        let mut paste = |src: &Canvas, ox: usize, oy: usize| {
            for y in 0..src.h as usize * s {
                for x in 0..src.w as usize * s {
                    let c = src.px[(y / s) * src.w as usize + x / s];
                    let (dx, dy) = (ox + x, oy + y);
                    if c.3 == 0 || dx >= w || dy >= h {
                        continue;
                    }
                    let i = (dy * w + dx) * 4;
                    let a = c.3 as f32 / 255.0;
                    for (k, v) in [c.0, c.1, c.2].into_iter().enumerate() {
                        out[i + k] = (v as f32 * a + out[i + k] as f32 * (1.0 - a)) as u8;
                    }
                }
            }
        };
        paste(&cv, 20, 20);
        paste(&pc, w - PW as usize * s - 20, h - PH as usize * s - 20);
        std::fs::write(dir.join(format!("{}.png", theme.id())), png(w as u32, h as u32, &out)).unwrap();
    }
}
