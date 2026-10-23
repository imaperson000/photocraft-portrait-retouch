//! Filter › Portrait Retouch › Glow: bloom or light streaks from the image's highlights on a
//! layer of their own (Screen mode), editable afterwards.
//!
//! The glow layer keeps its settings in its own XMP packet (namespace [`NS`]), which PSD, layered
//! TIFF and `.pcraft` save. Running Glow with a glow layer active renders that layer again from
//! the image beneath it with the new settings, keeping its mask, opacity and mode, so the look can
//! be changed at any time. The layer's mask (on by default) is painted with the Brush and Eraser
//! as any layer mask; Invert / Clear Glow Mask flip or reset it in one step.
//!
//! **Local glow** (Local Glow: Start Painting / Apply): Start clears the glow and shows the
//! layer's mask as a red overlay; strokes paint red (black in the mask) where the light should
//! come from, the Eraser takes it back. Apply renders the glow from the painted lights only and
//! hides the overlay. In this mode (`maskUse: paint`) the mask only chooses the lights: it is off
//! for compositing, so their glow spreads past the strokes.
//!
//! The glow is computed at no more than [`MAX_WORK_SIDE`] pixels on the long side (a glow has no
//! fine detail to lose; its sizes are relative to the image) and scaled up bilinearly.

use photocraft_algo::glow::{GlowMode, GlowParams, Planes};
use photocraft_color::{ColorMode, PixelFormat};
use photocraft_doc::{BlendMode, Document, Layer, LayerId, LayerMask};
use photocraft_geom::Rect;
use photocraft_raster::Surface;
use serde_json::{Map, Value, json};

use crate::commands::CommandSpec;
use crate::{EngineError, Result, Session};

pub const GLOW: &str = "portrait.glow";
pub const MASK_INVERT: &str = "portrait.glowMaskInvert";
pub const MASK_CLEAR: &str = "portrait.glowMaskClear";
pub const PAINT_START: &str = "portrait.glowPaintStart";
pub const PAINT_APPLY: &str = "portrait.glowPaintApply";

/// XMP namespace of the glow settings.
pub const NS: &str = "https://photocraft.app/ns/glow/1.0/";

const LAYER_NAME: &str = "光暈";

/// Long side of the image the glow is computed on.
pub const MAX_WORK_SIDE: usize = 2048;

/// Rows of the document converted at a time.
const BAND_ROWS: usize = 256;

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: GLOW.into(), msg: msg.into() }
}

/// A Glow call's settings, checked.
struct Settings {
    params: GlowParams,
    /// Give a new glow layer a mask.
    mask: bool,
    /// Local glow: lights glow only where the mask is painted (black), instead of the mask
    /// hiding parts of the glow.
    paint: bool,
    /// The canonical JSON stored on the layer.
    stored: Value,
}

