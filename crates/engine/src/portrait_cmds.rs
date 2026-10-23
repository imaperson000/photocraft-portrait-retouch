//! Filter › Portrait Retouch: one-step layer kits for skin retouching, built from PhotoCraft's own
//! filters, blend modes and adjustment layers so every piece stays editable afterwards.
//!
//! - **Frequency Separation** (`portrait.frequencySeparation`): the visible image split into a
//!   Gaussian-blurred low-frequency layer (Normal) and a high-frequency layer holding
//!   `(image − low) / 2 + 50% gray` (Linear Light), grouped. Linear Light adds `2·high − 1` back, so
//!   the pair reproduces the image: exactly at 16 and 32 bits, within one level at 8 bits (the
//!   halved difference is stored in 8 bits, as Photoshop's Apply Image recipe does).
//! - **Dodge & Burn** (`portrait.dodgeBurn`): a group with a 50% gray Soft Light (or Overlay) layer
//!   to paint light and shadow on, optional brighten / darken Curves layers behind black masks, and
//!   a viewing aid (Black & White plus a contrast curve) on top that shows tone without colour.
//!
//! - **Skin Smoothing** (`portrait.skinSmooth`): a smoothed copy of the visible image on its own
//!   layer with a mask. The copy keeps the image's edges (Surface Blur) and its fine texture (the
//!   detail finer than a small Gaussian), and evens out the band in between, where blotches and
//!   uneven tone live: `image + strength · (surfaceBlur(image) − gaussian(image, detail))`. The
//!   mask starts as a soft skin-tone map (chroma range in YCbCr), hidden, or revealed.
//! - **Skin Detail View** (`portrait.skinDetailView`, `…Solarize`): a toggled viewing aid on top
//!   of the stack (Black & White plus a steep or solarizing curve) that makes uneven skin visible.
//!
//! Layer names are Traditional Chinese, as the person who asked for these kits wanted. They are
//! document data, so they stay the same whatever the UI language is.

use photocraft_algo::FilterParams;
use photocraft_color::{ColorMode, PixelFormat};
use photocraft_doc::adjust::CurvePoint;
use photocraft_doc::{Adjustment, BlendMode, Document, Layer, LayerContent, LayerId, LayerMask};
use photocraft_geom::Rect;
use photocraft_raster::Surface;
use serde_json::{Value, json};

use crate::commands::CommandSpec;
use crate::{EngineError, Result, Session};

pub const FREQUENCY_SEPARATION: &str = "portrait.frequencySeparation";
pub const DODGE_BURN: &str = "portrait.dodgeBurn";
pub const SKIN_SMOOTH: &str = "portrait.skinSmooth";
pub const DETAIL_VIEW: &str = "portrait.skinDetailView";
pub const DETAIL_VIEW_SOLARIZE: &str = "portrait.skinDetailViewSolarize";

/// XMP namespace marking the skin detail view group.
const VIEW_NS: &str = "https://photocraft.app/ns/retouch-view/1.0/";

/// The largest blur radius the low-frequency layer accepts (as the Gaussian Blur dialog's range
/// would allow far more, but past this the "low" layer is just the average colour).
const MAX_RADIUS: f64 = 250.0;

/// Rows processed at a time, so a large document never needs full-size float copies.
const BAND_ROWS: i32 = 256;

const GROUP_FS: &str = "頻率分離";
const LOW: &str = "低頻";
const LOW_RETOUCH: &str = "低頻修飾";
const HIGH: &str = "高頻";
const GROUP_DB: &str = "加深減淡";
const GRAY: &str = "中性灰";
const DODGE: &str = "加亮";
const BURN: &str = "加深";
const HELPER: &str = "觀察組";
const HELPER_BW: &str = "黑白";
const HELPER_CURVE: &str = "對比";
const SMOOTH: &str = "磨皮";
const VIEW_GROUP: &str = "皮膚細節檢視";

