//! Copy / Paste Camera Raw Grade: take the Camera Raw settings an image was graded with and give
//! them to another image, as Lightroom's Copy / Paste Settings do.
//!
//! - Every Camera Raw commit (the open-time develop of a raw file, the Camera Raw Filter) records
//!   the dialog's whole settings for that document ([`record`]), the raw-stage ones included, so
//!   "this image's grade" is what was actually used, even after the dialog closed.
//! - **Copy** takes the open dialog's settings, or else the active document's recorded grade, and
//!   keeps them in the preferences (`dialogs["cameraRaw.grade"]`), so they survive a restart.
//! - **Paste** puts them into the Camera Raw dialog (opening it on the active layer when it is
//!   closed): the preview shows the result and OK applies it. Pasted while a raw file's open-time
//!   dialog is up, the raw-stage settings re-develop the file, exactly as on the source image.
//!
//! The recorded grades live with the open documents for this session; they are not written into
//! saved files.

use photocraft_algo::camera_raw::CameraRaw;
use photocraft_doc::DocId;
use serde_json::{Value, json};

use crate::PhotocraftApp;

pub const COPY: &str = "cameraRaw.copyGrade";
pub const PASTE: &str = "cameraRaw.pasteGrade";

/// Preferences key of the copied grade.
const PREF: &str = "cameraRaw.grade";

/// Remember `params` as the grade of document `doc`.
pub fn record(app: &mut PhotocraftApp, doc: DocId, params: &CameraRaw) {
    app.camera_raw_grades.insert(doc, params.clone());
}

/// The grade recorded for `from` now belongs to `to` (a raw file re-developed into a new document).
pub fn moved(app: &mut PhotocraftApp, from: DocId, to: DocId) {
    if let Some(g) = app.camera_raw_grades.remove(&from) {
        app.camera_raw_grades.insert(to, g);
    }
}

fn active_doc(app: &PhotocraftApp) -> Option<DocId> {
    app.session.active().map(|st| st.doc.id)
}

/// The grade Copy would take: the open dialog's, else the active document's.
fn source(app: &PhotocraftApp) -> Option<CameraRaw> {
    if let Some(d) = app.camera_raw.as_ref() {
        return Some(d.params.clone());
    }
    app.camera_raw_grades.get(&active_doc(app)?).cloned()
}

/// The copied grade, if any (and readable).
pub fn copied(app: &PhotocraftApp) -> Option<CameraRaw> {
    serde_json::from_value(app.session.prefs().dialogs.get(PREF)?.clone()).ok()
}

pub fn is_enabled(app: &PhotocraftApp, id: &str) -> Option<bool> {
    match id {
        COPY => Some(source(app).is_some()),
        PASTE => Some(
            copied(app).is_some()
                && (app.camera_raw.is_some() || photocraft_engine::commands::find("filter.cameraRaw").is_some_and(|c| (c.enabled)(&app.session).is_ok())),
        ),
        _ => None,
    }
}

fn copy(app: &mut PhotocraftApp) -> Result<Value, String> {
    let grade = source(app).ok_or(tl!("This image has no Camera Raw grade to copy yet: grade it in Camera Raw first."))?;
    let mut stored = serde_json::to_value(&grade).map_err(|e| e.to_string())?;
    if let Value::Object(m) = &mut stored {
        // Preview resolution and the document's encoding belong to the image, not the grade.
        m.remove("pixelScale");
        m.remove("encodingGamma");
    }
    app.session.prefs.edit(|p| p.dialogs.insert(PREF.into(), stored.clone()));
    app.ui.status_error = false;
    app.ui.status = tl!("Copied the Camera Raw grade").into();
    Ok(json!({"grade": stored}))
}

fn paste(app: &mut PhotocraftApp, ctx: &egui::Context) -> Result<Value, String> {
    let grade = copied(app).ok_or(tl!("No Camera Raw grade has been copied yet."))?;
    if app.camera_raw.is_none() {
        crate::camera_raw_ui::open(app, ctx)?;
    }
    crate::camera_raw_ui::set_params(app, grade)?;
    app.ui.status_error = false;
    app.ui.status = tl!("Pasted the Camera Raw grade: check the preview, then click OK").into();
    Ok(json!({"pasted": true}))
}

/// Menu / control-channel dispatch for the two commands.
pub fn menu(app: &mut PhotocraftApp, ctx: &egui::Context, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if id != COPY && id != PASTE {
        return None;
    }
    if !(params.is_null() || params.as_object().is_some_and(|m| m.is_empty())) {
        return Some(Err(format!("{id} takes no parameters")));
    }
    Some(if id == COPY { copy(app) } else { paste(app, ctx) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_image() -> PhotocraftApp {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", json!({"width": 40, "height": 30, "mode": "rgb", "depth": 8})).unwrap();
        app
    }

    #[test]
    fn copy_takes_the_recorded_grade_and_paste_opens_the_dialog_with_it() {
        let ctx = egui::Context::default();
        let mut a = app_with_image();
        assert!(!is_enabled(&a, COPY).unwrap(), "nothing graded yet");
        assert!(menu(&mut a, &ctx, COPY, &json!({})).unwrap().is_err());
        let grade = CameraRaw { exposure: 0.7, temperature: 12.0, vibrance: 25.0, ..Default::default() };
        let doc = a.session.active().unwrap().doc.id;
        record(&mut a, doc, &grade);
        assert!(is_enabled(&a, COPY).unwrap());
        menu(&mut a, &ctx, COPY, &json!({})).unwrap().unwrap();
        assert_eq!(copied(&a).unwrap(), grade);

        // Another image: paste opens Camera Raw with the grade, OK applies it and records it.
        a.session.execute("file.new", json!({"width": 20, "height": 20, "mode": "rgb", "depth": 16})).unwrap();
        assert!(is_enabled(&a, PASTE).unwrap());
        menu(&mut a, &ctx, PASTE, &json!({})).unwrap().unwrap();
        let d = a.camera_raw.as_ref().expect("dialog open");
        assert_eq!((d.params.exposure, d.params.temperature, d.params.vibrance), (0.7, 12.0, 25.0));
        assert!(menu(&mut a, &ctx, COPY, &json!(5)).unwrap().is_err());
        assert!(menu(&mut a, &ctx, "filter.blur.blur", &json!({})).is_none());
    }

    #[test]
    fn a_moved_document_keeps_its_grade() {
        let mut a = app_with_image();
        let grade = CameraRaw { contrast: 10.0, ..Default::default() };
        record(&mut a, DocId(1), &grade);
        moved(&mut a, DocId(1), DocId(2));
        assert!(!a.camera_raw_grades.contains_key(&DocId(1)));
        assert_eq!(a.camera_raw_grades.get(&DocId(2)), Some(&grade));
    }
}
