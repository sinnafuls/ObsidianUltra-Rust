//! Color helpers mirroring `Library:GetBetterColor/GetLighterColor/GetDarkerColor`
//! plus HSV and hex conversions used by the color picker.

use epaint::Color32;

/// A color with Obsidian-style transparency (`0` = opaque, `1` = invisible).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub color: Color32,
    pub transparency: f32,
}

impl Rgba {
    pub const fn opaque(color: Color32) -> Self {
        Self { color, transparency: 0.0 }
    }

    /// Alpha in `0..=1` (`1` = opaque).
    pub fn alpha(&self) -> f32 {
        (1.0 - self.transparency).clamp(0.0, 1.0)
    }

    /// The color with its transparency baked into the alpha channel.
    pub fn to_color32(&self) -> Color32 {
        with_alpha(self.color, self.transparency)
    }
}

impl From<Color32> for Rgba {
    fn from(color: Color32) -> Self {
        Rgba::opaque(color)
    }
}

/// Shifts every channel by `add * 2` (dark theme) or `add * -4` (light theme).
pub fn better(c: Color32, add: f32, is_light: bool) -> Color32 {
    let add = add * if is_light { -4.0 } else { 2.0 };
    let ch = |v: u8| (v as f32 + add).clamp(0.0, 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(ch(c.r()), ch(c.g()), ch(c.b()), c.a())
}

/// Lighter variant: saturation -0.1, value +0.1.
pub fn lighter(c: Color32) -> Color32 {
    let (h, s, v) = to_hsv(c);
    from_hsv(h, (s - 0.1).max(0.0), (v + 0.1).min(1.0))
}

/// Darker variant: value halved.
pub fn darker(c: Color32) -> Color32 {
    let (h, s, v) = to_hsv(c);
    from_hsv(h, s, v / 2.0)
}

/// Apply an Obsidian-style transparency (0 = opaque) to an opaque color.
pub fn with_alpha(c: Color32, transparency: f32) -> Color32 {
    let a = ((1.0 - transparency).clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Multiply the color's alpha by `f`.
pub fn fade(c: Color32, f: f32) -> Color32 {
    let a = (c.a() as f32 * f.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Linear interpolation between two colors (unmultiplied channels).
pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Multiply two colors channel-wise (Roblox `UIGradient` over a colored frame).
pub fn multiply(a: Color32, b: Color32) -> Color32 {
    let m = |x: u8, y: u8| ((x as u32 * y as u32) / 255) as u8;
    Color32::from_rgba_unmultiplied(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()), a.a())
}

/// RGB -> HSV, all components in `0..=1`.
pub fn to_hsv(c: Color32) -> (f32, f32, f32) {
    let r = c.r() as f32 / 255.0;
    let g = c.g() as f32 / 255.0;
    let b = c.b() as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let v = max;
    let s = if max > 0.0 { d / max } else { 0.0 };
    let h = if d <= 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, s, v)
}

/// HSV -> RGB, all components in `0..=1`.
pub fn from_hsv(h: f32, s: f32, v: f32) -> Color32 {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let u = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(u(r), u(g), u(b))
}

/// `#RRGGBB` (uppercase).
pub fn to_hex(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

/// Parse `#RRGGBB` / `RRGGBB` / `#RGB`.
pub fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    let parse = |h: &str| u8::from_str_radix(h, 16).ok();
    match s.len() {
        6 => Some(Color32::from_rgb(parse(&s[0..2])?, parse(&s[2..4])?, parse(&s[4..6])?)),
        3 => {
            let x = |i: usize| parse(&s[i..i + 1]).map(|v| v * 17);
            Some(Color32::from_rgb(x(0)?, x(1)?, x(2)?))
        }
        _ => None,
    }
}

/// `"r, g, b"`.
pub fn to_rgb_string(c: Color32) -> String {
    format!("{}, {}, {}", c.r(), c.g(), c.b())
}

/// Parse `"r, g, b"` (Lua pattern `(%d+),%s*(%d+),%s*(%d+)`).
pub fn parse_rgb(s: &str) -> Option<Color32> {
    let mut it = s.split(',').map(|p| p.trim().parse::<u32>().ok());
    let r = it.next()??;
    let g = it.next()??;
    let b = it.next()??;
    if r > 255 || g > 255 || b > 255 {
        return None;
    }
    Some(Color32::from_rgb(r as u8, g as u8, b as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn better_direction_depends_on_theme() {
        let c = Color32::from_rgb(100, 100, 100);
        assert_eq!(better(c, 4.0, false), Color32::from_rgb(108, 108, 108));
        assert_eq!(better(c, 4.0, true), Color32::from_rgb(84, 84, 84));
        assert_eq!(better(Color32::from_rgb(254, 0, 0), 4.0, false).r(), 255);
    }

    #[test]
    fn hsv_roundtrip_and_hex() {
        let c = Color32::from_rgb(125, 85, 255);
        let (h, s, v) = to_hsv(c);
        assert_eq!(from_hsv(h, s, v), c);
        assert_eq!(to_hex(c), "#7D55FF");
        assert_eq!(parse_hex("#7d55ff"), Some(c));
        assert_eq!(parse_hex("7D55FF"), Some(c));
        assert_eq!(parse_hex("#zzz"), None);
        assert_eq!(parse_rgb("125, 85,255"), Some(c));
        assert_eq!(parse_rgb("300,0,0"), None);
        assert_eq!(darker(Color32::WHITE), Color32::from_rgb(128, 128, 128));
    }
}
