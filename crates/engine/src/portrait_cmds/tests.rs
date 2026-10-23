use photocraft_doc::{BlendMode, Layer, LayerContent, LayerId};
use photocraft_geom::Rect;
use serde_json::json;

use super::*;
use crate::Session;

/// A document whose Background has a different colour at every pixel, plus fine detail.
fn busy(mode: &str, depth: u32) -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 40, "height": 30, "mode": mode, "depth": depth})).unwrap();
    s.edit("busy", |doc, active| {
        let surf = doc.layer_mut(active.unwrap()).unwrap().surface_mut().unwrap();
        let fmt = surf.format();
        for y in 0..30 {
            for x in 0..40 {
                let speck = if (x * 13 + y * 7) % 5 == 0 { 0.3 } else { 0.0 };
                let rgba = [(x as f32 / 39.0 * 0.7 + speck).min(1.0), y as f32 / 29.0, ((x * 7 + y * 3) % 32) as f32 / 31.0, 1.0];
                surf.fill_rect(Rect::new(x, y, x + 1, y + 1), &photocraft_raster::from_rgba(&fmt, rgba));
            }
        }
        Ok(())
    })
    .unwrap();
    s
}

fn shown(s: &Session) -> photocraft_compose::Buffer {
    let doc = &s.active().unwrap().doc;
    photocraft_compose::render(doc, doc.bounds())
}

fn max_diff(a: &photocraft_compose::Buffer, b: &photocraft_compose::Buffer) -> f32 {
    a.px.iter().zip(&b.px).flat_map(|(p, q)| p.iter().zip(q).map(|(x, y)| (x - y).abs())).fold(0.0, f32::max)
}

fn top(s: &Session) -> &Layer {
    s.active().unwrap().doc.layers.last().unwrap()
}

fn names(l: &Layer) -> Vec<&str> {
    l.children().unwrap().iter().map(|c| c.name.as_str()).collect()
}

fn layer(s: &Session, id: u64) -> &Layer {
    s.active().unwrap().doc.layer(LayerId(id)).unwrap()
}

/// Low + high in Linear Light give the image back, at every depth: exactly at 16 and 32 bits,
/// within the one level the 8-bit high-frequency layer rounds to.
#[test]
fn frequency_separation_reproduces_the_image() {
    for (mode, depth) in [("rgb", 8), ("rgb", 16), ("rgb", 32), ("gray", 8), ("gray", 16)] {
        let mut s = busy(mode, depth);
        let before = shown(&s);
        let r = s.execute(FREQUENCY_SEPARATION, json!({"radius": 3})).unwrap();
        assert_eq!(r["radius"], 3.0);
        let low = layer(&s, r["low"].as_u64().unwrap());
        assert_eq!(low.blend, BlendMode::Normal);
        let high = layer(&s, r["high"].as_u64().unwrap());
        assert_eq!(high.blend, BlendMode::LinearLight);
        let d = max_diff(&shown(&s), &before);
        let tolerance = if depth == 8 { 2.0 / 255.0 + 1e-4 } else { 2e-3 };
        assert!(d <= tolerance, "{mode}/{depth}: off by {d}");
        // The low layer on its own is the blur: the detail is gone from it.
        let px = low.surface().unwrap().read_region(Rect::new(10, 10, 30, 20));
        let orig = s.active().unwrap().doc.layers[0].surface().unwrap().read_region(Rect::new(10, 10, 30, 20));
        assert!(px.iter().zip(&orig).any(|(a, b)| (a - b).abs() > 0.02), "{mode}/{depth}: low is blurred");
    }
}

#[test]
fn frequency_separation_builds_one_undoable_group() {
    let mut s = busy("rgb", 8);
    let steps = s.active().unwrap().history.past_len();
    let r = s.execute(FREQUENCY_SEPARATION, json!({})).unwrap();
    let st = s.active().unwrap();
    assert_eq!(st.history.past_len(), steps + 1);
    let g = top(&s);
    assert_eq!(g.id.0, r["group"].as_u64().unwrap());
    assert_eq!(g.name, "頻率分離");
    assert_eq!(g.blend, BlendMode::PassThrough);
    assert_eq!(names(g), ["低頻", "高頻"]);
    // Texture work starts on the high-frequency layer.
    assert_eq!(st.active_layer, Some(LayerId(r["high"].as_u64().unwrap())));
    // 0 = automatic: a 40 px document gets the smallest radius.
    assert_eq!(r["radius"], 2.0);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.active().unwrap().doc.layers.len(), 1);

    s.execute(FREQUENCY_SEPARATION, json!({"retouchLayer": true})).unwrap();
    assert_eq!(names(top(&s)), ["低頻", "低頻修飾", "高頻"]);
}

#[test]
fn auto_radius_follows_the_image_size() {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 6000, "height": 4000, "mode": "rgb", "depth": 8})).unwrap();
    assert_eq!(auto_radius(&s.active().unwrap().doc), 10.0);
    s.execute("file.new", json!({"width": 100_000, "height": 10, "mode": "rgb", "depth": 8})).unwrap();
    assert_eq!(auto_radius(&s.active().unwrap().doc), 30.0);
}