/// The settings `p` asks for.
fn parse(p: &Value) -> Result<Settings> {
    match p {
        Value::Object(_) | Value::Null => {}
        _ => return Err(bad("expected an object")),
    }
    let num = |key: &str, min: f64, max: f64, default: f64| -> Result<f32> {
        let v = match p.get(key) {
            None | Some(Value::Null) => return Ok(default as f32),
            Some(v) => v.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(format!("`{key}` must be a number")))?,
        };
        if !(min..=max).contains(&v) {
            return Err(bad(format!("`{key}` must be from {min} to {max}")));
        }
        Ok(v as f32)
    };
    let flag = |key: &str, default: bool| -> Result<bool> {
        match p.get(key) {
            None | Some(Value::Null) => Ok(default),
            Some(v) => v.as_bool().ok_or_else(|| bad(format!("`{key}` must be true or false"))),
        }
    };
    let mode = match p.get("mode").and_then(Value::as_str) {
        None if p.get("mode").is_none_or(Value::is_null) => GlowMode::Bloom,
        Some("bloom") => GlowMode::Bloom,
        Some("streaks") => GlowMode::Streaks,
        _ => return Err(bad("`mode` must be bloom or streaks")),
    };
    let rays = match p.get("rays") {
        None | Some(Value::Null) => 2,
        Some(v) => match v.as_str().and_then(|s| s.parse::<u32>().ok()).or_else(|| v.as_u64().and_then(|n| u32::try_from(n).ok())) {
            Some(n @ (2 | 4 | 6 | 8)) => n,
            _ => return Err(bad("`rays` must be 2, 4, 6 or 8")),
        },
    };
    let g = GlowParams {
        mode,
        rays,
        intensity: num("intensity", 0.0, 100.0, 100.0)?,
        radius: num("glowRadius", 0.0, 100.0, 20.0)?,
        threshold: num("threshold", 0.01, 1.0, 0.25)?,
        range: flag("range", false)?,
        exposure: num("exposure", 0.0, 255.0, 0.0)?,
        dispersion: num("dispersion", 0.0, 100.0, 0.0)?,
        rotation: num("rotation", -90.0, 90.0, 0.0)?,
        detail: num("detail", 0.0, 100.0, 0.0)?,
        colorize: flag("colorize", false)?,
        hue: num("hue", 0.0, 360.0, 0.0)?,
        saturation: num("saturation", 0.0, 100.0, 80.0)?,
        lightness: num("lightness", -100.0, 0.0, 0.0)?,
    };
    let mask = flag("mask", true)?;
    let paint = match p.get("maskUse") {
        None | Some(Value::Null) => false,
        Some(v) => match v.as_str() {
            Some("output") => false,
            Some("paint") => true,
            _ => return Err(bad("`maskUse` must be output or paint")),
        },
    };
    let stored = json!({
        "maskUse": if paint { "paint" } else { "output" },
        "mode": if g.mode == GlowMode::Streaks { "streaks" } else { "bloom" },
        "rays": g.rays.to_string(),
        "intensity": g.intensity, "glowRadius": g.radius, "threshold": g.threshold, "range": g.range,
        "exposure": g.exposure, "dispersion": g.dispersion, "rotation": g.rotation, "detail": g.detail,
        "colorize": g.colorize, "hue": g.hue, "saturation": g.saturation, "lightness": g.lightness,
    });
    Ok(Settings { params: g, mask: mask || paint, paint, stored })
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The XMP packet holding `settings`.
fn packet(settings: &Value) -> String {
    format!(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
         <rdf:Description rdf:about=\"\" xmlns:photocraftGlow=\"{NS}\"><photocraftGlow:params>{}</photocraftGlow:params>\
         </rdf:Description></rdf:RDF></x:xmpmeta>",
        escape(&settings.to_string())
    )
}

/// The glow settings stored in a layer's XMP, if it is a glow layer.
pub fn settings_of(layer: &Layer) -> Option<Map<String, Value>> {
    let xmp = layer.xmp.as_deref()?;
    if !xmp.contains(NS) {
        return None;
    }
    let doc = roxmltree::Document::parse(xmp).ok()?;
    let node = doc.descendants().find(|n| n.tag_name().namespace() == Some(NS) && n.tag_name().name() == "params")?;
    match serde_json::from_str(node.text()?).ok()? {
        Value::Object(m) => Some(m),
        _ => None,
    }
}

/// The active layer's glow settings, for the dialog to open with when re-editing.
pub fn active_settings(s: &Session) -> Option<Map<String, Value>> {
    let st = s.active()?;
    settings_of(st.doc.layer(st.active_layer?)?)
}

fn active_glow_layer(s: &Session) -> Option<LayerId> {
    let st = s.active()?;
    let id = st.active_layer?;
    settings_of(st.doc.layer(id)?).map(|_| id)
}

fn enabled(s: &Session) -> std::result::Result<(), String> {
    let st = s.active().ok_or("no document")?;
    match st.doc.pixel_format().mode {
        ColorMode::Rgb | ColorMode::Grayscale => Ok(()),
        m => Err(format!("Glow needs an RGB or Grayscale document (this one is {m:?})")),
    }
}

fn mask_enabled(s: &Session) -> std::result::Result<(), String> {
    let st = s.active().ok_or("no document")?;
    let id = active_glow_layer(s).ok_or("the active layer is not a glow layer")?;
    st.doc.layer(id).and_then(|l| l.mask.as_ref()).map(|_| ()).ok_or_else(|| "the glow layer has no mask".into())
}

fn alloc(e: photocraft_raster::AllocationError) -> EngineError {
    EngineError::Other(format!("not enough memory for the glow: {e}"))
}

