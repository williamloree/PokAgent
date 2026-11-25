//! Tampon de pixels en mémoire et primitives de dessin (rectangles, dégradés,
//! polygones, contours, texte bitmap). Tout est dessiné sur le CPU.

use embedded_graphics::{
    mono_font::{MonoFont, MonoTextStyle},
    pixelcolor::{Rgb888, RgbColor},
    prelude::*,
    text::{Baseline, Text},
};

/// Couleur RGBA, alpha non prémultiplié.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct C(pub u8, pub u8, pub u8, pub u8);

pub const CLEAR: C = C(0, 0, 0, 0);

pub fn hex(s: &str) -> C {
    let s = s.trim_start_matches('#');
    let v = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0);
    C(v(0), v(2), v(4), if s.len() >= 8 { v(6) } else { 255 })
}

pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> C {
    C(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

impl C {
    /// Éclaircit (a > 0) ou assombrit (a < 0).
    pub fn sh(self, a: f32) -> C {
        let t = if a < 0.0 { 0.0 } else { 255.0 };
        let k = a.abs();
        let f = |v: u8| (v as f32 + (t - v as f32) * k).round() as u8;
        C(f(self.0), f(self.1), f(self.2), self.3)
    }
    pub fn a(self, a: u8) -> C {
        C(self.0, self.1, self.2, a)
    }
}

fn lerp(a: C, b: C, t: f32) -> C {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    C(f(a.0, b.0), f(a.1, b.1), f(a.2, b.2), f(a.3, b.3))
}

#[inline]
fn blend(d: C, s: C) -> C {
    if s.3 == 255 || d.3 == 0 {
        return if s.3 == 0 { d } else { s };
    }
    if s.3 == 0 {
        return d;
    }
    let sa = s.3 as f32 / 255.0;
    let da = d.3 as f32 / 255.0;
    let oa = sa + da * (1.0 - sa);
    let ch = |sc: u8, dc: u8| ((sc as f32 * sa + dc as f32 * da * (1.0 - sa)) / oa).round() as u8;
    C(ch(s.0, d.0), ch(s.1, d.1), ch(s.2, d.2), (oa * 255.0).round() as u8)
}

#[derive(Clone, PartialEq)]
pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub px: Vec<C>,
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        Canvas { w, h, px: vec![CLEAR; (w * h) as usize] }
    }

    pub fn clear(&mut self) {
        self.px.fill(CLEAR);
    }

    /// Valeur qu'aucun dessin ne produit : tout pixel comparé paraîtra changé.
    pub fn invalidate(&mut self) {
        self.px.fill(C(255, 0, 255, 1));
    }

    #[inline]
    pub fn put(&mut self, x: i32, y: i32, c: C) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let i = (y * self.w + x) as usize;
        self.px[i] = blend(self.px[i], c);
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: C) {
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = ((x + w).min(self.w), (y + h).min(self.h));
        for yy in y0..y1 {
            let row = (yy * self.w) as usize;
            for xx in x0..x1 {
                let i = row + xx as usize;
                self.px[i] = blend(self.px[i], c);
            }
        }
    }

    /// Rectangle aux coins rognés d'un pixel.
    pub fn blob(&mut self, x: i32, y: i32, w: i32, h: i32, c: C) {
        self.fill(x + 1, y, w - 2, h, c);
        self.fill(x, y + 1, w, h - 2, c);
    }

    pub fn blit(&mut self, src: &Canvas, x: i32, y: i32) {
        for sy in 0..src.h {
            for sx in 0..src.w {
                let c = src.px[(sy * src.w + sx) as usize];
                if c.3 > 0 {
                    self.put(x + sx, y + sy, c);
                }
            }
        }
    }

    /// Dégradé vertical de c0 (en y0) à c1 (en y1), posé sur le rectangle.
    pub fn grad_v(&mut self, x: i32, y: i32, w: i32, h: i32, y0: f32, y1: f32, c0: C, c1: C) {
        for yy in y..y + h {
            let t = ((yy as f32 + 0.5 - y0) / (y1 - y0)).clamp(0.0, 1.0);
            self.fill(x, yy, w, 1, lerp(c0, c1, t));
        }
    }

    /// Dégradé radial centré, de c0 (rayon r0) à c1 (rayon r1).
    #[allow(clippy::too_many_arguments)]
    pub fn grad_r(&mut self, x: i32, y: i32, w: i32, h: i32, cx: f32, cy: f32, r0: f32, r1: f32, c0: C, c1: C) {
        for yy in y..y + h {
            for xx in x..x + w {
                let d = ((xx as f32 + 0.5 - cx).powi(2) + (yy as f32 + 0.5 - cy).powi(2)).sqrt();
                let t = ((d - r0) / (r1 - r0)).clamp(0.0, 1.0);
                self.put(xx, yy, lerp(c0, c1, t));
            }
        }
    }

    /// Polygone plein (règle pair-impair, centres de pixels).
    pub fn poly(&mut self, p: &[(f32, f32)], c: C) {
        let ymin = p.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor() as i32;
        let ymax = p.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil() as i32;
        for yy in ymin..ymax {
            let py = yy as f32 + 0.5;
            let mut xs = Vec::new();
            for i in 0..p.len() {
                let (a, b) = (p[i], p[(i + 1) % p.len()]);
                if (a.1 <= py && b.1 > py) || (b.1 <= py && a.1 > py) {
                    xs.push(a.0 + (py - a.1) / (b.1 - a.1) * (b.0 - a.0));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for pair in xs.chunks(2) {
                if let [x0, x1] = pair {
                    let (a, b) = ((x0 - 0.5).ceil() as i32, (x1 - 0.5).floor() as i32);
                    self.fill(a, yy, b - a + 1, 1, c);
                }
            }
        }
    }

    /// Chaque pixel vide voisin d'un pixel plein prend la couleur du contour.
    pub fn outline(&mut self, col: C) {
        let src: Vec<bool> = self.px.iter().map(|c| c.3 > 0).collect();
        let (w, h) = (self.w, self.h);
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if src[i] {
                    continue;
                }
                let n = (x > 0 && src[i - 1])
                    || (x < w - 1 && src[i + 1])
                    || (y > 0 && src[i - w as usize])
                    || (y < h - 1 && src[i + w as usize]);
                if n {
                    self.px[i] = col;
                }
            }
        }
    }

    pub fn flipped(&self) -> Canvas {
        let mut out = Canvas::new(self.w, self.h);
        for y in 0..self.h {
            for x in 0..self.w {
                out.px[(y * self.w + x) as usize] = self.px[(y * self.w + self.w - 1 - x) as usize];
            }
        }
        out
    }

    /// Texte bitmap, coin haut-gauche en (x, y).
    pub fn text(&mut self, x: i32, y: i32, s: &str, c: C, font: &MonoFont) {
        let style = MonoTextStyle::new(font, Rgb888::new(c.0, c.1, c.2));
        let mut t = Target { cv: self, a: c.3 };
        let _ = Text::with_baseline(s, Point::new(x, y), style, Baseline::Top).draw(&mut t);
    }
}

