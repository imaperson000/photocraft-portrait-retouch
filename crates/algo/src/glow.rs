//! Glow: bloom and light streaks grown from an image's highlights (Filter › Portrait Retouch ›
//! Glow). The result is meant for a layer in Screen mode above the image.
//!
//! An original design, matched to the visible behaviour of glow plug-ins by eye:
//! 1. **Bright pass**: every pixel whose brightest channel is past `threshold` (a hard step, or a
//!    soft ramp with `range`) is kept, boosted by `exposure`.
//! 2. **Bloom**: the bright pass blurred at five scales an octave apart and summed, so a light
//!    gets a tight core and a long soft tail. Scales are fractions of the image's long side, so
//!    the look does not depend on the resolution the glow is computed at.
//! 3. **Streaks**: `rays` spokes from every light, each a one-sided exponential trail built by
//!    repeated four-tap passes whose step grows fourfold (a classic real-time streak filter), at
//!    `rotation`. `detail` keeps the spokes crisp; at 0 the source is softened first.
//! 4. **Dispersion** scales the blur per channel (red reaches furthest, blue least), giving
//!    coloured fringes. **Colorize** recolours the glow with `hue` / `saturation` keeping its
//!    luminance; `lightness` darkens it and `intensity` scales the whole result.

use crate::par;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlowMode {
    Bloom,
    Streaks,
}

/// Glow settings in the units the dialog shows.
#[derive(Clone, Debug, PartialEq)]
pub struct GlowParams {
    pub mode: GlowMode,
    /// Spokes per light in streak mode (2 = one line through it, 4 = a cross).
    pub rays: u32,
    /// 0–100 %.
    pub intensity: f32,
    /// 0–100: glow size relative to the image's long side.
    pub radius: f32,
    /// 0.01–1: brightness a pixel needs to glow.
    pub threshold: f32,
    /// Soft threshold (a wide ramp instead of a step).
    pub range: bool,
    /// 0–255: gain on the bright pass.
    pub exposure: f32,
    /// 0–100: chromatic spread of the glow.
    pub dispersion: f32,
    /// −90–90 degrees: streak angle (0 = horizontal).
    pub rotation: f32,
    /// 0–100: streak crispness.
    pub detail: f32,
    pub colorize: bool,
    /// 0–360 degrees.
    pub hue: f32,
    /// 0–100 %.
    pub saturation: f32,
    /// −100–0.
    pub lightness: f32,
}

impl Default for GlowParams {
    fn default() -> Self {
        Self {
            mode: GlowMode::Bloom,
            rays: 2,
            intensity: 100.0,
            radius: 20.0,
            threshold: 0.25,
            range: false,
            exposure: 0.0,
            dispersion: 0.0,
            rotation: 0.0,
            detail: 0.0,
            colorize: false,
            hue: 0.0,
            saturation: 80.0,
            lightness: 0.0,
        }
    }
}

/// A planar RGB image (encoded values, 0–1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Planes {
    pub width: usize,
    pub height: usize,
    pub rgb: [Vec<f32>; 3],
}

impl Planes {
    /// A black image.
    pub fn new(width: usize, height: usize) -> Self {
        let n = width.saturating_mul(height);
        Self { width, height, rgb: [vec![0.0; n], vec![0.0; n], vec![0.0; n]] }
    }

    fn len(&self) -> usize {
        self.width.saturating_mul(self.height)
    }
}

/// Bloom scales relative to the base size, and their weights (summing to 1).
const BLOOM_SCALES: [(f32, f32); 5] = [(0.25, 0.3), (0.5, 0.25), (1.0, 0.2), (2.0, 0.15), (4.0, 0.1)];

/// Brightness of the bloom relative to the bright pass.
const BLOOM_GAIN: f32 = 2.0;

/// Channel scale of the glow's reach at full dispersion (red, green, blue).
const DISPERSION: [f32; 3] = [0.6, 0.0, -0.5];