/// `image` over `bounds`, box-averaged down by `factor`, as planar RGB.
fn downsample(image: &Surface, bounds: Rect, factor: usize) -> Result<Planes> {
    let (w, h) = (bounds.width() as usize, bounds.height() as usize);
    let (pw, ph) = (w.div_ceil(factor), h.div_ceil(factor));
    let mut out = Planes::new(pw, ph);
    let mut count = vec![0u32; pw.saturating_mul(ph)];
    let band = BAND_ROWS.div_ceil(factor) * factor;
    let mut row = vec![[0.0f32; 4]; 0];
    let mut y0 = 0;
    while y0 < h {
        let y1 = (y0 + band).min(h);
        let r = Rect::new(bounds.x0, bounds.y0 + y0 as i32, bounds.x1, bounds.y0 + y1 as i32);
        row.clear();
        row.try_reserve_exact(w * (y1 - y0)).map_err(|_| EngineError::Other("not enough memory for the glow".into()))?;
        row.resize(w * (y1 - y0), [0.0; 4]);
        image.read_rgba_into(r, &mut row);
        for (i, px) in row.iter().enumerate() {
            let (x, y) = (i % w, y0 + i / w);
            let at = (y / factor) * pw + x / factor;
            // Straight alpha: transparent pixels give no light.
            for (c, plane) in out.rgb.iter_mut().enumerate() {
                if let (Some(d), Some(v)) = (plane.get_mut(at), px.get(c)) {
                    *d += v * px[3];
                }
            }
            if let Some(n) = count.get_mut(at) {
                *n += 1;
            }
        }
        y0 = y1;
    }
    for plane in &mut out.rgb {
        for (v, n) in plane.iter_mut().zip(&count) {
            *v /= (*n).max(1) as f32;
        }
    }
    Ok(out)
}

/// `glow` scaled up by `factor` (bilinear, pixel centres aligned) into an opaque surface.
fn upsample(glow: &Planes, bounds: Rect, factor: usize, fmt: PixelFormat) -> Result<Surface> {
    let (w, h) = (bounds.width() as usize, bounds.height() as usize);
    let n = fmt.channels();
    let mut out = Surface::new(fmt);
    let at = |c: usize, x: f32, y: f32| -> f32 {
        let fx = x.clamp(0.0, (glow.width.max(1) - 1) as f32);
        let fy = y.clamp(0.0, (glow.height.max(1) - 1) as f32);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(glow.width.saturating_sub(1)), (y0 + 1).min(glow.height.saturating_sub(1)));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let Some(p) = glow.rgb.get(c) else { return 0.0 };
        let v = |x: usize, y: usize| p.get(y * glow.width + x).copied().unwrap_or(0.0);
        (v(x0, y0) * (1.0 - tx) + v(x1, y0) * tx) * (1.0 - ty) + (v(x0, y1) * (1.0 - tx) + v(x1, y1) * tx) * ty
    };
    let f = factor as f32;
    let mut y0 = 0;
    let mut buf = Vec::new();
    while y0 < h {
        let y1 = (y0 + BAND_ROWS).min(h);
        buf.clear();
        buf.try_reserve_exact(w * (y1 - y0) * n).map_err(|_| EngineError::Other("not enough memory for the glow".into()))?;
        buf.resize(w * (y1 - y0) * n, 0.0);
        for (i, px) in buf.chunks_exact_mut(n).enumerate() {
            let (x, y) = (i % w, y0 + i / w);
            let (sx, sy) = ((x as f32 + 0.5) / f - 0.5, (y as f32 + 0.5) / f - 0.5);
            let rgba = [at(0, sx, sy), at(1, sx, sy), at(2, sx, sy), 1.0];
            photocraft_raster::from_rgba_into(&fmt, rgba, px);
        }
        let r = Rect::new(bounds.x0, bounds.y0 + y0 as i32, bounds.x1, bounds.y0 + y1 as i32);
        out.try_write_region(r, &buf).map_err(alloc)?;
        y0 = y1;
    }
    Ok(out)
}

/// How much light `mask` lets through at the centre of every `factor`-pixel block of `bounds`:
/// where it is painted (black, red in the overlay), whether or not it is on for compositing.
fn emission(mask: &LayerMask, bounds: Rect, factor: usize, (pw, ph): (usize, usize)) -> Vec<f32> {
    let half = (factor / 2) as i32;
    let f = factor as i32;
    (0..ph)
        .flat_map(|j| (0..pw).map(move |i| (i as i32, j as i32)))
        .map(|(i, j)| {
            let x = (bounds.x0 + i * f + half).min(bounds.x1 - 1);
            let y = (bounds.y0 + j * f + half).min(bounds.y1 - 1);
            mask.density * (1.0 - mask.surface.sample_channel(x, y, 0))
        })
        .collect()
}

