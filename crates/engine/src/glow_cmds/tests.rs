use photocraft_doc::{BlendMode, LayerId};
use photocraft_geom::Rect;
use serde_json::json;

use super::*;
use crate::Session;

/// A dark document with one bright light.
fn night(mode: &str, depth: u32, w: i32, h: i32) -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": w, "height": h, "mode": mode, "depth": depth})).unwrap();
    s.edit("paint", |doc, active| {
        let surf = doc.layer_mut(active.unwrap()).unwrap().surface_mut().unwrap();
        let fmt = surf.format();
        surf.fill_rect(Rect::new(0, 0, w, h), &photocraft_raster::from_rgba(&fmt, [0.05, 0.05, 0.08, 1.0]));
        let r = (w / 60).max(2);
        surf.fill_rect(Rect::new(w / 2 - r, h / 2 - r, w / 2 + r, h / 2 + r), &photocraft_raster::from_rgba(&fmt, [1.0, 0.95, 0.8, 1.0]));
        Ok(())
    })
    .unwrap();
    s
}

fn shown(s: &Session) -> photocraft_compose::Buffer {
    let doc = &s.active().unwrap().doc;
    photocraft_compose::render(doc, doc.bounds())
}

fn red_at(b: &photocraft_compose::Buffer, w: i32, x: i32, y: i32) -> f32 {
    b.px[(y * w + x) as usize][0]
}

#[test]
fn glow_adds_a_screen_layer_that_lights_around_the_highlight() {
    for (mode, depth) in [("rgb", 8), ("rgb", 16), ("rgb", 32), ("gray", 8)] {
        let mut s = night(mode, depth, 120, 80);
        let before = shown(&s);
        let steps = s.active().unwrap().history.past_len();
        let r = s.execute(GLOW, json!({})).unwrap();
        assert_eq!(s.active().unwrap().history.past_len(), steps + 1);
        let id = LayerId(r["layer"].as_u64().unwrap());
        let l = s.active().unwrap().doc.layer(id).unwrap();
        assert_eq!((l.name.as_str(), l.blend), ("光暈", BlendMode::Screen));
        assert!(l.mask.is_some());
        assert_eq!(s.active().unwrap().active_layer, Some(id));
        let after = shown(&s);
        // Near the light it is brighter; far away (the dark sky) it is unchanged.
        assert!(red_at(&after, 120, 66, 40) > red_at(&before, 120, 66, 40) + 0.02, "{mode}/{depth} glow");
        assert!((red_at(&after, 120, 2, 2) - red_at(&before, 120, 2, 2)).abs() < 0.01, "{mode}/{depth} far");
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(s.active().unwrap().doc.layers.len(), 1);
    }
}

#[test]
fn settings_are_stored_and_re_editing_renders_again() {
    let mut s = night("rgb", 8, 100, 60);
    let r = s.execute(GLOW, json!({"mode": "streaks", "rays": "4", "glowRadius": 30, "colorize": true, "hue": 200})).unwrap();
    let id = LayerId(r["layer"].as_u64().unwrap());
    let stored = active_settings(&s).unwrap();
    assert_eq!(stored["mode"], "streaks");
    assert_eq!(stored["rays"], "4");
    assert_eq!(stored["hue"], 200.0);
    assert_eq!(r["edited"], false);
    let layers = s.active().unwrap().doc.layers.len();
    // A new mask stroke and opacity survive re-editing; the glow does not feed on itself.
    s.edit("tweak", |doc, _| {
        doc.layer_mut(id).unwrap().opacity = 0.5;
        Ok(())
    })
    .unwrap();
    let first = s.active().unwrap().doc.layer(id).unwrap().surface().unwrap().read_region(Rect::new(0, 0, 100, 60));
    let r = s.execute(GLOW, json!({"mode": "streaks", "rays": "4", "glowRadius": 30, "colorize": true, "hue": 200})).unwrap();
    assert_eq!(r["edited"], true);
    assert_eq!(r["layer"].as_u64(), Some(id.0));
    let st = s.active().unwrap();
    assert_eq!(st.doc.layers.len(), layers);
    let l = st.doc.layer(id).unwrap();
    assert_eq!(l.opacity, 0.5);
    let again = l.surface().unwrap().read_region(Rect::new(0, 0, 100, 60));
    assert_eq!(first, again, "same settings, same glow");
    s.execute(GLOW, json!({"mode": "bloom", "intensity": 50})).unwrap();
    assert_eq!(active_settings(&s).unwrap()["mode"], "bloom");
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(active_settings(&s).unwrap()["mode"], "streaks");
    // Another layer active: a new glow layer.
    s.execute("layer.new.layer", json!({})).unwrap();
    assert!(active_settings(&s).is_none());
    assert_eq!(s.execute(GLOW, json!({"mask": false})).unwrap()["edited"], false);
    assert!(s.active().unwrap().doc.layers.last().unwrap().mask.is_none());
}