fn bad(cmd: &str, msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

fn object(p: &Value, cmd: &str) -> Result<()> {
    match p {
        Value::Object(_) | Value::Null => Ok(()),
        _ => Err(bad(cmd, "expected an object")),
    }
}

fn flag(p: &Value, cmd: &str, key: &str, default: bool) -> Result<bool> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v.as_bool().ok_or_else(|| bad(cmd, format!("`{key}` must be true or false"))),
    }
}

fn choice<'a>(p: &'a Value, cmd: &str, key: &str, options: &[&'a str], default: &'a str) -> Result<&'a str> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v.as_str().filter(|s| options.contains(s)).ok_or_else(|| bad(cmd, format!("`{key}` must be one of {}", options.join(", ")))),
    }
}

/// The kits work on colour values channel by channel, which is what the blend modes do in RGB and
/// Grayscale; CMYK and Lab blend differently, so they are refused rather than built wrong.
fn supported(s: &Session) -> std::result::Result<(), String> {
    let st = s.active().ok_or("no document")?;
    match st.doc.pixel_format().mode {
        ColorMode::Rgb | ColorMode::Grayscale => Ok(()),
        m => Err(format!("portrait retouch kits need an RGB or Grayscale document (this one is {m:?})")),
    }
}

/// The document's pixel format with an alpha channel.
fn with_alpha(doc: &Document) -> PixelFormat {
    let f = doc.pixel_format();
    PixelFormat::new(f.mode, f.sample, true)
}

/// A blur radius that suits the document when the caller leaves it at 0: about one pixel per 600
/// pixels of the long side (10 px on a 24 MP photo), the scale of skin texture.
pub fn auto_radius(doc: &Document) -> f32 {
    let b = doc.bounds();
    (b.width().max(b.height()) as f32 / 600.0).clamp(2.0, 30.0)
}

fn radius(p: &Value, cmd: &str, doc: &Document) -> Result<f32> {
    let r = match p.get("radius") {
        None | Some(Value::Null) => 0.0,
        Some(v) => v.as_f64().filter(|r| r.is_finite()).ok_or_else(|| bad(cmd, "`radius` must be a number of pixels (0 = automatic)"))?,
    };
    if !(0.0..=MAX_RADIUS).contains(&r) {
        return Err(bad(cmd, format!("`radius` must be from 0 to {MAX_RADIUS} (0 = automatic)")));
    }
    Ok(if r < 0.1 { auto_radius(doc) } else { r as f32 })
}

fn alloc(e: photocraft_raster::AllocationError) -> EngineError {
    EngineError::Other(format!("not enough memory for the retouch layers: {e}"))
}

/// The low- and high-frequency surfaces of `image` over `bounds`.
fn split_frequencies(image: &Surface, bounds: Rect, radius: f32) -> Result<(Surface, Surface)> {
    let low = photocraft_algo::apply_in(image, &FilterParams::GaussianBlur { radius }, bounds, bounds, None, bounds);
    let fmt = image.format();
    let n = fmt.channels();
    let colors = n.saturating_sub(usize::from(fmt.alpha));
    let mut high = Surface::new(fmt);
    let mut y = bounds.y0;
    while y < bounds.y1 {
        let band = Rect::new(bounds.x0, y, bounds.x1, (y + BAND_ROWS).min(bounds.y1));
        let orig = image.try_read_region(band).map_err(alloc)?;
        let mut out = low.try_read_region(band).map_err(alloc)?;
        for (o, l) in orig.chunks_exact(n).zip(out.chunks_exact_mut(n)) {
            for c in 0..n {
                let (Some(&ov), Some(lv)) = (o.get(c), l.get_mut(c)) else { continue };
                // Colour: half the detail around 50% gray. Alpha: the image's own.
                *lv = if c < colors { ((ov - *lv) * 0.5 + 0.5).clamp(0.0, 1.0) } else { ov };
            }
        }
        high.try_write_region(band, &out).map_err(alloc)?;
        y = band.y1;
    }
    high.prune();
    Ok((low, high))
}

/// The top-level layer holding `active` (itself when it is not in a group): a kit goes above it,
/// so running a kit while working inside another kit's group never nests the two.
fn top_level(doc: &Document, active: Option<LayerId>) -> Option<LayerId> {
    let path = doc.path_of(active?)?;
    doc.layer_at(path.get(..1)?).map(|l| l.id)
}