/// The glow of `doc`'s visible image, as an opaque surface in `fmt`; only the lights `emit`
/// allows glow when it is given.
fn render(doc: &Document, params: &GlowParams, fmt: PixelFormat, emit: Option<&LayerMask>) -> Result<Surface> {
    let bounds = doc.bounds();
    if bounds.is_empty() {
        return Err(EngineError::Other("the document has no pixels".into()));
    }
    let long = bounds.width().max(bounds.height()) as usize;
    let factor = long.div_ceil(MAX_WORK_SIDE).max(1);
    let image = photocraft_compose::flatten_to_surface(doc, PixelFormat::new(doc.pixel_format().mode, doc.pixel_format().sample, true), None);
    let small = downsample(&image, bounds, factor)?;
    let emit = emit.map(|m| emission(m, bounds, factor, (small.width, small.height)));
    let glow = photocraft_algo::glow::glow_from(&small, params, emit.as_deref());
    upsample(&glow, bounds, factor, fmt)
}

fn select_only(s: &mut Session, id: LayerId) {
    if let Some(st) = s.active_mut() {
        st.selected_layers = vec![id];
        st.active_layer = Some(id);
        st.layer_anchor = Some(id);
        crate::fix_selection(st);
    }
}

fn glow(s: &mut Session, p: &Value) -> Result<Value> {
    enabled(s).map_err(EngineError::Other)?;
    let Settings { params, mask, paint, stored } = parse(p)?;
    let xmp: std::sync::Arc<str> = packet(&stored).into();
    let existing = active_glow_layer(s);
    let label = if existing.is_some() { "Edit Glow" } else { "Glow" };
    let id = s.edit(label, |doc, active| {
        let fmt = doc.pixel_format();
        if let Some(id) = existing {
            // Render from the image without this glow, then put the new pixels in its place.
            let mut image = doc.clone();
            let layer = image.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
            layer.visible = false;
            if paint && layer.mask.is_none() {
                layer.mask = Some(LayerMask::reveal_all());
            }
            let emit = layer.mask.clone().filter(|_| paint);
            let pixels = render(&image, &params, fmt, emit.as_ref())?;
            let layer = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
            *crate::pixels_mut(layer)? = pixels;
            layer.xmp = Some(xmp.clone());
            if paint && layer.mask.is_none() {
                layer.mask = Some(LayerMask::reveal_all());
            }
            // Choosing the lights, the mask no longer hides the glow, which spreads past it.
            if let Some(m) = layer.mask.as_mut() {
                m.enabled = !paint;
            }
            return Ok(id);
        }
        let mut layer = Layer::raster(LAYER_NAME, fmt);
        let emit = paint.then(LayerMask::reveal_all);
        *crate::pixels_mut(&mut layer)? = render(doc, &params, fmt, emit.as_ref())?;
        layer.blend = BlendMode::Screen;
        layer.xmp = Some(xmp.clone());
        if mask {
            layer.mask = Some(LayerMask { enabled: !paint, ..LayerMask::reveal_all() });
        }
        let id = doc.insert_above(*active, layer);
        *active = Some(id);
        Ok(id)
    })?;
    select_only(s, id);
    Ok(json!({ "layer": id.0, "edited": existing.is_some(), "settings": stored }))
}

/// `portrait.glowPaintStart`: clear the glow (a glow layer is made with the default settings
/// when the active layer is not one), show its mask as a red overlay and set up the Brush to
/// paint the lights (black in the mask, red on screen) and the Eraser to take them back.
fn paint_start(s: &mut Session, p: &Value) -> Result<Value> {
    if !(p.is_object() || p.is_null()) {
        return Err(EngineError::BadParams { cmd: PAINT_START.into(), msg: "expected an object".into() });
    }
    enabled(s).map_err(EngineError::Other)?;
    let existing = active_glow_layer(s);
    let mut settings = existing.and_then(|_| active_settings(s)).unwrap_or_default();
    settings.insert("maskUse".into(), json!("paint"));
    let Settings { stored, .. } = parse(&Value::Object(settings))?;
    let xmp: std::sync::Arc<str> = packet(&stored).into();
    let id = s.edit("Local Glow", |doc, active| {
        let fmt = doc.pixel_format();
        let black = Surface::with_default(fmt, &photocraft_raster::from_rgba(&fmt, [0.0, 0.0, 0.0, 1.0]));
        let mask = LayerMask { enabled: false, ..LayerMask::reveal_all() };
        if let Some(id) = existing {
            let layer = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
            *crate::pixels_mut(layer)? = black;
            layer.mask = Some(mask);
            layer.xmp = Some(xmp.clone());
            return Ok(id);
        }
        let mut layer = Layer::raster(LAYER_NAME, fmt);
        *crate::pixels_mut(&mut layer)? = black;
        layer.blend = BlendMode::Screen;
        layer.mask = Some(mask);
        layer.xmp = Some(xmp.clone());
        let id = doc.insert_above(*active, layer);
        *active = Some(id);
        Ok(id)
    })?;
    select_only(s, id);
    crate::mask_view_cmds::set_view(s, &json!({"layer": id.0, "mode": "overlay"}))?;
    s.tools.foreground = [0.0, 0.0, 0.0, 1.0];
    s.tools.background = [1.0, 1.0, 1.0, 1.0];
    Ok(json!({ "layer": id.0, "tool": "brush" }))
}

