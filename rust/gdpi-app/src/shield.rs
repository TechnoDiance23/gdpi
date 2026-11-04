//! VPN tarzı kalkan simgesi (Python `shield_image`): açıkken yeşil + tik, kapalıyken gri + güç işareti.
//! Pencere, tepsi, üst bar ve Windows .ico için aynı çizim. Sadece std kullanır (build.rs de içeri alır).

const SHIELD: [(f32, f32); 8] = [
    (128.0, 14.0), (226.0, 50.0), (222.0, 132.0), (196.0, 190.0),
    (128.0, 244.0), (60.0, 190.0), (34.0, 132.0), (30.0, 50.0),
];

fn inside(x: f32, y: f32) -> bool {
    let mut c = false;
    let n = SHIELD.len();
    for i in 0..n {
        let (xi, yi) = SHIELD[i];
        let (xj, yj) = SHIELD[(i + n - 1) % n];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            c = !c;
        }
    }
    c
}

fn seg_dist(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((px - a.0) * dx + (py - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    let (qx, qy) = (a.0 + t * dx - px, a.1 + t * dy - py);
    (qx * qx + qy * qy).sqrt()
}

/// 256'lık tuvaldeki bir noktanın rengi (RGBA, 0..1).
fn sample(on: bool, x: f32, y: f32) -> [f32; 4] {
    if !inside(x, y) {
        return [0.0; 4];
    }
    let white = if on {
        let pts = [(84.0, 130.0), (116.0, 162.0), (176.0, 98.0)];
        seg_dist(x, y, pts[0], pts[1]) <= 13.0 || seg_dist(x, y, pts[1], pts[2]) <= 13.0
    } else {
        // PIL arc((78,82,178,182), -60, 240, width=20): r 30..50, üstte -120..-60 derece boşluk
        let (dx, dy) = (x - 128.0, y - 132.0);
        let r = (dx * dx + dy * dy).sqrt();
        let a = dy.atan2(dx).to_degrees();
        let ring = (30.0..=50.0).contains(&r) && !(-120.0..-60.0).contains(&a);
        ring || (x - 128.0).abs() <= 10.0 && (66.0..=130.0).contains(&y)
    };
    if white {
        return [1.0, 1.0, 1.0, 1.0];
    }
    let (top, bot): ([f32; 3], [f32; 3]) = if on {
        ([52.0, 211.0, 153.0], [5.0, 150.0, 105.0])
    } else {
        ([100.0, 116.0, 139.0], [51.0, 65.0, 85.0])
    };
    let t = (y / 256.0).clamp(0.0, 1.0);
    let c = |k: usize| (top[k] + (bot[k] - top[k]) * t) / 255.0;
    [c(0), c(1), c(2), 1.0]
}

/// `size`x`size` RGBA (düz, önçarpımsız) piksel dizisi.
pub fn shield_rgba(on: bool, size: u32) -> Vec<u8> {
    let n = 4u32;
    let scale = 256.0 / size as f32;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0f32; 4];
            for sy in 0..n {
                for sx in 0..n {
                    let x = (px as f32 + (sx as f32 + 0.5) / n as f32) * scale;
                    let y = (py as f32 + (sy as f32 + 0.5) / n as f32) * scale;
                    let s = sample(on, x, y);
                    for k in 0..3 {
                        acc[k] += s[k] * s[3];
                    }
                    acc[3] += s[3];
                }
            }
            let cnt = (n * n) as f32;
            let a = acc[3] / cnt;
            for v in acc.iter().take(3) {
                let c = if acc[3] > 0.0 { v / acc[3] } else { 0.0 };
                out.push((c * 255.0).round() as u8);
            }
            out.push((a * 255.0).round() as u8);
        }
    }
    out
}

/// Windows .ico (32 bit BMP girdileri).
#[allow(dead_code)]
pub fn shield_ico(on: bool, sizes: &[u32]) -> Vec<u8> {
    let mut imgs = Vec::new();
    for &s in sizes {
        let rgba = shield_rgba(on, s);
        let mut b = Vec::new();
        let mask_row = s.div_ceil(32) * 4;
        b.extend_from_slice(&40u32.to_le_bytes());
        b.extend_from_slice(&(s as i32).to_le_bytes());
        b.extend_from_slice(&((s * 2) as i32).to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&32u16.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&(s * s * 4 + mask_row * s).to_le_bytes());
        b.extend_from_slice(&[0u8; 16]);
        for y in (0..s).rev() {
            for x in 0..s {
                let i = ((y * s + x) * 4) as usize;
                b.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
            }
        }
        b.extend(std::iter::repeat_n(0u8, (mask_row * s) as usize));
        imgs.push((s, b));
    }
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(imgs.len() as u16).to_le_bytes());
    let mut off = 6 + 16 * imgs.len() as u32;
    for (s, b) in &imgs {
        let d = if *s >= 256 { 0 } else { *s as u8 };
        out.extend_from_slice(&[d, d, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(b.len() as u32).to_le_bytes());
        out.extend_from_slice(&off.to_le_bytes());
        off += b.len() as u32;
    }
    for (_, b) in imgs {
        out.extend(b);
    }
    out
}