fn select_only(s: &mut Session, id: LayerId) {
    if let Some(st) = s.active_mut() {
        st.selected_layers = vec![id];
        st.active_layer = Some(id);
        st.layer_anchor = Some(id);
        crate::fix_selection(st);
    }
}

fn frequency_separation(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = FREQUENCY_SEPARATION;
    object(p, cmd)?;
    supported(s).map_err(EngineError::Other)?;
    let retouch = flag(p, cmd, "retouchLayer", false)?;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let radius = radius(p, cmd, &st.doc)?;
    let (group, high_id, low_id) = s.edit("Frequency Separation", |doc, active| {
        let bounds = doc.bounds();
        if bounds.is_empty() {
            return Err(EngineError::Other("the document has no pixels".into()));
        }
        let fmt = with_alpha(doc);
        let image = photocraft_compose::flatten_to_surface(doc, fmt, None);
        let (low_px, high_px) = split_frequencies(&image, bounds, radius)?;
        let mut low = Layer::raster(LOW, fmt);
        *crate::pixels_mut(&mut low)? = low_px;
        let mut high = Layer::raster(HIGH, fmt);
        *crate::pixels_mut(&mut high)? = high_px;
        high.blend = BlendMode::LinearLight;
        let (low_id, high_id) = (low.id, high.id);
        // Bottom to top: low, (blank layer to paint low-frequency fixes on), high.
        let mut children = vec![low];
        if retouch {
            children.push(Layer::raster(LOW_RETOUCH, fmt));
        }
        children.push(high);
        let group = doc.insert_above(top_level(doc, *active), Layer::group(GROUP_FS, children));
        // High frequency is where texture work (healing, cloning) starts.
        *active = Some(high_id);
        Ok((group, high_id, low_id))
    })?;
    select_only(s, high_id);
    Ok(json!({ "group": group.0, "low": low_id.0, "high": high_id.0, "radius": radius }))
}

fn curve(points: &[(f32, f32)]) -> Vec<CurvePoint> {
    points.iter().map(|&(input, output)| CurvePoint { input, output }).collect()
}

/// A Curves adjustment layer whose composite curve runs through `points`.
fn curves_layer(name: &str, mode: ColorMode, points: &[(f32, f32)]) -> Result<Layer> {
    let mut adj = crate::adjust_params::default_for("curves", mode)?;
    if let Adjustment::Curves { master, .. } = &mut adj {
        *master = curve(points);
    }
    Ok(Layer::new(name, LayerContent::Adjustment(adj)))
}

fn dodge_burn(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = DODGE_BURN;
    object(p, cmd)?;
    supported(s).map_err(EngineError::Other)?;
    let method = choice(p, cmd, "method", &["gray", "curves", "both"], "gray")?;
    let blend = match choice(p, cmd, "blend", &["softLight", "overlay"], "softLight")? {
        "overlay" => BlendMode::Overlay,
        _ => BlendMode::SoftLight,
    };
    let helper = flag(p, cmd, "helper", true)?;
    let (group, paint_on, layers) = s.edit("Dodge & Burn", |doc, active| {
        let mode = doc.pixel_format().mode;
        let bounds = doc.bounds();
        // Bottom to top: gray, burn, dodge, viewing aid.
        let mut children = Vec::new();
        if method != "curves" {
            let mut gray = Layer::raster(GRAY, doc.pixel_format());
            let surface = crate::pixels_mut(&mut gray)?;
            let px = photocraft_raster::from_rgba(&surface.format(), [0.5, 0.5, 0.5, 1.0]);
            surface.fill_rect(bounds, &px);
            gray.blend = blend;
            children.push(gray);
        }
        if method != "gray" {
            let mut burn = curves_layer(BURN, mode, &[(0.0, 0.0), (0.5, 0.38), (1.0, 1.0)])?;
            burn.mask = Some(LayerMask::hide_all());
            let mut dodge = curves_layer(DODGE, mode, &[(0.0, 0.0), (0.5, 0.62), (1.0, 1.0)])?;
            dodge.mask = Some(LayerMask::hide_all());
            children.push(burn);
            children.push(dodge);
        }
        // Painting starts on the gray layer, or on the dodge curve when there is none.
        let paint_on =
            children.last().filter(|_| method == "curves").or(children.first()).map(|l| l.id).ok_or_else(|| EngineError::Other("nothing to build".into()))?;
        let layers: Vec<u64> = children.iter().map(|l| l.id.0).collect();
        if helper {
            let bw = Layer::new(HELPER_BW, LayerContent::Adjustment(crate::adjust_params::default_for("blackWhite", mode)?));
            let contrast = curves_layer(HELPER_CURVE, mode, &[(0.0, 0.0), (0.25, 0.1), (0.5, 0.5), (0.75, 0.9), (1.0, 1.0)])?;
            children.push(Layer::group(HELPER, vec![bw, contrast]));
        }
        let group = doc.insert_above(top_level(doc, *active), Layer::group(GROUP_DB, children));
        *active = Some(paint_on);
        Ok((group, paint_on, layers))
    })?;
    select_only(s, paint_on);
    Ok(json!({ "group": group.0, "paintOn": paint_on.0, "layers": layers }))
}

