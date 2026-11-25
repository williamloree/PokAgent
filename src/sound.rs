//! Petit jingle 8 bits généré en mémoire (WAV), différent par univers.

use crate::themes::Theme;

const RATE: u32 = 22050;

pub fn jingle(theme: Theme) -> Vec<u8> {
    let (notes, tri, vol): (&[(f32, f32)], bool, f32) = match theme {
        Theme::Labo => (&[(1318.5, 0.0), (1975.5, 0.09), (1975.5, 0.2)], false, 0.18),
        Theme::Village => (&[(784.0, 0.0), (988.0, 0.1), (1175.0, 0.2), (1568.0, 0.3)], true, 0.35),
        Theme::Donjon => (&[(523.0, 0.0), (659.0, 0.1), (784.0, 0.2), (1046.0, 0.32)], false, 0.16),
    };
    let dur = notes.iter().map(|n| n.1).fold(0.0, f32::max) + 0.12;
    let n = (dur * RATE as f32) as usize;
    let mut s = vec![0f32; n];
    for &(f, t0) in notes {
        let a = (t0 * RATE as f32) as usize;
        let len = (0.1 * RATE as f32) as usize;
        for i in 0..len {
            if a + i >= n {
                break;
            }
            let ph = (i as f32 * f / RATE as f32).fract();
            let w = if tri { 4.0 * (ph - 0.5).abs() - 1.0 } else if ph < 0.5 { 1.0 } else { -1.0 };
            let env = (1.0 - i as f32 / len as f32).powi(2);
            s[a + i] += w * env * vol;
        }
    }
    let data: Vec<u8> = s.iter().map(|v| ((v.clamp(-1.0, 1.0) * 127.0) + 128.0) as u8).collect();
    let mut out = Vec::with_capacity(44 + data.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes()); // octets/s
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&8u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    out
}
