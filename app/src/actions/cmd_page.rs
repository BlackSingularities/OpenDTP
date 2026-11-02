//! Polecenia tła strony, układu i typografii dokumentu.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle(st: &mut State, ui: &crate::AppWindow, app: &App, unit: i32, uk: f32, is_step: bool, base: &str, id: &str, arg: &str, scroll: &mut bool) -> Flow {
    let _ = (ui, unit, uk, is_step, id, &scroll);
    match base {
        // ---------------------------------------------------------- tło strony i typografia
        "page-color" => {
            let c = color_of(app.get_page_color());
            st.ed.begin(Kind::Other);
            st.ed.doc.page.color = c;
            st.ed.changed();
        }
        "orphans" => {
            st.ed.begin(Kind::Other);
            st.ed.doc.settings.orphans = !st.ed.doc.settings.orphans;
            st.ed.changed();
        }
        "baseline-grid" => {
            st.ed.begin(Kind::Other);
            let g = st.ed.doc.settings.baseline_grid;
            st.ed.doc.settings.baseline_grid = if g > 0.0 { 0.0 } else { 14.0 };
            st.ed.changed();
        }
        "page-size" => {
            if let Some((_, w, h)) = PAGE_SIZES.iter().find(|p| p.0 == arg) {
                st.ed.begin(Kind::Other);
                let land = st.ed.doc.page.landscape();
                let (w, h) = (w * MM, h * MM);
                st.ed.doc.page.width = if land { h.max(w) } else { w.min(h) };
                st.ed.doc.page.height = if land { w.min(h) } else { h.max(w) };
                st.ed.changed();
                st.view.invalidate();
            }
        }
        "page-w" | "page-h" => {
            let p = &st.ed.doc.page;
            let cur = if base == "page-w" { p.width } else { p.height } / uk;
            let step = if unit == 0 { 1.0 } else if unit == 3 { 2.0 } else { 0.1 };
            if let Some(v) = value_or_step(arg, cur, step, is_step) {
                // Od 2 cm do 5 m – dalej to już nie strona.
                let v = (v * uk).clamp(20.0 * MM, 5000.0 * MM);
                st.ed.begin(Kind::Other);
                let pg = &mut st.ed.doc.page;
                if base == "page-w" {
                    pg.width = v;
                } else {
                    pg.height = v;
                }
                st.ed.changed();
                st.view.invalidate();
            }
        }
        "page-dialog" => {
            app.set_dialog("page".into());
            return Flow::Skip;
        }
        "orientation" => {
            st.ed.begin(Kind::Other);
            match section_of(st) {
                Some(s) => s.landscape = !s.landscape,
                None => {
                    let p = &mut st.ed.doc.page;
                    std::mem::swap(&mut p.width, &mut p.height);
                    // Sekcje zachowują własną orientację względem nowego układu.
                    for q in st.ed.doc.body.paras.iter_mut() {
                        if let Some(s) = q.fmt.section.as_mut() {
                            s.landscape = !s.landscape;
                        }
                    }
                }
            }
            st.ed.changed();
            st.view.invalidate();
        }
        "margins" => {
            let (t, b, l, r) = match arg {
                "narrow" => (12.7, 12.7, 12.7, 12.7),
                "moderate" => (25.4, 25.4, 19.1, 19.1),
                "wide" => (25.4, 25.4, 38.0, 38.0),
                "book" => (20.0, 25.0, 30.0, 20.0),
                "none" => (0.0, 0.0, 0.0, 0.0),
                _ => (25.0, 25.0, 25.0, 25.0),
            };
            st.ed.begin(Kind::Other);
            let p = &mut st.ed.doc.page;
            p.margin_top = t * MM;
            p.margin_bottom = b * MM;
            p.margin_left = l * MM;
            p.margin_right = r * MM;
            st.ed.changed();
        }
        "m-top" | "m-bottom" | "m-left" | "m-right" | "col-gap" => {
            let p = st.ed.doc.page.clone();
            let cur = match base {
                "m-top" => p.margin_top,
                "m-bottom" => p.margin_bottom,
                "m-left" => p.margin_left,
                "m-right" => p.margin_right,
                _ => p.column_gap,
            } / uk;
            let step = if unit == 0 { 1.0 } else if unit == 2 { 0.1 } else if unit == 1 { 0.1 } else { 2.0 };
            if let Some(v) = value_or_step(arg, cur, step, is_step) {
                let v = (v * uk).clamp(0.0, p.width / 2.0 - 20.0);
                st.ed.begin(Kind::Other);
                let pg = &mut st.ed.doc.page;
                match base {
                    "m-top" => pg.margin_top = v,
                    "m-bottom" => pg.margin_bottom = v,
                    "m-left" => pg.margin_left = v,
                    "m-right" => pg.margin_right = v,
                    _ => pg.column_gap = v,
                }
                st.ed.changed();
            }
        }
        "columns" => {
            if let Ok(n) = arg.parse::<u8>() {
                st.ed.begin(Kind::Other);
                match section_of(st) {
                    Some(s) => s.columns = n.clamp(1, 6),
                    None => st.ed.doc.page.columns = n.clamp(1, 6),
                }
                st.ed.changed();
            }
        }
        "col-rule" => {
            st.ed.begin(Kind::Other);
            match section_of(st) {
                Some(s) => s.column_rule = !s.column_rule,
                None => st.ed.doc.page.column_rule = !st.ed.doc.page.column_rule,
            }
            st.ed.changed();
        }
        "section-new" | "section-new-cont" => {
            if st.ed.story != StoryRef::Body {
                toast(st, tr("Sekcje dotyczą treści głównej"));
                return Flow::Skip;
            }
            let cont = arg == "cont" || id == "section-new-cont";
            let sec = st.ed.doc.setup_at(st.ed.head.para).section(cont);
            st.ed.begin(Kind::Other);
            st.ed.enter();
            let pi = st.ed.head.para;
            if let Some(p) = st.ed.doc.body.paras.get_mut(pi) {
                p.fmt.section = Some(sec);
            }
            st.ed.changed();
            toast(st, tr("Nowa sekcja – kolumny i orientacja ustawiasz teraz osobno dla niej"));
        }
        "section-remove" => {
            if let Some(i) = (st.ed.story == StoryRef::Body).then(|| st.ed.doc.section_start(st.ed.head.para)).flatten() {
                st.ed.begin(Kind::Other);
                st.ed.doc.body.paras[i].fmt.section = None;
                st.ed.changed();
            }
        }
        "footnote" | "endnote" => {
            if st.ed.story != StoryRef::Body {
                toast(st, tr("Przypis wstawisz w treści głównej"));
                return Flow::Skip;
            }
            st.ed.begin(Kind::Other);
            let id = st.ed.doc.new_id();
            let mut p = Paragraph::with_text("normal", "");
            p.runs[0].fmt.size = Some(9.0);
            p.fmt.space_after = Some(0.0);
            st.ed.doc.notes.push(Note { id, end: base == "endnote", story: Story { paras: vec![p] } });
            let mut fmt = st.ed.caret_fmt();
            fmt.script = Some(Script::Super);
            fmt.link = None;
            let h = st.ed.head;
            let end = opendtp_core::edit::insert_field(&mut st.ed.doc.body, h, Field::Footnote(id), &fmt);
            st.ed.anchor = end;
            st.ed.head = end;
            st.ed.changed();
            st.ed.story = StoryRef::Note(id);
            st.ed.anchor = At::new(0, 0);
            st.ed.head = At::new(0, 0);
            *scroll = true;
        }
        "note-back" => {
            if let StoryRef::Note(id) = st.ed.story {
                let pos = st.ed.doc.body.paras.iter().enumerate().find_map(|(pi, p)| {
                    let mut off = 0;
                    for r in &p.runs {
                        off += r.text.len();
                        if r.field == Some(Field::Footnote(id)) {
                            return Some(At::new(pi, off));
                        }
                    }
                    None
                });
                st.ed.story = StoryRef::Body;
                let at = pos.unwrap_or(At::new(0, 0));
                st.ed.anchor = at;
                st.ed.head = at;
                *scroll = true;
            }
        }

        _ => return Flow::Unhandled,
    }
    Flow::Done
}