fn number(p: &Value, cmd: &str, key: &str, min: f64, max: f64, default: f64) -> Result<f64> {
    let v = match p.get(key) {
        None | Some(Value::Null) => return Ok(default),
        Some(v) => v.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(cmd, format!("`{key}` must be a number")))?,
    };
    if !(min..=max).contains(&v) {
        return Err(bad(cmd, format!("`{key}` must be from {min} to {max}")));
    }
    Ok(v)
}

/// 0 to 1 with linear ramps `soft` wide inside `lo..hi`.
fn band(v: f32, lo: f32, hi: f32, soft: f32) -> f32 {
    ((v - lo) / soft).min((hi - v) / soft).clamp(0.0, 1.0)
}

/// How much an encoded RGB colour looks like skin: the chroma box commonly used for skin
/// detection in YCbCr (Cb 77–127, Cr 133–173 on the 8-bit scale), softened at its edges, and
/// faded out in deep shadow where chroma means little.
pub fn skin_likelihood(r: f32, g: f32, b: f32) -> f32 {
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    let cb = 128.0 + 255.0 * (-0.168_736 * r - 0.331_264 * g + 0.5 * b);
    let cr = 128.0 + 255.0 * (0.5 * r - 0.418_688 * g - 0.081_312 * b);
    band(cb, 72.0, 132.0, 10.0) * band(cr, 128.0, 178.0, 10.0) * ((y - 0.06) / 0.12).clamp(0.0, 1.0)
}

/// A soft skin map of `image` (RGB with alpha) for a layer mask, feathered by `feather` px.
fn skin_mask(image: &Surface, bounds: Rect, feather: f32) -> Result<Surface> {
    let n = image.format().channels();
    let mut mask = Surface::with_default(PixelFormat::GRAY8, &[0.0]);
    let mut y = bounds.y0;
    while y < bounds.y1 {
        let band_rect = Rect::new(bounds.x0, y, bounds.x1, (y + BAND_ROWS).min(bounds.y1));
        let px = image.try_read_region(band_rect).map_err(alloc)?;
        let out: Vec<f32> = px
            .chunks_exact(n)
            .map(|p| match p {
                [r, g, b, ..] => skin_likelihood(*r, *g, *b),
                _ => 0.0,
            })
            .collect();
        mask.try_write_region(band_rect, &out).map_err(alloc)?;
        y = band_rect.y1;
    }
    let mut mask = photocraft_algo::apply_in(&mask, &FilterParams::GaussianBlur { radius: feather.max(0.5) }, bounds, bounds, None, bounds);
    mask.prune();
    Ok(mask)
}