/// `portrait.glowPaintApply`: render the active glow layer from the lights painted in its mask and
/// hide the overlay.
fn paint_apply(s: &mut Session, p: &Value) -> Result<Value> {
    if !(p.is_object() || p.is_null()) {
        return Err(EngineError::BadParams { cmd: PAINT_APPLY.into(), msg: "expected an object".into() });
    }
    let mut settings = active_settings(s).ok_or_else(|| EngineError::Other("the active layer is not a glow layer".into()))?;
    settings.insert("maskUse".into(), json!("paint"));
    let r = glow(s, &Value::Object(settings))?;
    crate::mask_view_cmds::set_view(s, &json!({"mode": "off"}))?;
    Ok(r)
}

fn change_mask(s: &mut Session, label: &str, f: impl FnOnce(&mut LayerMask)) -> Result<Value> {
    mask_enabled(s).map_err(EngineError::Other)?;
    let id = active_glow_layer(s).ok_or_else(|| EngineError::Other("the active layer is not a glow layer".into()))?;
    s.edit(label, |doc, _| {
        let mask = doc.layer_mut(id).and_then(|l| l.mask.as_mut()).ok_or_else(|| EngineError::Other("the glow layer has no mask".into()))?;
        f(mask);
        Ok(())
    })?;
    Ok(json!({ "layer": id.0 }))
}

fn mask_invert(s: &mut Session, p: &Value) -> Result<Value> {
    if !(p.is_object() || p.is_null()) {
        return Err(EngineError::BadParams { cmd: MASK_INVERT.into(), msg: "expected an object".into() });
    }
    change_mask(s, "Invert Glow Mask", |m| m.surface.map_color(|v| 1.0 - v))
}

fn mask_clear(s: &mut Session, p: &Value) -> Result<Value> {
    if !(p.is_object() || p.is_null()) {
        return Err(EngineError::BadParams { cmd: MASK_CLEAR.into(), msg: "expected an object".into() });
    }
    change_mask(s, "Clear Glow Mask", |m| m.surface = LayerMask::reveal_all().surface)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: GLOW,
            label: "Glow…",
            menu: &["Filter", "Portrait Retouch"],
            shortcut: None,
            params: r##"{"mode":"bloom|streaks"="bloom","rays":"2|4|6|8"="2","intensity":0..100=100,"glowRadius":0..100=20,"threshold":0.01..1=0.25,"range":bool=false,"exposure":0..255=0,"dispersion":0..100=0,"rotation":-90..90=0,"detail":0..100=0,"colorize":bool=false,"hue":0..360=0,"saturation":0..100=80,"lightness":-100..0=0,"mask":bool=true,"maskUse":"output|paint"="output"} (maskUse paint: only the lights painted in the mask glow, see Local Glow; with a glow layer active: re-render it with these settings) → {layer, edited, settings}"##,
            enabled,
            run: glow,
            journal: true,
        },
        CommandSpec {
            id: MASK_INVERT,
            label: "Invert Glow Mask",
            menu: &["Filter", "Portrait Retouch"],
            shortcut: None,
            params: "{}",
            enabled: mask_enabled,
            run: mask_invert,
            journal: true,
        },
        CommandSpec {
            id: MASK_CLEAR,
            label: "Clear Glow Mask",
            menu: &["Filter", "Portrait Retouch"],
            shortcut: None,
            params: "{}",
            enabled: mask_enabled,
            run: mask_clear,
            journal: true,
        },
        CommandSpec {
            id: PAINT_START,
            label: "Local Glow: Start Painting",
            menu: &["Filter", "Portrait Retouch"],
            shortcut: None,
            params: "{} (clears the glow and shows its mask as a red overlay: paint the lights, then Local Glow: Apply) → {layer, tool}",
            enabled,
            run: paint_start,
            journal: true,
        },
        CommandSpec {
            id: PAINT_APPLY,
            label: "Local Glow: Apply",
            menu: &["Filter", "Portrait Retouch"],
            shortcut: None,
            params: "{} → {layer, edited, settings}",
            enabled: |s| active_glow_layer(s).map(|_| ()).ok_or_else(|| "the active layer is not a glow layer".into()),
            run: paint_apply,
            journal: true,
        },
    ]
}

#[cfg(test)]
mod tests;