/// The glow of `src` (black where nothing glows), the same size as `src`.
pub fn glow(src: &Planes, p: &GlowParams) -> Planes {
    let (w, h) = (src.width, src.height);
    let mut out = Planes::new(w, h);
    if w == 0 || h == 0 || src.rgb.iter().any(|c| c.len() != src.len()) {
        return out;
    }
    let bright = bright_pass(src, p);
    // Base size: 15 % of the long side at radius 100 (3 % at the default 20).
    let base = p.radius.clamp(0.0, 100.0) / 100.0 * w.max(h) as f32 * 0.15;
    let dispersion = p.dispersion.clamp(0.0, 100.0) / 100.0;
    for (c, plane) in out.rgb.iter_mut().enumerate() {
        let reach = 1.0 + DISPERSION.get(c).copied().unwrap_or(0.0) * dispersion;
        let Some(b) = bright.rgb.get(c) else { continue };
        *plane = match p.mode {
            GlowMode::Bloom => bloom(b, w, h, base * reach),
            GlowMode::Streaks => streaks(b, w, h, base * reach, p),
        };
    }
    finish(&mut out, p);
    out
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn bright_pass(src: &Planes, p: &GlowParams) -> Planes {
    let t = p.threshold.clamp(0.01, 1.0);
    let knee = if p.range { 0.25 } else { 0.02 };
    let gain = 1.0 + p.exposure.clamp(0.0, 255.0) / 255.0 * 15.0;
    let mut out = Planes::new(src.width, src.height);
    let [r, g, b] = &src.rgb;
    let [or, og, ob] = &mut out.rgb;
    for (((sr, sg), sb), ((dr, dg), db)) in r.iter().zip(g).zip(b).zip(or.iter_mut().zip(og.iter_mut()).zip(ob.iter_mut())) {
        let k = smoothstep(t - knee, t + knee, sr.max(*sg).max(*sb)) * gain;
        (*dr, *dg, *db) = (sr * k, sg * k, sb * k);
    }
    out
}

fn bloom(src: &[f32], w: usize, h: usize, base: f32) -> Vec<f32> {
    if base < 0.25 {
        return src.to_vec();
    }
    let mut acc = vec![0.0f32; src.len()];
    for (scale, weight) in BLOOM_SCALES {
        let blurred = gaussian(src, w, h, base * scale);
        for (a, v) in acc.iter_mut().zip(&blurred) {
            *a += v * weight * BLOOM_GAIN;
        }
    }
    acc
}

fn streaks(src: &[f32], w: usize, h: usize, base: f32, p: &GlowParams) -> Vec<f32> {
    let detail = p.detail.clamp(0.0, 100.0) / 100.0;
    let soft = gaussian(src, w, h, base * 0.2 * (1.0 - detail));
    let rays = p.rays.clamp(1, 16);
    // Trail length: six times the base size; weights fall to e⁻³ at its end.
    let length = (base * 6.0).max(1.0);
    let atten = (-3.0 / length).exp();
    // A spoke spreads a light over its length: scale it so trails read as clearly as the bloom.
    let gain = length.sqrt() / rays as f32 * 2.0;
    let mut acc = vec![0.0f32; src.len()];
    for i in 0..rays {
        let angle = (p.rotation.clamp(-90.0, 90.0) + i as f32 * 360.0 / rays as f32).to_radians();
        let dir = (angle.cos(), -angle.sin());
        let spoke = spoke(&soft, w, h, dir, length, atten);
        for (a, v) in acc.iter_mut().zip(&spoke) {
            *a += v * gain;
        }
    }
    acc
}

/// One spoke: light from `-dir` trails along `dir`. Each pass gathers four taps `step` apart
/// with exponentially falling weights; the step grows fourfold per pass until it covers `length`.
fn spoke(src: &[f32], w: usize, h: usize, dir: (f32, f32), length: f32, atten: f32) -> Vec<f32> {
    let mut cur = src.to_vec();
    let mut step = 1.0f32;
    let mut passes = 0;
    while step <= length && passes < 8 {
        let weights: [f32; 4] = std::array::from_fn(|s| atten.powf(step * s as f32));
        let total: f32 = weights.iter().sum();
        let prev = cur.clone();
        par::chunks_mut(&mut cur, w, h * w > 65_536, |y, row| {
            for (x, v) in row.iter_mut().enumerate() {
                let mut a = 0.0;
                for (s, wt) in weights.iter().enumerate() {
                    let d = step * s as f32;
                    a += wt * sample(&prev, w, h, x as f32 - dir.0 * d, y as f32 - dir.1 * d);
                }
                *v = a / total;
            }
        });
        step *= 4.0;
        passes += 1;
    }
    cur
}

/// Bilinear sample, black outside the image.
fn sample(src: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let at = |xi: f32, yi: f32| -> f32 {
        if xi < 0.0 || yi < 0.0 || xi >= w as f32 || yi >= h as f32 {
            return 0.0;
        }
        src.get(yi as usize * w + xi as usize).copied().unwrap_or(0.0)
    };
    let top = at(x0, y0) * (1.0 - fx) + at(x0 + 1.0, y0) * fx;
    let bottom = at(x0, y0 + 1.0) * (1.0 - fx) + at(x0 + 1.0, y0 + 1.0) * fx;
    top * (1.0 - fy) + bottom * fy
}

/// Box radii whose three passes approximate a Gaussian of `sigma` (Wells' method).
fn box_radii(sigma: f32) -> [usize; 3] {
    let ideal = (12.0 * sigma * sigma / 3.0 + 1.0).sqrt();
    let mut wl = ideal.floor() as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wl = wl.max(1);
    let wu = wl + 2;
    let m = ((12.0 * sigma * sigma - 3.0 * (wl * wl) as f32 - 12.0 * wl as f32 - 9.0) / (-4.0 * wl as f32 - 4.0)).round() as i64;
    std::array::from_fn(|i| {
        let width = if (i as i64) < m { wl } else { wu };
        (width.max(1) as usize - 1) / 2
    })
}

/// Gaussian blur of a plane (three box passes each way), black beyond the edges: light outside
/// the image does not exist.
pub fn gaussian(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    if sigma < 0.25 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let mut a = src.to_vec();
    let mut t = vec![0.0f32; a.len()];
    for r in box_radii(sigma) {
        box_rows(&a, &mut t, w, h, r);
        transpose(&t, &mut a, w, h);
        box_rows(&a, &mut t, h, w, r);
        transpose(&t, &mut a, h, w);
    }
    a
}

fn box_rows(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let norm = 1.0 / (2 * r + 1) as f32;
    par::chunks_mut(dst, w, w * h > 65_536, |y, out| {
        let Some(row) = src.get(y * w..(y + 1) * w) else { return };
        let mut sum: f32 = row.iter().take(r).sum();
        for (x, o) in out.iter_mut().enumerate() {
            if let Some(v) = row.get(x + r) {
                sum += v;
            }
            *o = sum * norm;
            if let Some(v) = x.checked_sub(r).and_then(|i| row.get(i)) {
                sum -= v;
            }
        }
    });
}

fn transpose(src: &[f32], dst: &mut [f32], w: usize, h: usize) {
    par::chunks_mut(dst, h, w * h > 65_536, |x, col| {
        for (y, d) in col.iter_mut().enumerate() {
            *d = src.get(y * w + x).copied().unwrap_or(0.0);
        }
    });
}

fn hue_rgb(hue: f32) -> [f32; 3] {
    let h = hue.rem_euclid(360.0) / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    match h as u32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    }
}