/// Without the viewing aid, a fresh dodge & burn kit leaves the image unchanged (50% gray is
/// neutral in Soft Light and Overlay, the curves are masked out).
#[test]
fn dodge_burn_starts_neutral() {
    for (mode, depth) in [("rgb", 8), ("rgb", 16), ("rgb", 32), ("gray", 8)] {
        for method in ["gray", "curves", "both"] {
            for blend in ["softLight", "overlay"] {
                let mut s = busy(mode, depth);
                let before = shown(&s);
                s.execute(DODGE_BURN, json!({"method": method, "blend": blend, "helper": false})).unwrap();
                let d = max_diff(&shown(&s), &before);
                let tolerance = if depth == 8 { 1.0 / 255.0 + 1e-4 } else { 1e-3 };
                assert!(d <= tolerance, "{mode}/{depth} {method} {blend}: changed by {d}");
            }
        }
    }
}

#[test]
fn dodge_burn_layers_by_method() {
    let mut s = busy("rgb", 16);
    let steps = s.active().unwrap().history.past_len();
    let r = s.execute(DODGE_BURN, json!({})).unwrap();
    assert_eq!(s.active().unwrap().history.past_len(), steps + 1);
    let g = top(&s);
    assert_eq!(g.name, "加深減淡");
    assert_eq!(names(g), ["中性灰", "觀察組"]);
    let gray = &g.children().unwrap()[0];
    assert_eq!(gray.blend, BlendMode::SoftLight);
    assert_eq!(r["paintOn"].as_u64(), Some(gray.id.0));
    assert_eq!(s.active().unwrap().active_layer, Some(gray.id));
    let helper = &g.children().unwrap()[1];
    assert_eq!(names(helper), ["黑白", "對比"]);
    assert!(matches!(helper.children().unwrap()[0].content, LayerContent::Adjustment(Adjustment::BlackWhite { .. })));
    // The viewing aid shows tone only.
    let buf = shown(&s);
    assert!(buf.px.iter().all(|p| (p[0] - p[1]).abs() < 1e-3 && (p[1] - p[2]).abs() < 1e-3));

    s.execute(DODGE_BURN, json!({"method": "both", "blend": "overlay", "helper": false})).unwrap();
    let g = top(&s);
    assert_eq!(names(g), ["中性灰", "加深", "加亮"]);
    assert_eq!(g.children().unwrap()[0].blend, BlendMode::Overlay);
    for c in &g.children().unwrap()[1..] {
        assert!(matches!(c.content, LayerContent::Adjustment(Adjustment::Curves { .. })), "{}", c.name);
        assert!(c.mask.is_some(), "{} is masked", c.name);
    }

    let r = s.execute(DODGE_BURN, json!({"method": "curves", "helper": false})).unwrap();
    let g = top(&s);
    assert_eq!(names(g), ["加深", "加亮"]);
    assert_eq!(r["paintOn"].as_u64(), Some(g.children().unwrap()[1].id.0));
}

/// Revealing the curves' masks brightens and darkens.
#[test]
fn dodge_and_burn_curves_work_when_revealed() {
    let mut s = busy("rgb", 8);
    let before = shown(&s);
    s.execute(DODGE_BURN, json!({"method": "curves", "helper": false})).unwrap();
    let ids: Vec<LayerId> = top(&s).children().unwrap().iter().map(|l| l.id).collect();
    let sum = |b: &photocraft_compose::Buffer| b.px.iter().map(|p| p[0] + p[1] + p[2]).sum::<f32>();
    for (id, brighter) in [(ids[0], false), (ids[1], true)] {
        let mut t = busy("rgb", 8);
        t.execute(DODGE_BURN, json!({"method": "curves", "helper": false})).unwrap();
        let tid = top(&t).children().unwrap().iter().find(|l| l.name == layer(&s, id.0).name).unwrap().id;
        t.edit("reveal", |doc, _| {
            doc.layer_mut(tid).unwrap().mask = Some(photocraft_doc::LayerMask::reveal_all());
            Ok(())
        })
        .unwrap();
        let after = sum(&shown(&t));
        assert_eq!(after > sum(&before), brighter, "{}", layer(&s, id.0).name);
    }
}

#[test]
fn bad_params_and_modes_are_errors() {
    let mut s = busy("rgb", 8);
    for (cmd, p) in [
        (FREQUENCY_SEPARATION, json!({"radius": -1})),
        (FREQUENCY_SEPARATION, json!({"radius": 251})),
        (FREQUENCY_SEPARATION, json!({"radius": "big"})),
        (FREQUENCY_SEPARATION, json!({"retouchLayer": 1})),
        (FREQUENCY_SEPARATION, json!(7)),
        (DODGE_BURN, json!({"method": "magic"})),
        (DODGE_BURN, json!({"blend": "screen"})),
        (DODGE_BURN, json!({"helper": "yes"})),
        (DODGE_BURN, json!([1])),
    ] {
        assert!(s.execute(cmd, p.clone()).is_err(), "{cmd} {p}");
    }
    assert_eq!(s.active().unwrap().doc.layers.len(), 1);

    let mut s = busy("cmyk", 8);
    assert!(s.execute(FREQUENCY_SEPARATION, json!({})).is_err());
    assert!(s.execute(DODGE_BURN, json!({})).is_err());
    assert!(supported(&s).is_err());
    assert!(supported(&Session::new()).is_err());
    assert!(Session::new().execute(FREQUENCY_SEPARATION, json!({})).is_err());
}