#[test]
fn settings_survive_a_pcraft_round_trip() {
    let mut s = night("rgb", 8, 40, 30);
    s.execute(GLOW, json!({"threshold": 0.5})).unwrap();
    let doc = &s.active().unwrap().doc;
    let bytes = photocraft_format::save_to_bytes(doc, &Default::default()).unwrap();
    let back = photocraft_format::load_from_bytes(&bytes).unwrap();
    let l = back.layers.last().unwrap();
    assert_eq!(settings_of(l).unwrap()["threshold"], 0.5);
}

#[test]
fn mask_commands_invert_and_clear() {
    let mut s = night("rgb", 8, 40, 30);
    assert!(s.execute(MASK_INVERT, json!({})).is_err(), "no glow layer yet");
    s.execute(GLOW, json!({})).unwrap();
    let mask_at = |s: &Session| s.active().unwrap().doc.layers.last().unwrap().mask.as_ref().unwrap().value(5, 5);
    assert_eq!(mask_at(&s), 1.0);
    s.execute(MASK_INVERT, json!({})).unwrap();
    assert_eq!(mask_at(&s), 0.0);
    s.execute(MASK_CLEAR, json!({})).unwrap();
    assert_eq!(mask_at(&s), 1.0);
    assert!(s.execute(MASK_CLEAR, json!(3)).is_err());
    s.execute(GLOW, json!({})).unwrap();
    s.execute("layer.new.layer", json!({})).unwrap();
    s.execute(GLOW, json!({"mask": false})).unwrap();
    assert!(mask_enabled(&s).is_err());
}

#[test]
fn bad_params_are_errors() {
    let mut s = night("rgb", 8, 40, 30);
    for p in [
        json!({"mode": "sparkle"}),
        json!({"mode": 1}),
        json!({"rays": "3"}),
        json!({"rays": 5}),
        json!({"intensity": 101}),
        json!({"glowRadius": -1}),
        json!({"threshold": 0}),
        json!({"exposure": 256}),
        json!({"rotation": 91}),
        json!({"hue": 361}),
        json!({"lightness": 1}),
        json!({"colorize": "yes"}),
        json!({"mask": 0}),
        json!([]),
    ] {
        assert!(s.execute(GLOW, p.clone()).is_err(), "{p}");
    }
    assert_eq!(s.active().unwrap().doc.layers.len(), 1);
    assert!(s.execute(GLOW, json!({"rays": 8, "mode": "streaks"})).is_ok());
    assert!(night("cmyk", 8, 10, 10).execute(GLOW, json!({})).is_err());
    assert!(Session::new().execute(GLOW, json!({})).is_err());
}

/// A document bigger than the working size: computed small, scaled up, still lit at the light.
#[test]
fn large_documents_are_computed_reduced() {
    let mut s = night("rgb", 8, 4200, 1200);
    s.execute(GLOW, json!({})).unwrap();
    let after = shown(&s);
    assert!(red_at(&after, 4200, 2100, 600) > 0.9);
    assert!(red_at(&after, 4200, 2300, 600) > 0.06, "{}", red_at(&after, 4200, 2300, 600));
}

fn two_lights(w: i32, h: i32) -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": w, "height": h, "mode": "rgb", "depth": 8})).unwrap();
    s.edit("paint", |doc, active| {
        let surf = doc.layer_mut(active.unwrap()).unwrap().surface_mut().unwrap();
        let fmt = surf.format();
        surf.fill_rect(Rect::new(0, 0, w, h), &photocraft_raster::from_rgba(&fmt, [0.0, 0.0, 0.0, 1.0]));
        for x0 in [30, 126] {
            surf.fill_rect(Rect::new(x0, 28, x0 + 4, 32), &photocraft_raster::from_rgba(&fmt, [1.0, 1.0, 1.0, 1.0]));
        }
        Ok(())
    })
    .unwrap();
    s
}