fn luma(c: [f32; 3]) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

fn finish(out: &mut Planes, p: &GlowParams) {
    let scale = p.intensity.clamp(0.0, 100.0) / 100.0 * (1.0 + p.lightness.clamp(-100.0, 0.0) / 100.0);
    let tint = p.colorize.then(|| {
        let c = hue_rgb(p.hue);
        let l = luma(c).max(1e-3);
        let s = p.saturation.clamp(0.0, 100.0) / 100.0;
        c.map(|v| 1.0 - s + s * v / l)
    });
    let [r, g, b] = &mut out.rgb;
    for ((r, g), b) in r.iter_mut().zip(g.iter_mut()).zip(b.iter_mut()) {
        let mut c = [*r, *g, *b];
        if let Some(t) = tint {
            let l = luma(c);
            c = [l * t[0], l * t[1], l * t[2]];
        }
        [*r, *g, *b] = c.map(|v| (v * scale).clamp(0.0, 1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A black image with one bright square in the middle.
    fn spot(w: usize, h: usize) -> Planes {
        let mut p = Planes::new(w, h);
        for y in h / 2 - 2..h / 2 + 2 {
            for x in w / 2 - 2..w / 2 + 2 {
                for c in &mut p.rgb {
                    c[y * w + x] = 1.0;
                }
            }
        }
        p
    }

    fn at(p: &Planes, c: usize, x: usize, y: usize) -> f32 {
        p.rgb[c][y * p.width + x]
    }

    #[test]
    fn gaussian_matches_its_sigma_and_keeps_energy() {
        let (w, h) = (101, 101);
        let mut src = vec![0.0f32; w * h];
        src[50 * w + 50] = 1.0;
        let g = gaussian(&src, w, h, 6.0);
        let total: f32 = g.iter().sum();
        assert!((total - 1.0).abs() < 1e-3, "energy {total}");
        // Variance along a row of the 2-D result ≈ σ².
        let row = &g[50 * w..51 * w];
        let s: f32 = row.iter().sum();
        let var: f32 = row.iter().enumerate().map(|(x, v)| v * (x as f32 - 50.0).powi(2)).sum::<f32>() / s;
        assert!((var.sqrt() - 6.0).abs() < 0.6, "sigma {}", var.sqrt());
        assert_eq!(gaussian(&src, w, h, 0.0), src);
        assert!(gaussian(&[], 0, 0, 5.0).is_empty());
    }

    #[test]
    fn bloom_spreads_light_and_threshold_gates_it() {
        let src = spot(80, 60);
        let g = glow(&src, &GlowParams::default());
        assert!(at(&g, 0, 40, 30) > 0.3, "core");
        assert!(at(&g, 0, 50, 30) > 0.0, "tail");
        assert!(at(&g, 0, 40, 30) > at(&g, 0, 50, 30));
        // Symmetric: bloom has no direction.
        assert!((at(&g, 0, 50, 30) - at(&g, 0, 40, 40)).abs() < 0.02);
        // A dim spot under the threshold does not glow.
        let mut dim = src.clone();
        for c in &mut dim.rgb {
            c.iter_mut().for_each(|v| *v *= 0.2);
        }
        assert!(glow(&dim, &GlowParams::default()).rgb[0].iter().all(|v| *v == 0.0));
        // Exposure brightens; intensity 0 and lightness −100 switch it off.
        let bright = glow(&src, &GlowParams { exposure: 255.0, ..Default::default() });
        assert!(at(&bright, 0, 50, 30) > at(&g, 0, 50, 30));
        for p in [GlowParams { intensity: 0.0, ..Default::default() }, GlowParams { lightness: -100.0, ..Default::default() }] {
            assert!(glow(&src, &p).rgb.iter().flatten().all(|v| *v == 0.0));
        }
    }

    #[test]
    fn streaks_follow_rays_and_rotation() {
        let src = spot(120, 120);
        let p = GlowParams { mode: GlowMode::Streaks, radius: 40.0, detail: 100.0, ..Default::default() };
        let g = glow(&src, &p);
        // Two rays at 0°: a horizontal line through the light, nothing straight above it.
        assert!(at(&g, 0, 90, 60) > 0.01, "right {}", at(&g, 0, 90, 60));
        assert!(at(&g, 0, 30, 60) > 0.01, "left");
        assert!(at(&g, 0, 60, 30) < at(&g, 0, 90, 60) * 0.1, "above");
        // Rotated 90°: vertical.
        let v = glow(&src, &GlowParams { rotation: 90.0, ..p.clone() });
        assert!(at(&v, 0, 60, 30) > 0.01 && at(&v, 0, 90, 60) < at(&v, 0, 60, 30) * 0.1);
        // Four rays: a cross.
        let x = glow(&src, &GlowParams { rays: 4, ..p });
        assert!(at(&x, 0, 90, 60) > 0.005 && at(&x, 0, 60, 30) > 0.005);
    }

    #[test]
    fn dispersion_and_colorize_tint_the_glow() {
        let src = spot(80, 60);
        let g = glow(&src, &GlowParams { dispersion: 100.0, ..Default::default() });
        // Red reaches further than blue.
        assert!(at(&g, 0, 60, 30) > at(&g, 2, 60, 30) * 1.5);
        let c = glow(&src, &GlowParams { colorize: true, hue: 220.0, saturation: 100.0, ..Default::default() });
        assert!(at(&c, 2, 45, 30) > at(&c, 0, 45, 30), "blue tint");
        let gray = glow(&src, &GlowParams { colorize: true, saturation: 0.0, ..Default::default() });
        let plain = glow(&src, &GlowParams::default());
        assert!((at(&gray, 0, 45, 30) - at(&plain, 0, 45, 30)).abs() < 1e-4, "saturation 0 = no tint on white light");
    }

    #[test]
    fn odd_inputs_do_not_panic() {
        assert_eq!(glow(&Planes::default(), &GlowParams::default()), Planes::default());
        let bad = Planes { width: 4, height: 4, rgb: [vec![1.0; 3], vec![], vec![]] };
        assert_eq!(glow(&bad, &GlowParams::default()).rgb[0], vec![0.0; 16]);
        let one = spot(5, 5);
        for p in [
            GlowParams { radius: 100.0, mode: GlowMode::Streaks, rays: 99, ..Default::default() },
            GlowParams { radius: f32::NAN, threshold: f32::INFINITY, exposure: -5.0, ..Default::default() },
            GlowParams { radius: 0.0, ..Default::default() },
        ] {
            let g = glow(&one, &p);
            assert!(g.rgb.iter().flatten().all(|v| v.is_finite() || v.is_nan()));
        }
    }
}