pub fn text_width(s: &str, font: &MonoFont) -> i32 {
    s.chars().count() as i32 * (font.character_size.width + font.character_spacing) as i32
}

/// Coupe le texte pour qu'il tienne dans `max` pixels.
pub fn fit(s: &str, max: i32, font: &MonoFont) -> String {
    let cw = (font.character_size.width + font.character_spacing) as i32;
    let n = (max / cw).max(1) as usize;
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// Découpe en lignes d'au plus `cols` caractères, mot par mot.
pub fn wrap(s: &str, cols: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let wl = word.chars().count();
        let cl = cur.chars().count();
        if cl > 0 && cl + 1 + wl > cols {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        if wl > cols {
            cur.push_str(&word.chars().take(cols).collect::<String>());
        } else {
            cur.push_str(word);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

struct Target<'a> {
    cv: &'a mut Canvas,
    a: u8,
}

impl OriginDimensions for Target<'_> {
    fn size(&self) -> Size {
        Size::new(self.cv.w as u32, self.cv.h as u32)
    }
}

impl DrawTarget for Target<'_> {
    type Color = Rgb888;
    type Error = core::convert::Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb888>>>(&mut self, pixels: I) -> Result<(), Self::Error> {
        for Pixel(p, col) in pixels {
            self.cv.put(p.x, p.y, C(col.r(), col.g(), col.b(), self.a));
        }
        Ok(())
    }
}

/// Générateur pseudo-aléatoire déterministe (même graine, même décor).
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(seed)
    }
    pub fn f(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        self.0 as f32 / 4294967296.0
    }
    pub fn i(&mut self, n: i32) -> i32 {
        (self.f() * n as f32) as i32
    }
}
