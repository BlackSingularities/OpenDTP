//! Pliki: nowy, otwórz, zapisz, eksport, druk przez PDF, kopie do odzyskania po awarii
//! i pytanie o niezapisane zmiany.

use slint::ComponentHandle;
use std::path::{Path, PathBuf};

use opendtp_core::io;
use opendtp_core::model::*;

use crate::editor::Editor;
use crate::settings::Prefs;
use crate::{refresh, sync_lists, toast, tr, State};

pub fn blank(prefs: &Prefs) -> Document {
    let mut d = Document::new();
    if let Some(n) = d.styles.iter_mut().find(|s| s.id == "normal") {
        n.chars.font = Some(prefs.default_font.clone());
        n.chars.size = Some(prefs.default_size);
    }
    d.meta.author = prefs.author.clone();
    d
}

pub fn doc_name(ed: &Editor) -> String {
    match &ed.path {
        Some(p) => p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
        None if !ed.doc.meta.title.is_empty() => ed.doc.meta.title.clone(),
        None => tr("Dokument bez tytułu").to_string(),
    }
}

/// Zastępuje bieżący dokument (wcześniej trzeba zapytać o niezapisane zmiany).
pub fn set_doc(st: &mut State, doc: Document, path: Option<PathBuf>) {
    st.ed = Editor::new(doc);
    st.ed.path = path;
    st.doc_theme = "classic".into();
    st.view.invalidate();
    if let Some(ui) = st.ui.upgrade() {
        let app = ui.global::<crate::App>();
        app.set_view_x(0.0);
        app.set_view_y(0.0);
        app.set_backstage(false);
        app.set_right_panel("".into());
    }
    st.view.vx = 0.0;
    st.view.vy = 0.0;
    refresh(st);
}

pub fn load_any(path: &Path) -> Result<Document, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    match ext.as_str() {
        "odtp" => io::odtp::from_bytes(&bytes),
        "docx" => io::docx_in::import(&bytes),
        "md" | "markdown" => Ok(io::plain::from_markdown(&String::from_utf8_lossy(&bytes))),
        _ => Ok(io::plain::from_text(&String::from_utf8_lossy(&bytes))),
    }
}

pub fn open_path(st: &mut State, p: &str) -> bool {
    let path = PathBuf::from(p);
    match load_any(&path) {
        Ok(doc) => {
            set_doc(st, doc, Some(path.clone()));
            st.prefs.add_recent(p);
            st.prefs.last_doc = p.to_string();
            st.prefs.save();
            sync_lists(st);
            true
        }
        Err(e) => {
            toast(st, &opendtp_core::trf!("Nie udało się otworzyć pliku: {}", e));
            false
        }
    }
}

pub fn open_dialog(st: &mut State) {
    let pick = rfd::FileDialog::new()
        .set_title(tr("Otwórz dokument"))
        .add_filter(tr("Dokumenty"), &["odtp", "docx", "txt", "md"])
        .add_filter("OpenDTP", &["odtp"])
        .add_filter("Microsoft Word", &["docx"])
        .add_filter(tr("Tekst i Markdown"), &["txt", "md"])
        .pick_file();
    if let Some(p) = pick {
        open_path(st, &p.to_string_lossy());
    }
}

fn write_as(st: &mut State, path: &Path) -> Result<(), String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    st.ed.doc.meta.modified = opendtp_core::util::iso_now();
    if st.ed.doc.meta.created.is_empty() {
        st.ed.doc.meta.created = st.ed.doc.meta.modified.clone();
    }
    match ext.as_str() {
        "docx" => {
            let b = io::docx::export(&st.ed.doc, &st.ed.layout, &io::docx::DocxOpts { exact_shapes: st.prefs.exact_shapes })?;
            std::fs::write(path, b).map_err(|e| e.to_string())
        }
        "txt" => std::fs::write(path, io::plain::to_text(&st.ed.doc)).map_err(|e| e.to_string()),
        "md" => std::fs::write(path, io::plain::to_markdown(&st.ed.doc)).map_err(|e| e.to_string()),
        _ => io::odtp::save(&st.ed.doc, path),
    }
}