/// The smoothed copy of `image` (see the module docs).
fn smooth_skin(image: &Surface, bounds: Rect, radius: f32, threshold: f32, strength: f32, detail: f32) -> Result<Surface> {
    let edges = photocraft_algo::apply_in(image, &FilterParams::SurfaceBlur { radius, threshold }, bounds, bounds, None, bounds);
    let fine = photocraft_algo::apply_in(image, &FilterParams::GaussianBlur { radius: (radius / 5.0).max(0.5) }, bounds, bounds, None, bounds);
    let fmt = image.format();
    let n = fmt.channels();
    let colors = n.saturating_sub(usize::from(fmt.alpha));
    let mut out = Surface::new(fmt);
    let mut y = bounds.y0;
    while y < bounds.y1 {
        let band_rect = Rect::new(bounds.x0, y, bounds.x1, (y + BAND_ROWS).min(bounds.y1));
        let orig = image.try_read_region(band_rect).map_err(alloc)?;
        let s = edges.try_read_region(band_rect).map_err(alloc)?;
        let mut f = fine.try_read_region(band_rect).map_err(alloc)?;
        for ((o, sp), fp) in orig.chunks_exact(n).zip(s.chunks_exact(n)).zip(f.chunks_exact_mut(n)) {
            for c in 0..n {
                let (Some(&ov), Some(&sv), Some(fv)) = (o.get(c), sp.get(c), fp.get_mut(c)) else { continue };
                *fv = if c < colors { (ov + strength * (sv - ov) + strength * detail * (ov - *fv)).clamp(0.0, 1.0) } else { ov };
            }
        }
        out.try_write_region(band_rect, &f).map_err(alloc)?;
        y = band_rect.y1;
    }
    out.prune();
    Ok(out)
}

fn skin_smooth(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = SKIN_SMOOTH;
    object(p, cmd)?;
    supported(s).map_err(EngineError::Other)?;
    let strength = number(p, cmd, "strength", 0.0, 100.0, 60.0)? as f32 / 100.0;
    let detail = number(p, cmd, "detail", 0.0, 100.0, 50.0)? as f32 / 100.0;
    let threshold = number(p, cmd, "threshold", 2.0, 255.0, 24.0)? as f32;
    let mask_kind = choice(p, cmd, "mask", &["skin", "hideAll", "revealAll"], "skin")?;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let radius = match number(p, cmd, "radius", 0.0, 100.0, 0.0)? {
        r if r < 1.0 => auto_radius(&st.doc).min(100.0),
        r => r as f32,
    };
    let id = s.edit("Skin Smoothing", |doc, active| {
        let bounds = doc.bounds();
        if bounds.is_empty() {
            return Err(EngineError::Other("the document has no pixels".into()));
        }
        let fmt = with_alpha(doc);
        let image = photocraft_compose::flatten_to_surface(doc, fmt, None);
        let mut layer = Layer::raster(SMOOTH, fmt);
        *crate::pixels_mut(&mut layer)? = smooth_skin(&image, bounds, radius, threshold, strength, detail)?;
        layer.mask = Some(match mask_kind {
            "hideAll" => LayerMask::hide_all(),
            "revealAll" => LayerMask::reveal_all(),
            // Grayscale has no chroma to find skin by: the whole image is smoothed.
            _ if fmt.mode != ColorMode::Rgb => LayerMask::reveal_all(),
            _ => LayerMask { surface: skin_mask(&image, bounds, radius * 1.5)?, ..LayerMask::hide_all() },
        });
        let id = doc.insert_above(*active, layer);
        *active = Some(id);
        Ok(id)
    })?;
    select_only(s, id);
    Ok(json!({ "layer": id.0, "radius": radius }))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: FREQUENCY_SEPARATION,
            label: "Frequency Separation…",
            menu: &["Filter", "Portrait Retouch"],
            shortcut: None,
            params: r##"{"radius":0..250=0,"retouchLayer":bool=false} (radius 0 = automatic) → {group, low, high, radius}"##,
            enabled: supported,
            run: frequency_separation,
            journal: true,
        },
        CommandSpec {
            id: DODGE_BURN,
            label: "Dodge & Burn Layers…",
            menu: &["Filter", "Portrait Retouch"],
            shortcut: None,
            params: r##"{"method":"gray|curves|both"="gray","blend":"softLight|overlay"="softLight","helper":bool=true} → {group, paintOn, layers}"##,
            enabled: supported,
            run: dodge_burn,
            journal: true,
        },
        CommandSpec {
            id: SKIN_SMOOTH,
            label: "Skin Smoothing…",
            menu: &["Filter", "Portrait Retouch"],
            shortcut: None,
            params: r##"{"strength":0..100=60,"radius":0..100=0,"detail":0..100=50,"threshold":2..255=24,"mask":"skin|hideAll|revealAll"="skin"} (radius 0 = automatic) → {layer, radius}"##,
            enabled: supported,
            run: skin_smooth,
            journal: true,
        },
        CommandSpec {
            id: DETAIL_VIEW,
            label: "Skin Detail View",
            menu: &["Filter", "Portrait Retouch"],
            shortcut: None,
            params: r##"{"mode":"contrast|solarize"="contrast"} (run again to remove the view) → {shown, group?}"##,
            enabled: supported,
            run: detail_view,
            journal: true,
        },
        CommandSpec {
            id: DETAIL_VIEW_SOLARIZE,
            label: "Skin Detail View (Solarize)",
            menu: &["Filter", "Portrait Retouch"],
            shortcut: None,
            params: r##"{} (run again to remove the view) → {shown, group?}"##,
            enabled: supported,
            run: |s, p| {
                object(p, DETAIL_VIEW_SOLARIZE)?;
                detail_view(s, &json!({"mode": "solarize"}))
            },
            journal: true,
        },
    ]
}

