//! Colours: parsing, OKLab, hue-shifted ramps, luminance and contrast.

pub type Rgba = [u8; 4];

pub const CLEAR: Rgba = [0, 0, 0, 0];

pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    [r, g, b, 255]
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa`.
pub fn parse_hex(s: &str) -> Option<Rgba> {
    let h = s.strip_prefix('#')?;
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    match h.len() {
        3 => {
            let d = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|v| v * 17);
            Some([d(0)?, d(1)?, d(2)?, 255])
        }
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}

pub fn hex(c: Rgba) -> String {
    if c[3] == 255 {
        format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3])
    }
}

/// A GameMaker colour (0xBBGGRR) as RGBA.
pub fn from_gm(c: u32) -> Rgba {
    [(c & 0xff) as u8, ((c >> 8) & 0xff) as u8, ((c >> 16) & 0xff) as u8, 255]
}

fn to_linear(v: u8) -> f64 {
    let v = v as f64 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(v: f64) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Relative luminance (WCAG), 0..1.
pub fn luminance(c: Rgba) -> f64 {
    0.2126 * to_linear(c[0]) + 0.7152 * to_linear(c[1]) + 0.0722 * to_linear(c[2])
}

/// WCAG contrast ratio between two luminances, 1..21.
pub fn contrast(l1: f64, l2: f64) -> f64 {
    let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

/// OKLab (L 0..1, a, b).
pub fn oklab(c: Rgba) -> [f64; 3] {
    let (r, g, b) = (to_linear(c[0]), to_linear(c[1]), to_linear(c[2]));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    ]
}

pub fn from_oklab([ll, a, b]: [f64; 3], alpha: u8) -> Rgba {
    let l = (ll + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (ll - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (ll - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    [
        from_linear(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s),
        from_linear(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s),
        from_linear(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s),
        alpha,
    ]
}

/// Perceptual distance (OKLab, ~0.02 is barely visible).
pub fn distance(a: Rgba, b: Rgba) -> f64 {
    let (p, q) = (oklab(a), oklab(b));
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
}

/// Hue angle (degrees) that shadows lean towards: blue-violet.
pub const SHADOW_HUE: f64 = 285.0;
/// Hue angle (degrees) that highlights lean towards: warm yellow.
pub const LIGHT_HUE: f64 = 95.0;

/// Moves `c` by `steps` lightness steps: negative darkens towards violet, positive
/// lightens towards yellow (the "hue shift" painters use instead of plain black/white).
/// `hue` is the hue shift per step in degrees.
pub fn step(c: Rgba, steps: f64, hue: f64) -> Rgba {
    if steps == 0.0 {
        return c;
    }
    let [l, a, b] = oklab(c);
    let chroma = (a * a + b * b).sqrt();
    let mut h = b.atan2(a).to_degrees();
    let (target, dl, dc): (f64, f64, f64) = if steps < 0.0 { (SHADOW_HUE, -0.105, 0.96) } else { (LIGHT_HUE, 0.085, 0.86) };
    let n = steps.abs();
    // rotate the hue towards the target by at most `hue * n` degrees
    let mut diff = (target - h).rem_euclid(360.0);
    if diff > 180.0 {
        diff -= 360.0;
    }
    let turn = (hue * n).min(diff.abs()) * diff.signum();
    // grey colours have no hue to keep: give them a faint tint of the target
    let chroma2 = if chroma < 0.01 { 0.012 * n.min(2.0) } else { chroma * dc.powf(n) };
    if chroma >= 0.01 {
        h += turn;
    } else {
        h = target;
    }
    let l2 = (l + dl * n).clamp(0.04, 0.985);
    let hr = h.to_radians();
    from_oklab([l2, chroma2 * hr.cos(), chroma2 * hr.sin()], c[3])
}

/// `n` colours from dark to light with `base` at index `n / 2`.
pub fn ramp(base: Rgba, n: usize, hue: f64) -> Vec<Rgba> {
    let mid = n / 2;
    (0..n).map(|i| step(base, i as f64 - mid as f64, hue)).collect()
}

pub fn mix(a: Rgba, b: Rgba, t: f64) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
    [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2]), f(a[3], b[3])]
}

/// `top` drawn over `bottom` with alpha blending.
pub fn over(top: Rgba, bottom: Rgba) -> Rgba {
    let ta = top[3] as f64 / 255.0;
    if ta >= 1.0 {
        return top;
    }
    let ba = bottom[3] as f64 / 255.0;
    let oa = ta + ba * (1.0 - ta);
    if oa <= 0.0 {
        return CLEAR;
    }
    let ch = |t: u8, b: u8| ((t as f64 * ta + b as f64 * ba * (1.0 - ta)) / oa).round() as u8;
    [ch(top[0], bottom[0]), ch(top[1], bottom[1]), ch(top[2], bottom[2]), (oa * 255.0).round() as u8]
}

/// Adds light (additive blend) of colour `light` at strength `k` (0..1+).
pub fn add_light(c: Rgba, light: Rgba, k: f64) -> Rgba {
    let f = |v: u8, l: u8| (v as f64 + l as f64 * k).round().clamp(0.0, 255.0) as u8;
    [f(c[0], light[0]), f(c[1], light[1]), f(c[2], light[2]), c[3]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        assert_eq!(parse_hex("#c82832"), Some([0xc8, 0x28, 0x32, 255]));
        assert_eq!(parse_hex("#fff"), Some([255, 255, 255, 255]));
        assert_eq!(parse_hex("#00000080"), Some([0, 0, 0, 128]));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(hex([0xc8, 0x28, 0x32, 255]), "#c82832");
    }

    #[test]
    fn oklab_round_trip() {
        for c in [rgb(200, 40, 50), rgb(0, 0, 0), rgb(255, 255, 255), rgb(30, 160, 90)] {
            assert_eq!(from_oklab(oklab(c), 255), c);
        }
    }

    #[test]
    fn ramp_goes_dark_to_light() {
        let r = ramp(rgb(200, 40, 50), 5, 8.0);
        assert_eq!(r[2], rgb(200, 40, 50));
        for w in r.windows(2) {
            assert!(luminance(w[0]) < luminance(w[1]));
        }
    }
}