/// Rough variation between neighbours: what blotches and noise add.
fn roughness(b: &photocraft_compose::Buffer, w: usize) -> f32 {
    b.px.windows(2).enumerate().filter(|(i, _)| (i + 1) % w != 0).map(|(_, p)| (p[0][0] - p[1][0]).abs() + (p[0][1] - p[1][1]).abs()).sum()
}

#[test]
fn skin_smoothing_evens_out_and_is_one_step() {
    for depth in [8, 16, 32] {
        let mut s = busy("rgb", depth);
        let before = shown(&s);
        let steps = s.active().unwrap().history.past_len();
        let r = s.execute(SKIN_SMOOTH, json!({"strength": 100, "radius": 4, "mask": "revealAll", "detail": 0, "threshold": 120})).unwrap();
        assert_eq!(s.active().unwrap().history.past_len(), steps + 1);
        let l = layer(&s, r["layer"].as_u64().unwrap());
        assert_eq!(l.name, "磨皮");
        assert!(l.mask.is_some());
        assert_eq!(s.active().unwrap().active_layer, Some(l.id));
        let after = shown(&s);
        assert!(roughness(&after, 40) < roughness(&before, 40) * 0.8, "{depth}: smoother");
        // Strength 0 changes nothing.
        let mut t = busy("rgb", depth);
        t.execute(SKIN_SMOOTH, json!({"strength": 0, "mask": "revealAll"})).unwrap();
        let d = max_diff(&shown(&t), &before);
        assert!(d <= 1.0 / 255.0 + 1e-4, "{depth}: strength 0 changed {d}");
    }
}

#[test]
fn skin_mask_finds_skin_tones() {
    assert!(skin_likelihood(0.88, 0.67, 0.55) > 0.9, "light skin");
    assert!(skin_likelihood(0.55, 0.36, 0.26) > 0.9, "darker skin");
    assert!(skin_likelihood(0.2, 0.4, 0.9) < 0.01, "sky");
    assert!(skin_likelihood(0.2, 0.7, 0.2) < 0.01, "foliage");
    assert!(skin_likelihood(0.02, 0.01, 0.01) < 0.01, "shadow");

    // A skin patch on a blue background: the mask shows the patch and hides the background.
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 60, "height": 40, "mode": "rgb", "depth": 8})).unwrap();
    s.edit("paint", |doc, active| {
        let surf = doc.layer_mut(active.unwrap()).unwrap().surface_mut().unwrap();
        let fmt = surf.format();
        surf.fill_rect(Rect::new(0, 0, 60, 40), &photocraft_raster::from_rgba(&fmt, [0.2, 0.4, 0.9, 1.0]));
        surf.fill_rect(Rect::new(20, 10, 50, 35), &photocraft_raster::from_rgba(&fmt, [0.88, 0.67, 0.55, 1.0]));
        Ok(())
    })
    .unwrap();
    let r = s.execute(SKIN_SMOOTH, json!({"radius": 2})).unwrap();
    let mask = layer(&s, r["layer"].as_u64().unwrap()).mask.as_ref().unwrap();
    assert!(mask.value(35, 22) > 0.9, "patch shown");
    assert!(mask.value(5, 5) < 0.05, "background hidden");

    for (kind, want) in [("hideAll", 0.0), ("revealAll", 1.0)] {
        let r = s.execute(SKIN_SMOOTH, json!({"mask": kind})).unwrap();
        let mask = layer(&s, r["layer"].as_u64().unwrap()).mask.as_ref().unwrap();
        assert_eq!(mask.value(35, 22), want, "{kind}");
    }
}

#[test]
fn skin_smoothing_bad_params_are_errors() {
    let mut s = busy("rgb", 8);
    for p in [
        json!({"strength": 101}),
        json!({"strength": -1}),
        json!({"strength": "lots"}),
        json!({"radius": 101}),
        json!({"detail": 200}),
        json!({"threshold": 1}),
        json!({"mask": "face"}),
        json!("x"),
    ] {
        assert!(s.execute(SKIN_SMOOTH, p.clone()).is_err(), "{p}");
    }
    assert_eq!(s.active().unwrap().doc.layers.len(), 1);
    // Grayscale has no chroma: the whole image is smoothed.
    let mut g = busy("gray", 8);
    let r = g.execute(SKIN_SMOOTH, json!({})).unwrap();
    assert_eq!(layer(&g, r["layer"].as_u64().unwrap()).mask.as_ref().unwrap().value(3, 3), 1.0);
    assert!(busy("cmyk", 8).execute(SKIN_SMOOTH, json!({})).is_err());
}