/// The top-level skin detail view group, if there is one.
fn detail_view_group(doc: &Document) -> Option<LayerId> {
    doc.layers.iter().find(|l| l.is_group() && l.xmp.as_deref().is_some_and(|x| x.contains(VIEW_NS))).map(|l| l.id)
}

/// `portrait.skinDetailView`: a viewing aid on top of everything that shows tone without colour,
/// steepened so blotches, uneven transitions and texture stand out (`contrast`), or folded back
/// on itself so every small change of tone becomes a visible band (`solarize`). Running it again
/// takes it away. It is a group of adjustment layers: hide or delete it before exporting.
fn detail_view(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = DETAIL_VIEW;
    object(p, cmd)?;
    supported(s).map_err(EngineError::Other)?;
    let mode = choice(p, cmd, "mode", &["contrast", "solarize"], "contrast")?;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    if let Some(id) = detail_view_group(&st.doc) {
        s.edit("Skin Detail View", |doc, active| {
            doc.remove(id).ok_or(EngineError::NoLayer(id))?;
            if *active == Some(id) {
                *active = doc.layers.last().map(|l| l.id);
            }
            Ok(())
        })?;
        return Ok(json!({ "shown": false }));
    }
    let id = s.edit("Skin Detail View", |doc, _| {
        let color = doc.pixel_format().mode;
        let points: &[(f32, f32)] = match mode {
            "solarize" => &[(0.0, 0.0), (0.25, 1.0), (0.5, 0.0), (0.75, 1.0), (1.0, 0.0)],
            _ => &[(0.0, 0.0), (0.35, 0.08), (0.5, 0.5), (0.65, 0.92), (1.0, 1.0)],
        };
        let bw = Layer::new(HELPER_BW, LayerContent::Adjustment(crate::adjust_params::default_for("blackWhite", color)?));
        let curve = curves_layer(HELPER_CURVE, color, points)?;
        let mut group = Layer::group(VIEW_GROUP, vec![bw, curve]);
        group.xmp = Some(format!("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description rdf:about=\"\" xmlns:photocraftView=\"{VIEW_NS}\" photocraftView:mode=\"{mode}\"/></rdf:RDF></x:xmpmeta>").into());
        let top = doc.layers.last().map(|l| l.id);
        Ok(doc.insert_above(top, group))
    })?;
    Ok(json!({ "shown": true, "group": id.0 }))
}

#[cfg(test)]
mod tests;