/// Two lights. Local glow: Start clears the glow and shows the mask; painting over the right
/// light and applying makes only that one glow, and its glow spreads past the stroke.
#[test]
fn local_glow_paints_the_lights_that_glow() {
    let (w, h) = (160, 60);
    let full = {
        let mut t = two_lights(w, h);
        t.execute(GLOW, json!({})).unwrap();
        shown(&t)
    };
    let mut s = two_lights(w, h);
    // No glow layer yet: Start makes one, with nothing glowing.
    let before = shown(&s);
    let r = s.execute(PAINT_START, json!({})).unwrap();
    assert_eq!(r["tool"], "brush");
    let id = LayerId(r["layer"].as_u64().unwrap());
    assert_eq!(active_settings(&s).unwrap()["maskUse"], "paint");
    assert!((red_at(&shown(&s), w, 136, 30) - red_at(&before, w, 136, 30)).abs() < 1e-6, "cleared");
    let st = s.active().unwrap();
    assert_eq!(crate::mask_view_cmds::view_json(st)["mode"], "overlay");
    assert!(!st.doc.layer(id).unwrap().mask.as_ref().unwrap().enabled);
    assert_eq!(s.tools.foreground, [0.0, 0.0, 0.0, 1.0]);
    // A stroke over the right light (black in the mask, red on screen).
    s.edit("stroke", |doc, _| {
        doc.layer_mut(id).unwrap().mask.as_mut().unwrap().surface.fill_rect(Rect::new(120, 20, 136, 40), &[0.0]);
        Ok(())
    })
    .unwrap();
    let steps = s.active().unwrap().history.past_len();
    s.execute(PAINT_APPLY, json!({})).unwrap();
    let st = s.active().unwrap();
    assert_eq!(st.history.past_len(), steps + 1);
    assert!(crate::mask_view_cmds::view_json(st).is_null(), "overlay off");
    let after = shown(&s);
    assert!(red_at(&after, w, 140, 30) > 0.0, "right light glows");
    assert!((red_at(&after, w, 140, 30) - red_at(&full, w, 140, 30)).abs() < 0.005, "as strongly as a full glow, past the stroke");
    assert!(red_at(&full, w, 40, 30) > 0.0 && red_at(&after, w, 40, 30) < 1e-6, "left light not painted: no glow");
    // Start again on the same layer: cleared again, settings kept.
    s.execute(GLOW, json!({"maskUse": "paint", "glowRadius": 35})).unwrap();
    s.execute(PAINT_START, json!({})).unwrap();
    assert_eq!(s.active().unwrap().doc.layers.len(), 2);
    assert_eq!(active_settings(&s).unwrap()["glowRadius"], 35.0);
    assert!((red_at(&shown(&s), w, 140, 30) - red_at(&before, w, 140, 30)).abs() < 1e-6);
}

#[test]
fn local_glow_needs_a_glow_layer_to_apply() {
    let mut s = night("rgb", 8, 40, 30);
    assert!(s.execute(PAINT_APPLY, json!({})).is_err());
    assert!(s.execute(PAINT_START, json!(5)).is_err());
    assert!(s.execute(GLOW, json!({"maskUse": "source"})).is_err());
    assert!(night("cmyk", 8, 10, 10).execute(PAINT_START, json!({})).is_err());
}

/// While the red overlay shows, a brush stroke paints the glow mask, not the glow pixels.
#[test]
fn strokes_during_local_glow_paint_the_mask() {
    let mut s = night("rgb", 8, 60, 40);
    let id = LayerId(s.execute(PAINT_START, json!({})).unwrap()["layer"].as_u64().unwrap());
    let pixels = s.active().unwrap().doc.layer(id).unwrap().surface().unwrap().read_region(Rect::new(0, 0, 60, 40));
    s.execute("paint.stroke", json!({"points": [[20, 20], [40, 20]], "size": 9})).unwrap();
    let l = s.active().unwrap().doc.layer(id).unwrap();
    assert!(l.mask.as_ref().unwrap().surface.sample_channel(30, 20, 0) < 0.5, "painted into the mask");
    assert_eq!(l.surface().unwrap().read_region(Rect::new(0, 0, 60, 40)), pixels, "glow pixels untouched");
}