/// Zapis. Zwraca, czy się udał (Anuluj w oknie – nie).
pub fn save(st: &mut State, as_new: bool) -> bool {
    let path = match (&st.ed.path, as_new) {
        (Some(p), false) => p.clone(),
        _ => {
            let name = format!("{}.odtp", doc_name(&st.ed));
            let Some(p) = rfd::FileDialog::new()
                .set_title(tr("Zapisz dokument"))
                .set_file_name(&name)
                .add_filter("OpenDTP", &["odtp"])
                .add_filter("Microsoft Word", &["docx"])
                .add_filter(tr("Tekst"), &["txt"])
                .add_filter("Markdown", &["md"])
                .save_file()
            else {
                return false;
            };
            p
        }
    };
    match write_as(st, &path) {
        Ok(()) => {
            st.ed.path = Some(path.clone());
            st.ed.dirty = false;
            let p = path.to_string_lossy().to_string();
            st.prefs.add_recent(&p);
            st.prefs.last_doc = p;
            st.prefs.save();
            sync_lists(st);
            clear_recovery(st);
            toast(st, &opendtp_core::trf!("Zapisano: {}", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
            refresh(st);
            true
        }
        Err(e) => {
            toast(st, &opendtp_core::trf!("Nie udało się zapisać: {}", e));
            false
        }
    }
}

pub fn export(st: &mut State, kind: &str) {
    let (ext, filter) = match kind {
        "docx" => ("docx", "Microsoft Word"),
        "pdf" | "pdf-print" => ("pdf", "PDF"),
        "epub" => ("epub", "EPUB"),
        "html" => ("html", "HTML"),
        "md" => ("md", "Markdown"),
        "png" => ("png", "PNG"),
        _ => ("txt", "Tekst"),
    };
    let name = if kind == "pdf-print" { format!("{}-druk.pdf", doc_name(&st.ed)) } else { format!("{}.{}", doc_name(&st.ed), ext) };
    let mut dlg = rfd::FileDialog::new().set_title(tr("Eksportuj")).set_file_name(&name).add_filter(filter, &[ext]);
    if let Some(dir) = st.ed.path.as_ref().and_then(|p| p.parent()) {
        dlg = dlg.set_directory(dir);
    }
    let Some(path) = dlg.save_file() else { return };
    let res: Result<(), String> = (|| {
        let doc = &st.ed.doc;
        let lay = &st.ed.layout;
        let bytes = match kind {
            "docx" => io::docx::export(doc, lay, &io::docx::DocxOpts { exact_shapes: st.prefs.exact_shapes })?,
            "pdf" => io::pdf::export(doc, lay)?,
            "pdf-print" => io::pdf::export_with(doc, lay, true)?,
            "epub" => io::epub::export(doc)?,
            "html" => io::plain::to_html(doc).into_bytes(),
            "md" => io::plain::to_markdown(doc).into_bytes(),
            "png" => page_png(st, st.ed.caret_page().map(|c| c.0).unwrap_or(0), 300.0)?,
            _ => io::plain::to_text(doc).into_bytes(),
        };
        std::fs::write(&path, bytes).map_err(|e| e.to_string())
    })();
    match res {
        Ok(()) => {
            toast(st, &opendtp_core::trf!("Wyeksportowano: {}", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
            if let Some(ui) = st.ui.upgrade() {
                ui.global::<crate::App>().set_backstage(false);
            }
        }
        Err(e) => toast(st, &opendtp_core::trf!("Eksport nie powiódł się: {}", e)),
    }
}

pub fn page_png(st: &State, page: usize, dpi: f32) -> Result<Vec<u8>, String> {
    let scale = dpi / 72.0;
    let doc = &st.ed.doc;
    let mut pm = tiny_skia::Pixmap::new((doc.page.width * scale) as u32, (doc.page.height * scale) as u32).ok_or("za duża strona")?;
    let opts = opendtp_core::paint::PaintOpts { screen: false, marks: false, guides: false, misspelled: None, hide_obj: None };
    let ops = opendtp_core::paint::page_ops(doc, &st.ed.layout, page, &opts);
    let mut r = opendtp_core::raster::Raster::new();
    r.draw(doc, &ops, &mut pm, scale);
    pm.encode_png().map_err(|e| e.to_string())
}

/// Druk: PDF do folderu tymczasowego i otwarcie w domyślnej przeglądarce PDF (tam Drukuj).
pub fn print(st: &mut State) {
    match io::pdf::export(&st.ed.doc, &st.ed.layout) {
        Ok(b) => {
            let p = opendtp_core::paths::cache().join(format!("{}-druk.pdf", doc_name(&st.ed)));
            if std::fs::write(&p, b).is_ok() {
                let _ = open::that(&p);
                toast(st, tr("Otwarto PDF do wydruku"));
            }
        }
        Err(e) => toast(st, &opendtp_core::trf!("Nie udało się przygotować wydruku: {}", e)),
    }
}

// ---------------------------------------------------------------- odzyskiwanie

fn recovery_file(st: &State) -> PathBuf {
    opendtp_core::paths::recovery().join(format!("sesja-{}.odtp", st.session))
}

pub fn autosave_tick(st: &mut State) {
    static mut LAST: Option<std::time::Instant> = None;
    let every = st.prefs.autosave_secs();
    if every == 0 || !st.ed.dirty {
        return;
    }
    let now = std::time::Instant::now();
    // Jeden wątek UI – prosty licznik czasu bez synchronizacji.
    let due = unsafe {
        let last = *std::ptr::addr_of!(LAST);
        match last {
            Some(t) if now.duration_since(t).as_secs() < every => false,
            _ => {
                *std::ptr::addr_of_mut!(LAST) = Some(now);
                true
            }
        }
    };
    if !due {
        return;
    }
    let mut doc = st.ed.doc.clone();
    if let Some(p) = &st.ed.path {
        doc.meta.keywords = format!("{}\u{1}{}", doc.meta.keywords, p.display());
    }
    if let Ok(b) = io::odtp::to_bytes(&doc) {
        let _ = std::fs::write(recovery_file(st), b);
    }
}

pub fn clear_recovery(st: &State) {
    let _ = std::fs::remove_file(recovery_file(st));
}

/// Kopia z poprzedniej sesji (program zamknięty bez zapisu albo awaria) – otwieramy ją.
pub fn recover(st: &mut State) -> bool {
    let dir = opendtp_core::paths::recovery();
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&dir)
        .map(|d| d.flatten().filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).filter(|(_, p)| p.extension().is_some_and(|x| x == "odtp")).collect())
        .unwrap_or_default();
    files.sort();
    let Some((_, path)) = files.pop() else { return false };
    let ok = match io::odtp::load(&path) {
        Ok(mut doc) => {
            let mut orig = None;
            if let Some((k, p)) = doc.meta.keywords.clone().split_once('\u{1}') {
                doc.meta.keywords = k.to_string();
                orig = Some(PathBuf::from(p));
            }
            set_doc(st, doc, orig);
            st.ed.dirty = true;
            st.ed.recovered = true;
            toast(st, tr("Odzyskano niezapisany dokument z poprzedniej sesji"));
            true
        }
        Err(_) => false,
    };
    for (_, p) in files {
        let _ = std::fs::remove_file(p);
    }
    let _ = std::fs::remove_file(path);
    ok
}

// ---------------------------------------------------------------- niezapisane zmiany

/// Pyta o zapis przed akcją `then` (np. „exit”, „new:report”, „open”, „open-path:…”).
pub fn confirm(st: &mut State, then: &str) {
    if let Some(ui) = st.ui.upgrade() {
        let app = ui.global::<crate::App>();
        app.set_confirm_text(opendtp_core::trf!("Czy zapisać zmiany w dokumencie „{}”?", doc_name(&st.ed)).into());
        app.set_confirm_id(then.into());
        app.set_dialog("confirm".into());
    }
}

/// Przed zastąpieniem dokumentu: gdy są zmiany – pytanie i `false` (akcja wróci po odpowiedzi).
pub fn may_replace(st: &mut State, then: &str) -> bool {
    if st.ed.dirty {
        confirm(st, then);
        return false;
    }
    true
}

pub fn on_exit(st: &mut State) {
    clear_recovery(st);
    // Aktualizacja pobrana w tle – instalacja po zamknięciu (bez ponownego uruchamiania).
    if let Some(p) = st.pending_setup.take() {
        if opendtp_core::update::installed() {
            let _ = opendtp_core::update::apply(&p, false);
        } else {
            let _ = opendtp_core::update::apply_portable(&p);
        }
    }
    st.prefs.zoom = (st.view.zoom * 100.0).round() as i32;
    st.prefs.marks = st.view.marks;
    st.prefs.guides = st.view.guides;
    st.prefs.spell_live = st.view.spell;
    if let Some(ui) = st.ui.upgrade() {
        st.prefs.tab = ui.global::<crate::App>().get_tab();
    }
    st.prefs.last_doc = st.ed.path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    st.prefs.save();
}

pub fn quit(st: &mut State) {
    on_exit(st);
    let _ = slint::quit_event_loop();
}
