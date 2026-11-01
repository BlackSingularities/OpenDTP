//! Polecenia wstążki, menu, palety i skrótów: `run(id, argument)`.

use slint::ComponentHandle;
use std::time::Duration;

use opendtp_core::edit::At;
use opendtp_core::geom::{self, BoolOp};
use opendtp_core::model::*;
use opendtp_core::{io, styles};
use slint::{ModelRc, VecModel};

use crate::editor::Kind;
use crate::input::set_tool;
use crate::{commands, files, len_str, num, parse_num, refresh, refresh_scroll, sync_lists, toast, tr, unit_k, App, ItemV, State};

pub const PAGE_SIZES: [(&str, f32, f32); 9] = [
    ("A4", 210.0, 297.0),
    ("A5", 148.0, 210.0),
    ("A3", 297.0, 420.0),
    ("B5", 176.0, 250.0),
    ("Letter", 215.9, 279.4),
    ("Legal", 215.9, 355.6),
    ("A6", 105.0, 148.0),
    ("DL", 99.0, 210.0),
    ("Square", 210.0, 210.0),
];

fn ui_app(st: &State) -> Option<crate::AppWindow> {
    st.ui.upgrade()
}

pub fn kind_label(o: &Object) -> String {
    match &o.kind {
        ObjKind::Text { .. } if o.shape.is_rect() => tr("Ramka tekstowa").into(),
        ObjKind::Text { .. } => tr("Kształt tekstu").into(),
        ObjKind::Image { .. } => tr("Obraz").into(),
        ObjKind::Shape => tr("Kształt").into(),
        ObjKind::Table { .. } => tr("Tabela").into(),
        ObjKind::Chart { .. } => tr("Wykres").into(),
        ObjKind::PathText { .. } => tr("Tekst na ścieżce").into(),
    }
}

fn color_of(c: slint::Color) -> Color {
    Color { r: c.red(), g: c.green(), b: c.blue(), a: c.alpha() }
}

fn list_kind(id: &str) -> Option<ListKind> {
    Some(match id {
        "bullet" => ListKind::Bullet,
        "dash" => ListKind::Dash,
        "check" => ListKind::Check,
        "arrow" => ListKind::Arrow,
        "number" => ListKind::Number,
        "number-paren" => ListKind::NumberParen,
        "lower-letter" => ListKind::LowerLetter,
        "upper-letter" => ListKind::UpperLetter,
        "lower-roman" => ListKind::LowerRoman,
        "upper-roman" => ListKind::UpperRoman,
        _ => return None,
    })
}

fn wrap_mode(i: i32) -> WrapMode {
    match i {
        1 => WrapMode::Both,
        2 => WrapMode::Left,
        3 => WrapMode::Right,
        4 => WrapMode::Largest,
        5 => WrapMode::TopBottom,
        6 => WrapMode::Behind,
        _ => WrapMode::None,
    }
}

/// Wartość z pola liczbowego: liczba albo krok (+/−) od bieżącej.
fn value_or_step(arg: &str, cur: f32, step: f32, is_step: bool) -> Option<f32> {
    if is_step {
        let d: f32 = arg.parse().unwrap_or(0.0);
        Some(cur + d * step)
    } else {
        parse_num(arg)
    }
}

pub fn run(st: &mut State, id: &str, arg: &str) {
    // Polecenia z palety i paska szybkiego dostępu: „id:arg”.
    if arg.is_empty() {
        if let Some((a, b)) = id.split_once(':') {
            if !id.starts_with("spell-") {
                let (a, b) = (a.to_string(), b.to_string());
                return run(st, &a, &b);
            }
        }
        if let Some(rest) = id.strip_prefix("text-shape-") {
            let rest = rest.to_string();
            return run(st, "text-shape", &rest);
        }
        if let Some(rest) = id.strip_prefix("shape-") {
            let rest = rest.to_string();
            return run(st, "shape", &rest);
        }
        if let Some(rest) = id.strip_prefix("chart-").filter(|r| !matches!(*r, "data" | "title" | "kind" | "legend" | "values")) {
            let rest = rest.to_string();
            return run(st, "chart", &rest);
        }
        if let Some(rest) = id.strip_prefix("columns-") {
            let rest = rest.to_string();
            return run(st, "columns", &rest);
        }
        if let Some(rest) = id.strip_prefix("export-") {
            let rest = rest.to_string();
            return run(st, "export", &rest);
        }
        if let Some(rest) = id.strip_prefix("bool-") {
            let rest = rest.to_string();
            return run(st, "bool", &rest);
        }
        if id == "field-page" {
            return run(st, "field", "page-number");
        }
        if id == "field-date" {
            return run(st, "field", "date");
        }
    }
    let Some(ui) = ui_app(st) else { return };
    let app = ui.global::<App>();
    let unit = st.prefs.unit;
    let uk = unit_k(unit);
    let is_step = id.ends_with("-step");
    let base = id.trim_end_matches("-step");
    let mut scroll = false;
    match base {
        // ---------------------------------------------------------- plik
        "backstage" => {
            app.set_backstage_page(0);
            app.set_backstage(true);
            let (w, ns, c, p, pg) = st.ed.stats();
            app.set_stats_text(opendtp_core::trf!("Strony: {pg}\nAkapity: {p}\nWyrazy: {w}\nZnaki (bez spacji): {ns}\nZnaki (ze spacjami): {c}").into());
            return;
        }
        "new" => {
            let t = if arg.is_empty() { "blank" } else { arg };
            if !files::may_replace(st, &format!("new:{t}")) {
                return;
            }
            let doc = if t == "blank" { files::blank(&st.prefs) } else { opendtp_core::sample::template(t) };
            files::set_doc(st, doc, None);
            return;
        }
        "open" => {
            if !files::may_replace(st, "open") {
                return;
            }
            files::open_dialog(st);
            return;
        }
        "open-path" => {
            if !files::may_replace(st, &format!("open-path:{arg}")) {
                return;
            }
            files::open_path(st, arg);
            return;
        }
        "recent-remove" => {
            st.prefs.recent.retain(|(p, _)| p != arg);
            st.prefs.save();
            sync_lists(st);
            return;
        }
        "save" => {
            files::save(st, false);
            app.set_backstage(false);
            return;
        }
        "save-as" => {
            files::save(st, true);
            app.set_backstage(false);
            return;
        }
        "export" => {
            files::export(st, arg);
            return;
        }
        "print" => {
            files::print(st);
            return;
        }
        "doc-info" => {
            let (w, ns, c, p, pg) = st.ed.stats();
            app.set_stats_text(opendtp_core::trf!("Strony: {pg}\nAkapity: {p}\nWyrazy: {w}\nZnaki (bez spacji): {ns}\nZnaki (ze spacjami): {c}").into());
            return;
        }
        "meta-title" => {
            st.ed.doc.meta.title = arg.to_string();
            st.ed.dirty = true;
        }
        "meta-author" => {
            st.ed.doc.meta.author = arg.to_string();
            st.ed.dirty = true;
        }
        "confirm-save" | "confirm-discard" => {
            if base == "confirm-save" && !files::save(st, false) {
                return;
            }
            st.ed.dirty = false;
            let then = arg.to_string();
            if then == "exit" {
                st.pending_close = true;
                files::quit(st);
                return;
            }
            let (a, b) = then.split_once(':').unwrap_or((then.as_str(), ""));
            let (a, b) = (a.to_string(), b.to_string());
            return run(st, &a, &b);
        }
        "settings" => {
            app.set_backstage_page(6);
            app.set_backstage(true);
            return;
        }
        "about" => {
            app.set_backstage_page(7);
            app.set_backstage(true);
            return;
        }
        "website" => {
            let _ = open::that(opendtp_core::AUTHOR_URL);
            return;
        }
        "support" => {
            let _ = open::that(opendtp_core::SUPPORT_URL);
            return;
        }
        "check-update" => {
            check_update(true);
            return;
        }
        "install-update" => {
            install_update(st);
            return;
        }

        // ---------------------------------------------------------- edycja
        "undo" => {
            if !st.ed.undo() {
                toast(st, tr("Nie ma czego cofnąć"));
            }
            st.view.invalidate();
            scroll = true;
        }
        "redo" => {
            st.ed.redo();
            st.view.invalidate();
            scroll = true;
        }
        "copy" | "cut" => {
            if st.ed.has_sel() {
                let frag = st.ed.fragment();
                let text = opendtp_core::edit::fragment_text(&frag);
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(text.clone());
                }
                st.clip = Some((text, frag));
                if base == "cut" {
                    st.ed.delete_sel();
                }
            } else if let Some(o) = st.ed.object().cloned() {
                // Obiekt w schowku wewnętrznym (wklejanie duplikuje).
                st.clip = Some((format!("\u{1}obj{}", o.id), Vec::new()));
                OBJ_CLIP.with(|c| *c.borrow_mut() = Some(o));
                if base == "cut" {
                    st.ed.delete_object();
                }
            }
        }
        "paste" | "paste-plain" => {
            let sys = arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok());
            let obj = OBJ_CLIP.with(|c| c.borrow().clone());
            if let (Some((t, _)), Some(o)) = (&st.clip, obj) {
                if t.starts_with("\u{1}obj") && sys.as_deref().is_none_or(|s| s.is_empty() || s == t.as_str()) {
                    let mut c = o;
                    c.x += 12.0;
                    c.y += 12.0;
                    c.page = st.ed.caret_page().map(|p| p.0).unwrap_or(c.page);
                    if let ObjKind::Text { next, .. } = &mut c.kind {
                        *next = None;
                    }
                    st.ed.add_object(c);
                    refresh(st);
                    return;
                }
            }
            if base == "paste" {
                if let Some((t, frag)) = &st.clip {
                    if sys.as_deref() == Some(t.as_str()) && !frag.is_empty() {
                        let f = frag.clone();
                        st.ed.paste(&f);
                        refresh_scroll(st);
                        return;
                    }
                }
                // Obraz ze schowka.
                if sys.as_deref().is_none_or(|s| s.is_empty()) {
                    if let Some(img) = arboard::Clipboard::new().ok().and_then(|mut c| c.get_image().ok()) {
                        let mut png = Vec::new();
                        if let Some(buf) = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned()) {
                            let _ = image::DynamicImage::ImageRgba8(buf).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png);
                            insert_image_bytes(st, png);
                        }
                        return;
                    }
                }
            }
            if let Some(t) = sys {
                if !t.is_empty() {
                    st.ed.paste_text(&t);
                    scroll = true;
                }
            }
        }
        "select-all" => {
            st.ed.select_all();
        }
        "format-painter" | "copy-format" => {
            let (cp, _, style) = st.ed.caret_props();
            let _ = cp;
            let f = st.ed.caret_fmt();
            let p = st.ed.story_ref().paras[st.ed.range().0.para].fmt.clone();
            st.ed.painter = Some((f, Some((style, p))));
            if base == "format-painter" {
                set_tool(st, 6);
                toast(st, tr("Zaznacz tekst, który ma dostać to formatowanie"));
            } else {
                toast(st, tr("Skopiowano formatowanie (Ctrl+Shift+V wkleja)"));
            }
            return;
        }
        "find" | "replace" => {
            app.set_right_panel("find".into());
            if st.ed.has_sel() {
                let f = st.ed.fragment();
                let t = opendtp_core::edit::fragment_text(&f);
                if !t.contains('\n') && t.len() < 100 {
                    app.set_find_text(t.clone().into());
                    st.ed.find_run(&t, &find_opts(&app));
                }
            }
            find_info(st);
        }
        "find-run" => {
            st.ed.find_run(arg, &find_opts(&app));
            find_info(st);
        }
        "find-next" | "find-prev" => {
            if st.ed.find.is_empty() {
                let t = app.get_find_text().to_string();
                st.ed.find_run(&t, &find_opts(&app));
                st.ed.find_i = st.ed.find_i.wrapping_sub(1);
            }
            if st.ed.story != StoryRef::Body {
                st.ed.set_caret(StoryRef::Body, At::new(0, 0), false);
            }
            st.ed.find_select(if base == "find-next" { 1 } else { -1 });
            find_info(st);
            scroll = true;
        }
        "find-close" => {
            st.ed.find.clear();
        }
        "replace-one" => {
            let with = app.get_replace_text().to_string();
            if st.ed.has_sel() {
                st.ed.insert(&with);
            }
            let t = app.get_find_text().to_string();
            st.ed.find_run(&t, &find_opts(&app));
            st.ed.find_select(0);
            find_info(st);
            scroll = true;
        }
        "replace-all" => {
            let n = st.ed.replace_all(&app.get_find_text(), &app.get_replace_text(), &find_opts(&app));
            toast(st, &opendtp_core::trf!("Zamieniono: {}", n));
            st.ed.find.clear();
            find_info(st);
        }

        // ---------------------------------------------------------- znaki
        "bold" => st.ed.toggle(|c| c.bold, |f, v| f.bold = Some(v)),
        "italic" => st.ed.toggle(|c| c.italic, |f, v| f.italic = Some(v)),
        "underline" => st.ed.toggle(|c| c.underline != Underline::None, |f, v| f.underline = Some(if v { Underline::Single } else { Underline::None })),
        "underline-kind" => {
            let u = match arg {
                "double" => Underline::Double,
                "dotted" => Underline::Dotted,
                "wave" => Underline::Wave,
                "none" => Underline::None,
                _ => Underline::Single,
            };
            st.ed.format(&move |f| f.underline = Some(u));
        }
        "strike" => st.ed.toggle(|c| c.strike, |f, v| f.strike = Some(v)),
        "sup" => st.ed.toggle(|c| c.script == Script::Super, |f, v| f.script = Some(if v { Script::Super } else { Script::Normal })),
        "sub" => st.ed.toggle(|c| c.script == Script::Sub, |f, v| f.script = Some(if v { Script::Sub } else { Script::Normal })),
        "caps" => match arg {
            "0" | "1" | "2" => {
                let c = match arg {
                    "1" => Caps::All,
                    "2" => Caps::Small,
                    _ => Caps::None,
                };
                st.ed.format(&move |f| f.caps = Some(c));
            }
            how => change_case(st, how),
        },
        "font" => {
            let f = arg.to_string();
            st.prefs.add_recent_font(&f);
            sync_lists(st);
            st.ed.format(&move |x| x.font = Some(f.clone()));
            app.set_focus_serial(app.get_focus_serial() + 1);
        }
        "size" => {
            if let Some(v) = parse_num(arg) {
                let v = v.clamp(1.0, 1600.0);
                st.ed.format(&move |f| f.size = Some(v));
            }
            app.set_focus_serial(app.get_focus_serial() + 1);
        }
        "grow" | "shrink" => {
            let (cp, _, _) = st.ed.caret_props();
            let steps = [6.0, 7.0, 8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 28.0, 32.0, 36.0, 40.0, 48.0, 54.0, 60.0, 72.0, 96.0, 120.0, 144.0];
            let cur = cp.size;
            let v = if base == "grow" { steps.iter().copied().find(|s| *s > cur + 0.01).unwrap_or(cur + 12.0) } else { steps.iter().rev().copied().find(|s| *s < cur - 0.01).unwrap_or((cur - 1.0).max(1.0)) };
            st.ed.format(&move |f| f.size = Some(v));
        }
        "clear-format" => st.ed.clear_format(),
        "color" => {
            let c = color_of(app.get_last_color());
            st.ed.format(&move |f| f.color = Some(c));
        }
        "highlight" => {
            let c = color_of(app.get_last_highlight());
            st.ed.format(&move |f| f.highlight = Some(c));
        }
        "pick-hex" | "color-hex-check" => {
            if let Some(c) = Color::parse(arg) {
                app.set_last_color(slint::Color::from_rgb_u8(c.r, c.g, c.b));
                st.ed.format(&move |f| f.color = Some(c));
            } else {
                toast(st, tr("Nieprawidłowy kolor – wpisz np. #1f4e79"));
                return;
            }
        }
        "tracking" => {
            let (cp, _, _) = st.ed.caret_props();
            if let Some(v) = value_or_step(arg, cp.tracking, 0.5, is_step) {
                st.ed.format(&move |f| f.tracking = Some(v));
            }
        }
        "lang" => {
            let l = arg.to_string();
            st.ed.format(&move |f| f.lang = Some(l.clone()));
            st.view.invalidate();
        }
        "link" => {
            let f = st.ed.caret_fmt();
            app.set_link_url(f.link.unwrap_or_default().into());
            app.set_dialog("link".into());
            return;
        }
        "link-apply" => {
            app.set_dialog("".into());
            let url = arg.trim().to_string();
            if url.is_empty() {
                st.ed.format(&|f| {
                    f.link = Some(String::new());
                    f.underline = None;
                    f.color = None;
                });
            } else {
                let url = if url.contains("://") || url.starts_with("mailto:") { url } else { format!("https://{url}") };
                if !st.ed.has_sel() {
                    st.ed.insert(&url);
                    let h = st.ed.head;
                    st.ed.anchor = At::new(h.para, h.off - url.len());
                }
                st.ed.format(&move |f| {
                    f.link = Some(url.clone());
                    f.underline = Some(Underline::Single);
                    f.color = Some(Color::rgb(0x05, 0x63, 0xc1));
                });
            }
        }

        // ---------------------------------------------------------- akapit
        "align" => {
            let a = match arg {
                "1" => Align::Center,
                "2" => Align::Right,
                "3" => Align::Justify,
                _ => Align::Left,
            };
            st.ed.format_paras(&move |p| p.fmt.align = Some(a));
        }
        "list" => match arg {
            "restart" => st.ed.restart_numbering(),
            k => st.ed.set_list(list_kind(k)),
        },
        "indent" => st.ed.indent(1),
        "outdent" => st.ed.indent(-1),
        "line-spacing" => match arg {
            "exact" => {
                let (cp, _, _) = st.ed.caret_props();
                let v = cp.size * 1.2;
                st.ed.format_paras(&move |p| p.fmt.line = Some(LineSpacing::Exact(v)));
            }
            "before+" => st.ed.format_paras(&|p| p.fmt.space_before = Some(12.0)),
            "after0" => st.ed.format_paras(&|p| p.fmt.space_after = Some(0.0)),
            v => {
                if let Some(k) = parse_num(v) {
                    st.ed.format_paras(&move |p| p.fmt.line = Some(LineSpacing::Multiple(k)));
                }
            }
        },
        "shading" => {
            let c = color_of(app.get_shading());
            st.ed.format_paras(&move |p| p.fmt.shading = Some(c));
        }
        "indent-left" | "indent-right" | "first-line" | "space-before" | "space-after" => {
            let (_, pp, _) = st.ed.caret_props();
            let (cur, k, step) = match base {
                "indent-left" => (pp.indent_left / uk, uk, 1.0),
                "indent-right" => (pp.indent_right / uk, uk, 1.0),
                "first-line" => (pp.first_line / uk, uk, 1.0),
                "space-before" => (pp.space_before, 1.0, 2.0),
                _ => (pp.space_after, 1.0, 2.0),
            };
            let step = if k > 1.0 && unit == 0 { 2.5 } else if k > 1.0 && unit == 2 { 0.1 } else { step };
            if let Some(v) = value_or_step(arg, cur, step, is_step) {
                let v = v * k;
                let b = base.to_string();
                st.ed.format_paras(&move |p| match b.as_str() {
                    "indent-left" => p.fmt.indent_left = Some(v.max(0.0)),
                    "indent-right" => p.fmt.indent_right = Some(v.max(0.0)),
                    "first-line" => p.fmt.first_line = Some(v),
                    "space-before" => p.fmt.space_before = Some(v.max(0.0)),
                    _ => p.fmt.space_after = Some(v.max(0.0)),
                });
            }
        }
        "drop-cap" => {
            let (_, pp, _) = st.ed.caret_props();
            let on = pp.drop_cap == 0;
            st.ed.format_paras(&move |p| p.fmt.drop_cap = Some(if on { 3 } else { 0 }));
        }
        "style" => {
            st.ed.set_style(arg);
        }
        "style-update" => {
            let sid = if arg.is_empty() { st.ed.caret_props().2 } else { arg.to_string() };
            let f = st.ed.caret_fmt();
            st.ed.begin(Kind::Other);
            if let Some(s) = st.ed.doc.styles.iter_mut().find(|s| s.id == sid) {
                s.chars.merge(&f);
            }
            st.ed.changed();
            toast(st, tr("Styl zaktualizowany według zaznaczenia"));
        }
        "style-new" => {
            let f = st.ed.caret_fmt();
            let (_, _, cur) = st.ed.caret_props();
            let n = st.ed.doc.styles.len();
            let id = format!("custom{}", st.ed.doc.new_id());
            let name = opendtp_core::trf!("Mój styl {}", n - 14);
            st.ed.begin(Kind::Other);
            st.ed.doc.styles.push(Style { id: id.clone(), name, based_on: Some(cur), next: None, para: ParaFmt::default(), chars: f, outline: None, gallery: true });
            st.ed.changed();
            st.ed.set_style(&id);
            app.set_left_panel("styles".into());
        }

        // ---------------------------------------------------------- wstawianie
        "page-break" => {
            st.ed.page_break(false);
            scroll = true;
        }
        "column-break" => {
            st.ed.page_break(true);
            scroll = true;
        }
        "page-blank" => {
            st.ed.begin(Kind::Other);
            let n = st.ed.layout.pages.len();
            st.ed.doc.min_pages = st.ed.doc.min_pages.max(n + 1);
            st.ed.changed();
            toast(st, tr("Dodano pustą stronę – wstaw na nią obrazy, kształty i ramki"));
        }
        "image" => {
            let Some(p) = rfd::FileDialog::new().set_title(tr("Wstaw obraz")).add_filter(tr("Obrazy"), &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"]).pick_file() else { return };
            match std::fs::read(&p) {
                Ok(b) => insert_image_bytes(st, b),
                Err(e) => toast(st, &e.to_string()),
            }
            return;
        }
        "image-link" => {
            let Some(p) = rfd::FileDialog::new().set_title(tr("Wstaw obraz jako łącze do pliku")).add_filter(tr("Obrazy"), &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"]).pick_file() else { return };
            match io::odtp::link_file(&p) {
                Ok((asset, link)) => insert_image(st, asset, Some(link)),
                Err(e) => toast(st, &e),
            }
            return;
        }
        "image-embed" | "image-to-link" | "image-relink" => {
            // arg: klucz obrazu (z panelu Obrazy) albo pusty – obraz zaznaczonego obiektu.
            let key = if arg.is_empty() { selected_asset(st) } else { Some(arg.to_string()) };
            let Some(key) = key else {
                toast(st, tr("Zaznacz obraz"));
                return;
            };
            match base {
                "image-embed" => {
                    if st.ed.doc.missing.contains(&key) {
                        toast(st, tr("Brak pliku obrazu – najpierw wskaż go ponownie"));
                        return;
                    }
                    st.ed.begin(Kind::Other);
                    st.ed.doc.links.remove(&key);
                    st.ed.changed();
                    toast(st, tr("Obraz osadzony w dokumencie"));
                }
                "image-to-link" => {
                    let Some(a) = st.ed.doc.assets.get(&key).cloned() else { return };
                    let name = format!("{}.{}", files::doc_name(&st.ed), io::odtp::ext_of(&a.mime));
                    let mut dlg = rfd::FileDialog::new().set_title(tr("Zapisz obraz jako plik i połącz")).set_file_name(&name);
                    if let Some(dir) = st.ed.path.as_ref().and_then(|p| p.parent()) {
                        dlg = dlg.set_directory(dir);
                    }
                    let Some(p) = dlg.save_file() else { return };
                    if let Err(e) = std::fs::write(&p, a.data.as_slice()) {
                        toast(st, &e.to_string());
                        return;
                    }
                    match io::odtp::link_file(&p) {
                        Ok((asset, link)) => {
                            st.ed.begin(Kind::Other);
                            st.ed.doc.assets.insert(key.clone(), asset);
                            st.ed.doc.links.insert(key.clone(), link);
                            st.ed.changed();
                            toast(st, tr("Obraz jest teraz łączem do pliku – zmiany w pliku pojawią się w dokumencie"));
                        }
                        Err(e) => toast(st, &e),
                    }
                }
                _ => {
                    let Some(p) = rfd::FileDialog::new().set_title(tr("Wskaż plik obrazu")).add_filter(tr("Obrazy"), &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"]).pick_file() else { return };
                    match io::odtp::link_file(&p) {
                        Ok((asset, link)) => {
                            st.ed.begin(Kind::Other);
                            st.ed.doc.assets.insert(key.clone(), asset);
                            st.ed.doc.links.insert(key.clone(), link);
                            st.ed.doc.missing.remove(&key);
                            st.view.forget_image(&key);
                            st.ed.changed();
                            st.view.invalidate();
                        }
                        Err(e) => toast(st, &e),
                    }
                }
            }
        }
        "links-embed-all" => {
            let keys: Vec<String> = st.ed.doc.links.keys().filter(|k| !st.ed.doc.missing.contains(*k)).cloned().collect();
            if keys.is_empty() {
                toast(st, tr("Brak obrazów linkowanych do osadzenia"));
                return;
            }
            st.ed.begin(Kind::Other);
            for k in &keys {
                st.ed.doc.links.remove(k);
            }
            st.ed.changed();
            toast(st, &opendtp_core::trf!("Osadzono obrazy: {}", keys.len()));
        }
        "links-refresh" => {
            links_tick(st, true);
            return;
        }
        "package" => {
            if st.ed.doc.links.is_empty() {
                toast(st, tr("Dokument nie ma obrazów linkowanych – wystarczy zwykły zapis"));
            }
            let Some(dir) = rfd::FileDialog::new().set_title(tr("Spakuj: wybierz pusty folder na dokument i obrazy")).pick_folder() else { return };
            match io::odtp::package(&st.ed.doc, &dir, &files::doc_name(&st.ed)) {
                Ok(p) => toast(st, &opendtp_core::trf!("Spakowano do: {}", p.display())),
                Err(e) => toast(st, &e),
            }
            return;
        }
        "image-goto" => {
            let Ok(id) = arg.parse::<u64>() else { return };
            let Some((page, y)) = st.ed.doc.object(id).map(|o| (o.page, o.y)) else { return };
            st.ed.obj = Some(id);
            st.ed.multi.clear();
            let (_, cy) = st.view.to_canvas(page, 0.0, y);
            let vy = (cy - 60.0).max(0.0);
            app.set_view_y(-vy);
            st.view.vy = vy;
            object_selected(st);
        }
        "replace-image" => {
            // Obraz linkowany – wskazujemy nowy plik łącza (zostaje łączem).
            if let Some(key) = selected_asset(st).filter(|k| st.ed.doc.links.contains_key(k)) {
                return run(st, "image-relink", &key);
            }
            let Some(p) = rfd::FileDialog::new().set_title(tr("Zmień obraz")).add_filter(tr("Obrazy"), &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"]).pick_file() else { return };
            let Some(asset) = std::fs::read(&p).ok().and_then(io::odtp::asset_from) else {
                toast(st, tr("Nieobsługiwany format obrazu"));
                return;
            };
            let key = format!("img{}", st.ed.doc.new_id());
            st.ed.doc.assets.insert(key.clone(), asset);
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Image { asset, contour, .. } = &mut o.kind {
                    *asset = key.clone();
                    *contour = None;
                }
            });
        }
        "shape" | "text-shape" => {
            if arg == "pen" {
                return run(st, "draw-shape", "");
            }
            st.shape_preset = if arg.is_empty() { "rect".into() } else { arg.to_string() };
            st.text_shape = base == "text-shape";
            set_tool(st, 2);
            toast(st, tr("Przeciągnij na stronie, aby narysować (kliknięcie – rozmiar domyślny)"));
            return;
        }
        "text-frame" => {
            st.shape_preset = "rect".into();
            st.text_shape = true;
            set_tool(st, 1);
            toast(st, tr("Przeciągnij na stronie, aby narysować ramkę tekstową"));
            return;
        }
        "draw-shape" => {
            st.pen.clear();
            set_tool(st, 3);
            toast(st, tr("Klikaj, aby stawiać węzły (przeciągnij – krzywa). Kliknij pierwszy węzeł albo Enter, aby zamknąć."));
            return;
        }
        "path-text" => {
            set_tool(st, 4);
            toast(st, tr("Przeciągnij na stronie – powstanie łuk z tekstem"));
            return;
        }
        "chart" => {
            let kind = match arg {
                "bar" => ChartKind::Bar,
                "line" => ChartKind::Line,
                "area" => ChartKind::Area,
                "pie" => ChartKind::Pie,
                "doughnut" => ChartKind::Doughnut,
                _ => ChartKind::Column,
            };
            let page = st.ed.caret_page().map(|c| c.0).unwrap_or(0);
            let ps = st.ed.doc.page.clone();
            let mut ch = Chart { kind, title: tr("Tytuł wykresu").into(), ..Chart::default() };
            if matches!(kind, ChartKind::Pie | ChartKind::Doughnut) {
                ch.labels = vec![tr("Produkt A").into(), tr("Produkt B").into(), tr("Produkt C").into(), tr("Inne").into()];
                ch.series = vec![Series { name: tr("Udział").into(), values: vec![42.0, 28.0, 18.0, 12.0], color: None }];
            }
            let w = (ps.content_width() * 0.6).min(320.0);
            let y = st.ed.caret_page().map(|c| c.2).unwrap_or(ps.margin_top);
            let o = new_obj(page, ps.margin_left, y, w, w * 0.65, Shape::rect(), ObjKind::Chart { chart: ch }, WrapMode::Both);
            let id = st.ed.add_object(o);
            anchor_new(st, id);
            object_selected(st);
            app.set_right_panel("props".into());
        }
        "table" => {
            let page = st.ed.caret_page().map(|c| c.0).unwrap_or(0);
            let ps = st.ed.doc.page.clone();
            let y = st.ed.caret_page().map(|c| c.2).unwrap_or(ps.margin_top);
            let mut t = Table::default();
            t.rows = vec![
                vec![tr("Pozycja").into(), tr("Ilość").into(), tr("Cena").into(), tr("Razem").into()],
                vec![tr("Projekt").into(), "1".into(), "1200".into(), "1200".into()],
                vec![tr("Skład").into(), "12".into(), "80".into(), "960".into()],
                vec![tr("Druk").into(), "500".into(), "1,2".into(), "600".into()],
            ];
            if let Some(c) = st.ed.doc.style("heading1").and_then(|s| s.chars.color) {
                t.accent = c;
            }
            let w = ps.content_width();
            let h = t.rows.len() as f32 * 22.0;
            let o = new_obj(page, ps.margin_left, y, w, h, Shape::rect(), ObjKind::Table { table: t }, WrapMode::TopBottom);
            let id = st.ed.add_object(o);
            anchor_new(st, id);
            object_selected(st);
            app.set_right_panel("props".into());
        }
        "table-data" => {
            let rows: Vec<Vec<String>> = arg.lines().filter(|l| !l.trim().is_empty()).map(|l| l.split([';', '\t']).map(|c| c.trim().to_string()).collect()).collect();
            if rows.is_empty() {
                toast(st, tr("Tabela nie może być pusta"));
                return;
            }
            let n = rows.len() as f32;
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Table { table } = &mut o.kind {
                    // Wysokość wiersza zostaje taka jak była.
                    let rh = o.h / table.rows.len().max(1) as f32;
                    table.rows = rows.clone();
                    o.h = (rh * n).max(12.0);
                }
            });
        }
        "table-header" => st.ed.edit_obj(&|o| {
            if let ObjKind::Table { table } = &mut o.kind {
                table.header = !table.header;
            }
        }),
        "table-stripes" => st.ed.edit_obj(&|o| {
            if let ObjKind::Table { table } = &mut o.kind {
                table.stripes = !table.stripes;
            }
        }),
        "table-accent" => {
            let c = color_of(app.get_table_accent());
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Table { table } = &mut o.kind {
                    table.accent = c;
                }
            });
        }
        "anchor-toggle" => {
            let Some(id) = st.ed.obj else { return };
            st.ed.begin(Kind::Other);
            if st.ed.anchored(id) {
                st.ed.unanchor(id);
                toast(st, tr("Obiekt przypięty do strony"));
            } else if let Some(o) = st.ed.doc.object(id).cloned() {
                // Akapit, który zaczyna się najbliżej górnej krawędzi obiektu na jego stronie.
                let lay = &st.ed.layout;
                let para = lay.story_lines(StoryRef::Body).iter().map(|&i| &lay.lines[i]).filter(|l| l.page == o.page && l.first).min_by(|a, b| (a.top - o.y).abs().partial_cmp(&(b.top - o.y).abs()).unwrap()).map(|l| l.para).unwrap_or(0);
                st.ed.anchor_at(id, para);
                st.ed.relayout();
                st.ed.update_anchor_offset(id);
                toast(st, tr("Obiekt płynie teraz razem z tekstem"));
            }
            st.ed.changed();
        }
        "cell-commit" | "cell-next" | "cell-prev" => {
            if let Some((id, row, col)) = st.ed.cell {
                let text = arg.to_string();
                st.ed.obj = Some(id);
                st.ed.edit_table(&move |t| {
                    if let Some(c) = t.rows.get_mut(row).and_then(|r| r.get_mut(col)) {
                        *c = text.clone();
                    }
                });
                let next = match st.ed.doc.object(id).map(|o| &o.kind) {
                    Some(ObjKind::Table { table }) => {
                        let cols = table.cols();
                        let rows = table.rows.len();
                        let idx = row * cols + col;
                        if base == "cell-next" {
                            if idx + 1 >= rows * cols {
                                // Tab w ostatniej komórce – nowy wiersz.
                                st.ed.edit_table(&move |t| t.rows.push(vec![String::new(); cols]));
                            }
                            Some(((idx + 1) / cols, (idx + 1) % cols))
                        } else if base == "cell-prev" {
                            idx.checked_sub(1).map(|i| (i / cols, i % cols))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                st.ed.cell = next.map(|(r, c)| (id, r, c));
                show_cell(st);
            }
        }
        "cell-cancel" => {
            st.ed.cell = None;
            show_cell(st);
        }
        "tbl-row-above" | "tbl-row-below" | "tbl-col-left" | "tbl-col-right" | "tbl-row-del" | "tbl-col-del" | "tbl-merge-right" | "tbl-split" => {
            let Some((_, row, col)) = st.ctx_cell else { return };
            let op = base.to_string();
            st.ed.edit_table(&move |t| {
                let cols = t.cols();
                match op.as_str() {
                    "tbl-row-above" => t.rows.insert(row, vec![String::new(); cols]),
                    "tbl-row-below" => t.rows.insert((row + 1).min(t.rows.len()), vec![String::new(); cols]),
                    "tbl-col-left" | "tbl-col-right" => {
                        let at = if op == "tbl-col-left" { col } else { col + 1 };
                        for r in t.rows.iter_mut() {
                            r.insert(at.min(r.len()), String::new());
                        }
                        t.widths.clear();
                    }
                    "tbl-row-del" if t.rows.len() > 1 => {
                        t.rows.remove(row);
                    }
                    "tbl-col-del" if cols > 1 => {
                        for r in t.rows.iter_mut() {
                            if col < r.len() {
                                r.remove(col);
                            }
                        }
                        t.widths.clear();
                    }
                    "tbl-merge-right" => {
                        if let Some(r) = t.rows.get_mut(row) {
                            let mut c = col + 1;
                            while c < r.len() && r[c] == MERGED {
                                c += 1;
                            }
                            if c < r.len() {
                                let extra = std::mem::replace(&mut r[c], MERGED.to_string());
                                if !extra.trim().is_empty() {
                                    let v = format!("{} {}", r[col], extra);
                                    r[col] = v.trim().to_string();
                                }
                            }
                        }
                    }
                    "tbl-split" => {
                        if let Some(r) = t.rows.get_mut(row) {
                            let mut c = col + 1;
                            while c < r.len() && r[c] == MERGED {
                                r[c] = String::new();
                                c += 1;
                            }
                        }
                    }
                    _ => {}
                }
            });
        }
        "hyphenate" => {
            st.ed.begin(Kind::Other);
            st.ed.doc.settings.hyphenate = !st.ed.doc.settings.hyphenate;
            st.ed.changed();
        }
        "ruler-toggle" => {
            st.prefs.ruler = !st.prefs.ruler;
            st.prefs.save();
        }
        "ruler" => {
            let mut it = arg.split(';');
            let kind = it.next().unwrap_or("").to_string();
            let v1: f32 = it.next().and_then(|v| v.replace(',', ".").parse().ok()).unwrap_or(0.0);
            let v2: f32 = it.next().and_then(|v| v.replace(',', ".").parse().ok()).unwrap_or(0.0);
            let (_, pp, _) = st.ed.caret_props();
            let list = pp.list.map(|lf| (lf.level as f32 + 1.0) * 18.0).unwrap_or(0.0);
            let round = |v: f32| (v * 2.0).round() / 2.0;
            match kind.as_str() {
                "left" => {
                    let v = round((v1 - list).max(0.0));
                    st.ed.format_paras(&move |p| p.fmt.indent_left = Some(v));
                }
                "first" => {
                    let v = round(v1);
                    st.ed.format_paras(&move |p| p.fmt.first_line = Some(v));
                }
                "right" => {
                    let v = round(v1.max(0.0));
                    st.ed.format_paras(&move |p| p.fmt.indent_right = Some(v));
                }
                "tab+" | "tab-" | "tabmv" => {
                    let mut tabs = pp.tabs.clone();
                    match kind.as_str() {
                        "tab+" if v1 > 2.0 => tabs.push(round(v1)),
                        "tab-" => tabs.retain(|t| (t - v1).abs() > 0.6),
                        "tabmv" => {
                            tabs.retain(|t| (t - v1).abs() > 0.6);
                            if v2 > 2.0 {
                                tabs.push(round(v2));
                            }
                        }
                        _ => {}
                    }
                    tabs.sort_by(|x, y| x.partial_cmp(y).unwrap());
                    tabs.dedup_by(|x, y| (*x - *y).abs() < 0.6);
                    st.ed.format_paras(&move |p| p.fmt.tabs = Some(tabs.clone()));
                }
                _ => {}
            }
        }
        "font-dialog" => {
            let (cp, _, _) = st.ed.caret_props();
            app.set_f_font(cp.font.clone().into());
            app.set_f_size(num(cp.size).into());
            app.set_f_bold(cp.bold);
            app.set_f_italic(cp.italic);
            app.set_f_underline(match cp.underline {
                Underline::None => 0,
                Underline::Single => 1,
                Underline::Double => 2,
                Underline::Dotted => 3,
                Underline::Wave => 4,
            });
            app.set_f_strike(cp.strike);
            app.set_f_caps(match cp.caps {
                Caps::None => 0,
                Caps::All => 1,
                Caps::Small => 2,
            });
            app.set_f_script(match cp.script {
                Script::Normal => 0,
                Script::Super => 1,
                Script::Sub => 2,
            });
            app.set_f_color(cp.color.hex().into());
            app.set_f_highlight(if cp.highlight.is_none() { String::new() } else { cp.highlight.hex() }.into());
            app.set_f_tracking(num(cp.tracking).into());
            app.set_f_scale(num(cp.scale).into());
            app.set_f_baseline(num(cp.baseline).into());
            app.set_f_kerning(cp.kerning);
            app.set_f_ligatures(cp.ligatures);
            let feats: Vec<String> = cp.features.split([',', ' ']).map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect();
            let has = |f: &str| feats.iter().any(|x| x == f);
            app.set_f_onum(has("onum"));
            app.set_f_lnum(has("lnum"));
            app.set_f_tnum(has("tnum"));
            app.set_f_smcp(has("smcp"));
            app.set_f_frac(has("frac"));
            app.set_f_ss(feats.iter().filter(|f| !["onum", "lnum", "tnum", "smcp", "frac"].contains(&f.as_str())).cloned().collect::<Vec<_>>().join(", ").into());
            app.set_f_lang(if cp.lang.starts_with("en") { 1 } else { 0 });
            app.set_dialog("font".into());
            return;
        }
        "f-step" => {
            let (k, d) = arg.split_once(';').unwrap_or(("", "0"));
            let d: f32 = d.parse().unwrap_or(0.0);
            match k {
                "tracking" => app.set_f_tracking(num(parse_num(&app.get_f_tracking()).unwrap_or(0.0) + d * 0.5).into()),
                "scale" => app.set_f_scale(num((parse_num(&app.get_f_scale()).unwrap_or(100.0) + d * 5.0).clamp(10.0, 400.0)).into()),
                _ => app.set_f_baseline(num(parse_num(&app.get_f_baseline()).unwrap_or(0.0) + d * 0.5).into()),
            }
            return;
        }
        "font-apply" => {
            let font = app.get_f_font().to_string();
            let size = parse_num(&app.get_f_size());
            let (bold, italic, strike) = (app.get_f_bold(), app.get_f_italic(), app.get_f_strike());
            let underline = match app.get_f_underline() {
                1 => Underline::Single,
                2 => Underline::Double,
                3 => Underline::Dotted,
                4 => Underline::Wave,
                _ => Underline::None,
            };
            let caps = match app.get_f_caps() {
                1 => Caps::All,
                2 => Caps::Small,
                _ => Caps::None,
            };
            let script = match app.get_f_script() {
                1 => Script::Super,
                2 => Script::Sub,
                _ => Script::Normal,
            };
            let color = Color::parse(&app.get_f_color());
            let hl = app.get_f_highlight().to_string();
            let highlight = if hl.trim().is_empty() { Some(Color::NONE) } else { Color::parse(&hl) };
            let tracking = parse_num(&app.get_f_tracking());
            let scale = parse_num(&app.get_f_scale());
            let baseline = parse_num(&app.get_f_baseline());
            let (kern, liga) = (app.get_f_kerning(), app.get_f_ligatures());
            let mut feats: Vec<String> = Vec::new();
            for (on, f) in [(app.get_f_onum(), "onum"), (app.get_f_lnum(), "lnum"), (app.get_f_tnum(), "tnum"), (app.get_f_smcp(), "smcp"), (app.get_f_frac(), "frac")] {
                if on {
                    feats.push(f.into());
                }
            }
            feats.extend(app.get_f_ss().split([',', ' ']).map(|f| f.trim().to_string()).filter(|f| !f.is_empty()));
            let features = feats.join(",");
            let lang = if app.get_f_lang() == 1 { "en" } else { "pl" }.to_string();
            if !font.trim().is_empty() {
                st.prefs.add_recent_font(font.trim());
            }
            st.ed.format(&move |f| {
                if !font.trim().is_empty() {
                    f.font = Some(font.trim().to_string());
                }
                if let Some(v) = size {
                    f.size = Some(v.clamp(1.0, 1600.0));
                }
                f.bold = Some(bold);
                f.italic = Some(italic);
                f.strike = Some(strike);
                f.underline = Some(underline);
                f.caps = Some(caps);
                f.script = Some(script);
                if color.is_some() {
                    f.color = color;
                }
                if highlight.is_some() {
                    f.highlight = highlight;
                }
                f.tracking = tracking.or(f.tracking);
                f.scale = scale.or(f.scale);
                f.baseline = baseline.or(f.baseline);
                f.kerning = Some(kern);
                f.ligatures = Some(liga);
                f.features = Some(features.clone());
                f.lang = Some(lang.clone());
            });
            app.set_dialog("".into());
            st.view.invalidate();
        }
        "para-dialog" => {
            let (_, pp, _) = st.ed.caret_props();
            app.set_p_align(match pp.align {
                Align::Left => 0,
                Align::Center => 1,
                Align::Right => 2,
                Align::Justify => 3,
            });
            app.set_p_left(len_str(pp.indent_left, unit).into());
            app.set_p_right(len_str(pp.indent_right, unit).into());
            app.set_p_first(len_str(pp.first_line, unit).into());
            app.set_p_before(num(pp.space_before).into());
            app.set_p_after(num(pp.space_after).into());
            let (kind, v) = match pp.line {
                LineSpacing::Multiple(k) => (0, k),
                LineSpacing::Exact(v) => (1, v),
                LineSpacing::AtLeast(v) => (2, v),
            };
            app.set_p_line_kind(kind);
            app.set_p_line(num(v).into());
            app.set_p_keep_next(pp.keep_next);
            app.set_p_keep_lines(pp.keep_lines);
            app.set_p_widows(pp.widows);
            app.set_p_page_before(pp.break_before == Break::Page);
            app.set_p_hyphen(pp.hyphenate);
            app.set_p_shading(if pp.shading.is_none() { String::new() } else { pp.shading.hex() }.into());
            let b = pp.border.unwrap_or(BoxBorder { width: 0.0, color: Color::BLACK, radius: 0.0, padding: 4.0 });
            app.set_p_border_w(num(b.width).into());
            app.set_p_border_color(b.color.hex().into());
            app.set_p_radius(num(b.radius).into());
            app.set_p_padding(num(b.padding).into());
            app.set_p_dropcap(pp.drop_cap as i32);
            app.set_p_tabs(pp.tabs.iter().map(|t| num(*t)).collect::<Vec<_>>().join("; ").into());
            app.set_dialog("para".into());
            return;
        }
        "p-step" => {
            let (k, d) = arg.split_once(';').unwrap_or(("", "0"));
            let dv: f32 = d.replace(',', ".").parse().unwrap_or(0.0);
            let lstep = if unit == 0 { 1.0 } else if unit == 3 { 2.0 } else { 0.1 };
            let bump = |s: slint::SharedString, step: f32| num(parse_num(&s).unwrap_or(0.0) + dv * step);
            match k {
                "left" => app.set_p_left(bump(app.get_p_left(), lstep).into()),
                "right" => app.set_p_right(bump(app.get_p_right(), lstep).into()),
                "first" => app.set_p_first(bump(app.get_p_first(), lstep).into()),
                "before" => app.set_p_before(bump(app.get_p_before(), 2.0).into()),
                "after" => app.set_p_after(bump(app.get_p_after(), 2.0).into()),
                "dropcap" => app.set_p_dropcap((app.get_p_dropcap() + dv as i32).clamp(0, 6)),
                "dropcap-set" => app.set_p_dropcap((dv as i32).clamp(0, 6)),
                _ => {}
            }
            return;
        }
        "para-apply" => {
            let align = match app.get_p_align() {
                1 => Align::Center,
                2 => Align::Right,
                3 => Align::Justify,
                _ => Align::Left,
            };
            let l = parse_num(&app.get_p_left()).map(|v| v * uk);
            let r = parse_num(&app.get_p_right()).map(|v| v * uk);
            let fl = parse_num(&app.get_p_first()).map(|v| v * uk);
            let before = parse_num(&app.get_p_before());
            let after = parse_num(&app.get_p_after());
            let lv = parse_num(&app.get_p_line()).unwrap_or(1.15);
            let line = match app.get_p_line_kind() {
                1 => LineSpacing::Exact(lv.max(1.0)),
                2 => LineSpacing::AtLeast(lv.max(1.0)),
                _ => LineSpacing::Multiple(lv.clamp(0.5, 5.0)),
            };
            let (kn, kl, wd, pb, hy) = (app.get_p_keep_next(), app.get_p_keep_lines(), app.get_p_widows(), app.get_p_page_before(), app.get_p_hyphen());
            let sh = app.get_p_shading().to_string();
            let shading = if sh.trim().is_empty() { Color::NONE } else { Color::parse(&sh).unwrap_or(Color::NONE) };
            let bw = parse_num(&app.get_p_border_w()).unwrap_or(0.0);
            let border = BoxBorder {
                width: bw.max(0.0),
                color: Color::parse(&app.get_p_border_color()).unwrap_or(Color::BLACK),
                radius: parse_num(&app.get_p_radius()).unwrap_or(0.0).max(0.0),
                padding: parse_num(&app.get_p_padding()).unwrap_or(4.0).max(0.0),
            };
            let has_border = bw > 0.0 || border.radius > 0.0 || !shading.is_none();
            let drop = app.get_p_dropcap().clamp(0, 6) as u8;
            let tabs: Vec<f32> = app.get_p_tabs().split([';', ' ']).filter_map(parse_num).collect();
            st.ed.format_paras(&move |p| {
                p.fmt.align = Some(align);
                p.fmt.indent_left = l.or(p.fmt.indent_left);
                p.fmt.indent_right = r.or(p.fmt.indent_right);
                p.fmt.first_line = fl.or(p.fmt.first_line);
                p.fmt.space_before = before.or(p.fmt.space_before);
                p.fmt.space_after = after.or(p.fmt.space_after);
                p.fmt.line = Some(line);
                p.fmt.keep_next = Some(kn);
                p.fmt.keep_lines = Some(kl);
                p.fmt.widows = Some(wd);
                p.fmt.break_before = Some(if pb { Break::Page } else { Break::None });
                p.fmt.hyphenate = Some(hy);
                p.fmt.shading = Some(shading);
                p.fmt.border = Some(if has_border { Some(border) } else { None });
                p.fmt.drop_cap = Some(if drop >= 2 { drop } else { 0 });
                p.fmt.tabs = Some(tabs.clone());
            });
            app.set_dialog("".into());
        }
        "watermark" => {
            st.ed.begin(Kind::Other);
            st.ed.doc.page.watermark = arg.trim().to_string();
            st.ed.changed();
            app.set_watermark(arg.trim().into());
        }
        "page-border" => {
            st.ed.begin(Kind::Other);
            let p = &mut st.ed.doc.page;
            p.border = if p.border.is_some() { None } else { Some(StrokeStyle { color: Color::rgb(0x40, 0x40, 0x40), width: 1.5, dash: vec![] }) };
            st.ed.changed();
        }
        "open-url" => {
            let _ = open::that(arg);
            return;
        }
        "symbol" => {
            st.ed.insert(arg);
            scroll = true;
        }
        "field" => {
            let f = match arg {
                "page-number" => Some(Field::PageNumber),
                "page-count" => Some(Field::PageCount),
                "date" => Some(Field::Date),
                "time" => Some(Field::Time),
                "title" => Some(Field::Title),
                "author" => Some(Field::Author),
                "running-1" => Some(Field::Running(1)),
                "running-2" => Some(Field::Running(2)),
                _ => None,
            };
            match f {
                Some(f) => {
                    st.ed.begin(Kind::Other);
                    let fmt = st.ed.caret_fmt();
                    let h = st.ed.head;
                    let story = st.ed.story;
                    if let Some(s) = st.ed.doc.story_mut(story) {
                        let end = opendtp_core::edit::insert_field(s, h, f, &fmt);
                        st.ed.anchor = end;
                        st.ed.head = end;
                    }
                    st.ed.changed();
                }
                None => st.ed.insert(&opendtp_core::util::today()),
            }
        }
        "header-edit" | "footer-edit" => {
            let header = base == "header-edit";
            st.ed.begin(Kind::Other);
            let style = if header { "header" } else { "footer" };
            let slot = if header { &mut st.ed.doc.header } else { &mut st.ed.doc.footer };
            if slot.is_none() {
                let mut p = Paragraph::with_text(style, "");
                if !header {
                    p.runs = vec![Run { text: FIELD_CHAR.to_string(), field: Some(Field::PageNumber), ..Default::default() }];
                }
                *slot = Some(Story { paras: vec![p] });
            }
            st.ed.changed();
            let r = if header { StoryRef::Header } else { StoryRef::Footer };
            let len = st.ed.doc.story(r).map(|s| s.paras[0].len()).unwrap_or(0);
            st.ed.obj = None;
            st.ed.set_caret(r, At::new(0, len), false);
            let page = st.ed.caret_page().map(|c| c.0).unwrap_or(0);
            let _ = page;
            scroll = true;
            toast(st, tr("Edytujesz nagłówek/stopkę – Esc wraca do treści"));
        }
        "header-remove" | "footer-remove" => {
            st.ed.begin(Kind::Other);
            if base == "header-remove" {
                st.ed.doc.header = None;
                st.ed.doc.header_first = None;
                st.ed.doc.header_even = None;
            } else {
                st.ed.doc.footer = None;
                st.ed.doc.footer_first = None;
                st.ed.doc.footer_even = None;
            }
            if st.ed.story != StoryRef::Body && !matches!(st.ed.story, StoryRef::Obj(_)) {
                st.ed.story = StoryRef::Body;
            }
            st.ed.changed();
        }
        "diff-first" => {
            st.ed.begin(Kind::Other);
            st.ed.doc.page.different_first = !st.ed.doc.page.different_first;
            if st.ed.doc.page.different_first {
                if st.ed.doc.header.is_some() && st.ed.doc.header_first.is_none() {
                    st.ed.doc.header_first = Some(Story { paras: vec![Paragraph::with_text("header", "")] });
                }
                if st.ed.doc.footer.is_some() && st.ed.doc.footer_first.is_none() {
                    st.ed.doc.footer_first = Some(Story { paras: vec![Paragraph::with_text("footer", "")] });
                }
            }
            st.ed.changed();
        }
        "toc" | "toc-update" => {
            let paras = opendtp_core::layout::toc(&st.ed.doc, &st.ed.layout);
            if paras.is_empty() {
                toast(st, tr("Brak nagłówków – nadaj akapitom style „Nagłówek 1–3”"));
                return;
            }
            if base == "toc-update" {
                // Zastępujemy istniejące akapity spisu.
                let body = &st.ed.doc.body.paras;
                let first = body.iter().position(|p| p.style.starts_with("toc"));
                if let Some(f) = first {
                    st.ed.begin(Kind::Other);
                    let mut last = f;
                    while last + 1 < st.ed.doc.body.paras.len() && st.ed.doc.body.paras[last + 1].style.starts_with("toc") {
                        last += 1;
                    }
                    st.ed.doc.body.paras.splice(f..=last, paras);
                    st.ed.changed();
                    // Druga runda: numery stron po zmianie długości spisu.
                    let again = opendtp_core::layout::toc(&st.ed.doc, &st.ed.layout);
                    let n = again.len();
                    st.ed.doc.body.paras.splice(f..f + n, again);
                    st.ed.changed();
                    toast(st, tr("Spis treści zaktualizowany"));
                } else {
                    toast(st, tr("W dokumencie nie ma spisu treści"));
                }
            } else {
                if st.ed.story != StoryRef::Body {
                    st.ed.set_caret(StoryRef::Body, st.ed.head, false);
                }
                st.ed.insert_paragraphs_at_caret(paras);
            }
        }
        "caption" => {
            // Podpis z polem numeru – numery przeliczają się same po przestawieniu rysunków.
            let kind = SeqKind::from_id(arg).unwrap_or(SeqKind::Figure);
            if st.ed.story != StoryRef::Body {
                st.ed.set_caret(StoryRef::Body, st.ed.head, false);
            }
            st.ed.enter();
            st.ed.set_style("caption");
            st.ed.insert(&format!("{} ", kind.label()));
            let id = st.ed.doc.new_id();
            insert_field_at_caret(st, Field::Seq(id, kind));
            st.ed.insert(". ");
            scroll = true;
        }
        "bookmark" => {
            if st.ed.story != StoryRef::Body {
                toast(st, tr("Zakładki i hasła indeksu wstawia się w treści głównej"));
                return;
            }
            let sel: String = st.ed.fragment().iter().map(|p| p.text().replace(FIELD_CHAR, "")).collect::<Vec<_>>().join(" ");
            let name = if sel.trim().is_empty() { st.ed.para_text(st.ed.head.para).replace(FIELD_CHAR, "") } else { sel };
            let mut name: String = name.trim().chars().take(60).collect();
            if name.is_empty() {
                name = opendtp_core::trf!("Zakładka {}", st.ed.doc.bookmarks.len() + 1);
            }
            st.ed.begin(Kind::Other);
            let id = st.ed.doc.new_id();
            st.ed.doc.bookmarks.push(Bookmark { id, name: name.clone() });
            let (a, _) = st.ed.range();
            st.ed.anchor = a;
            st.ed.head = a;
            insert_field_at_caret(st, Field::Bookmark(id));
            toast(st, &opendtp_core::trf!("Dodano zakładkę „{}”", name));
        }
        "xref" => {
            // arg: „id:page|number|text”
            let (id, kind) = arg.split_once(':').unwrap_or((arg, "page"));
            let Ok(id) = id.parse::<u64>() else { return };
            let kind = match kind {
                "number" => RefKind::Number,
                "text" => RefKind::Text,
                _ => RefKind::Page,
            };
            insert_field_at_caret(st, Field::Ref(id, kind));
            scroll = true;
        }
        "index-mark" => {
            if st.ed.story != StoryRef::Body {
                toast(st, tr("Zakładki i hasła indeksu wstawia się w treści głównej"));
                return;
            }
            let sel: String = st.ed.fragment().iter().map(|p| p.text().replace(FIELD_CHAR, "")).collect::<Vec<_>>().join(" ");
            let sel = sel.trim().to_string();
            if sel.is_empty() {
                toast(st, tr("Zaznacz wyraz albo wyrażenie, które ma trafić do indeksu"));
                return;
            }
            // „hasło: podhasło” zapisuje podhasło.
            let (term, sub) = match sel.split_once(':') {
                Some((t, s)) if !t.trim().is_empty() && !s.trim().is_empty() => (t.trim().to_string(), s.trim().to_string()),
                _ => (sel.clone(), String::new()),
            };
            st.ed.begin(Kind::Other);
            let id = st.ed.doc.new_id();
            st.ed.doc.index.push(IndexEntry { id, term: term.clone(), sub });
            let (_, b) = st.ed.range();
            st.ed.anchor = b;
            st.ed.head = b;
            insert_field_at_caret(st, Field::IndexMark(id));
            toast(st, &opendtp_core::trf!("Hasło „{}” dodane do indeksu", term));
        }
        "index-mark-all" => {
            // Oznacza każde wystąpienie zaznaczonego wyrazu w treści głównej.
            let sel: String = st.ed.fragment().iter().map(|p| p.text().replace(FIELD_CHAR, "")).collect::<Vec<_>>().join(" ");
            let term = sel.trim().to_string();
            if term.is_empty() || st.ed.story != StoryRef::Body {
                toast(st, tr("Zaznacz wyraz albo wyrażenie, które ma trafić do indeksu"));
                return;
            }
            st.ed.begin(Kind::Other);
            let mut spots = Vec::new();
            for (pi, p) in st.ed.doc.body.paras.iter().enumerate() {
                if p.style.starts_with("index") || p.style.starts_with("toc") {
                    continue;
                }
                let text = p.text();
                let mut from = 0;
                while let Some(i) = text[from..].find(term.as_str()) {
                    let end = from + i + term.len();
                    let word_end = text[end..].chars().next().is_none_or(|c| !c.is_alphanumeric());
                    let word_start = text[..from + i].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
                    if word_start && word_end {
                        spots.push(At::new(pi, end));
                    }
                    from = end;
                }
            }
            // Od końca, żeby wcześniejsze położenia się nie przesuwały.
            let n = spots.len();
            for at in spots.into_iter().rev() {
                let id = st.ed.doc.new_id();
                st.ed.doc.index.push(IndexEntry { id, term: term.clone(), sub: String::new() });
                opendtp_core::edit::insert_field(&mut st.ed.doc.body, at, Field::IndexMark(id), &CharFmt::default());
            }
            st.ed.fix_selection();
            st.ed.changed();
            toast(st, &opendtp_core::trf!("Oznaczono wystąpienia: {}", n));
        }
        "index" | "index-update" => {
            let paras = opendtp_core::layout::index(&st.ed.doc, &st.ed.layout);
            if paras.is_empty() {
                toast(st, tr("Brak haseł – zaznacz wyraz i wybierz „Oznacz hasło”"));
                return;
            }
            // Starsze dokumenty nie mają stylów indeksu – dokładamy brakujące.
            for s in styles::defaults().into_iter().filter(|s| s.id.starts_with("index")) {
                if st.ed.doc.style(&s.id).is_none() {
                    st.ed.doc.styles.push(s);
                }
            }
            let is_idx = |p: &Paragraph| p.style.starts_with("index");
            let first = st.ed.doc.body.paras.iter().position(is_idx);
            if base == "index-update" || first.is_some() {
                let Some(f) = first else {
                    toast(st, tr("W dokumencie nie ma indeksu"));
                    return;
                };
                st.ed.begin(Kind::Other);
                let mut last = f;
                while last + 1 < st.ed.doc.body.paras.len() && is_idx(&st.ed.doc.body.paras[last + 1]) {
                    last += 1;
                }
                st.ed.doc.body.paras.splice(f..=last, paras);
                st.ed.fix_selection();
                st.ed.changed();
                toast(st, tr("Indeks zaktualizowany"));
            } else {
                if st.ed.story != StoryRef::Body {
                    st.ed.set_caret(StoryRef::Body, st.ed.head, false);
                }
                st.ed.insert_paragraphs_at_caret(paras);
            }
        }
        "odd-even" => {
            st.ed.begin(Kind::Other);
            let on = !st.ed.doc.page.odd_even;
            st.ed.doc.page.odd_even = on;
            if on {
                // Strony parzyste zaczynają od kopii nieparzystych z wyrównaniem lustrzanym.
                // Nagłówek bez własnego wyrównania przechodzi na prawo; stopka (zwykle wyśrodkowana) zostaje.
                let mirror = |s: &Story, header: bool| {
                    let mut s = s.clone();
                    for p in s.paras.iter_mut() {
                        p.fmt.align = match p.fmt.align {
                            Some(Align::Right) => Some(Align::Left),
                            Some(Align::Left) => Some(Align::Right),
                            None if header => Some(Align::Right),
                            a => a,
                        };
                    }
                    s
                };
                if st.ed.doc.header_even.is_none() {
                    st.ed.doc.header_even = st.ed.doc.header.as_ref().map(|h| mirror(h, true));
                }
                if st.ed.doc.footer_even.is_none() {
                    st.ed.doc.footer_even = st.ed.doc.footer.as_ref().map(|f| mirror(f, false));
                }
            }
            st.ed.changed();
        }
        "running-head" => {
            // Nagłówek strony z żywą paginą (tytuł rozdziału) – tworzony w razie potrzeby.
            let level: u8 = arg.parse().unwrap_or(1);
            st.ed.begin(Kind::Other);
            if st.ed.doc.header.is_none() {
                st.ed.doc.header = Some(Story { paras: vec![Paragraph::with_text("header", "")] });
            }
            st.ed.changed();
            let r = if matches!(st.ed.story, StoryRef::Header | StoryRef::HeaderFirst | StoryRef::HeaderEven) { st.ed.story } else { StoryRef::Header };
            if r != st.ed.story {
                let len = st.ed.doc.story(r).map(|s| s.paras[0].len()).unwrap_or(0);
                st.ed.obj = None;
                st.ed.set_caret(r, At::new(0, len), false);
            }
            insert_field_at_caret(st, Field::Running(level.clamp(1, 3)));
            scroll = true;
        }
        "repeat" => {
            let r = match arg {
                "all" => Repeat::All,
                "odd" => Repeat::Odd,
                "even" => Repeat::Even,
                _ => Repeat::None,
            };
            if let Some(o) = st.ed.object() {
                if r != Repeat::None && matches!(o.kind, ObjKind::Text { .. } | ObjKind::PathText { .. } | ObjKind::Table { .. }) {
                    toast(st, tr("Na wszystkich stronach można powtarzać obrazy, kształty i wykresy"));
                    return;
                }
            }
            st.ed.edit_obj(&move |o| o.repeat = r);
        }

        // ---------------------------------------------------------- projekt i strona
        "doc-theme" => {
            if let Some(t) = styles::THEMES.iter().find(|t| t.id == arg) {
                st.ed.begin(Kind::Other);
                styles::apply_theme(&mut st.ed.doc, t);
                st.doc_theme = t.id.into();
                st.ed.changed();
            }
        }
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
                return;
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
                return;
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
            scroll = true;
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
                scroll = true;
            }
        }

        // ---------------------------------------------------------- obiekty
        "wrap" => {
            let m = wrap_mode(arg.parse().unwrap_or(0));
            if st.ed.obj.is_none() {
                toast(st, tr("Najpierw zaznacz obiekt"));
                return;
            }
            st.ed.edit_obj(&move |o| o.wrap.mode = m);
        }
        "wrap-dist" => {
            let cur = st.ed.object().map(|o| o.wrap.dist).unwrap_or(9.0);
            if let Some(v) = value_or_step(arg, cur, 2.0, is_step) {
                st.ed.edit_obj(&move |o| o.wrap.dist = v.max(0.0));
            }
        }
        "wrap-contour" => st.ed.edit_obj(&|o| o.wrap.contour = !o.wrap.contour),
        "detect-contour" => {
            let Some(o) = st.ed.object().cloned() else { return };
            let ObjKind::Image { asset, .. } = &o.kind else {
                toast(st, tr("Kontur wykrywamy dla obrazów"));
                return;
            };
            let Some(a) = st.ed.doc.assets.get(asset) else { return };
            let found = image::load_from_memory(&a.data).ok().map(|i| i.to_rgba8()).and_then(|img| {
                let (w, h) = img.dimensions();
                geom::image_contour(img.as_raw(), w, h)
            });
            match found {
                Some(c) => {
                    st.ed.edit_obj(&move |o| {
                        if let ObjKind::Image { contour, .. } = &mut o.kind {
                            *contour = Some(c.clone());
                        }
                        o.wrap.contour = true;
                        if !o.wrap.mode.excludes() {
                            o.wrap.mode = WrapMode::Both;
                        }
                    });
                    toast(st, tr("Wykryto kontur – tekst opływa kształt obrazu"));
                }
                None => toast(st, tr("Nie znaleziono konturu (obraz bez przezroczystości i jednolitego tła)")),
            }
        }
        "z-front" => st.ed.z_order("front"),
        "z-back" => st.ed.z_order("back"),
        "z-forward" => st.ed.z_order("forward"),
        "z-backward" => st.ed.z_order("backward"),
        "align-obj" => st.ed.align_obj(arg),
        "duplicate" => {
            if st.ed.obj.is_some() {
                st.ed.duplicate();
            }
        }
        "delete-obj" => st.ed.delete_object(),
        "lock" => st.ed.edit_obj(&|o| o.locked = !o.locked),
        "flip-h" => st.ed.flip(false),
        "flip-v" => st.ed.flip(true),
        "obj-x" | "obj-y" | "obj-w" | "obj-h" => {
            let Some(o) = st.ed.object().cloned() else { return };
            let cur = match base {
                "obj-x" => o.x,
                "obj-y" => o.y,
                "obj-w" => o.w,
                _ => o.h,
            } / uk;
            let step = if unit == 0 { 1.0 } else if unit == 3 { 2.0 } else { 0.1 };
            if let Some(v) = value_or_step(arg, cur, step, is_step) {
                let v = v * uk;
                let b = base.to_string();
                st.ed.edit_obj(&move |o| match b.as_str() {
                    "obj-x" => o.x = v,
                    "obj-y" => o.y = v,
                    "obj-w" => o.w = v.max(2.0),
                    _ => o.h = v.max(2.0),
                });
            }
        }
        "obj-name" => {
            let n = arg.to_string();
            st.ed.edit_obj(&move |o| o.name = n.clone());
        }
        "fill" => {
            let c = color_of(app.get_fill());
            let kind = app.get_fill_kind();
            let c2 = color_of(app.get_fill2());
            st.ed.edit_obj(&move |o| {
                o.fill = if c.is_none() {
                    None
                } else {
                    Some(match kind {
                        1 => Fill::Linear { from: c, to: c2, angle: 90.0 },
                        2 => Fill::Radial { from: c, to: c2 },
                        _ => Fill::Solid { color: c },
                    })
                }
            });
        }
        "fill-kind" => {
            let k: i32 = arg.parse().unwrap_or(0);
            st.ed.edit_obj(&move |o| {
                let base_c = match &o.fill {
                    Some(Fill::Solid { color }) => *color,
                    Some(Fill::Linear { from, .. }) | Some(Fill::Radial { from, .. }) => *from,
                    None => Color::rgb(0x5b, 0x8c, 0xff),
                };
                let second = Color::rgb((base_c.r as u16 * 6 / 10) as u8, (base_c.g as u16 * 6 / 10) as u8, (base_c.b as u16 * 6 / 10) as u8);
                o.fill = Some(match k {
                    1 => Fill::Linear { from: base_c, to: second, angle: 90.0 },
                    2 => Fill::Radial { from: Color::WHITE, to: base_c },
                    _ => Fill::Solid { color: base_c },
                });
            });
        }
        "stroke" => {
            let c = color_of(app.get_stroke());
            st.ed.edit_obj(&move |o| {
                if c.is_none() {
                    o.stroke = None;
                } else {
                    let w = o.stroke.as_ref().map(|s| s.width).filter(|w| *w > 0.0).unwrap_or(1.0);
                    let dash = o.stroke.as_ref().map(|s| s.dash.clone()).unwrap_or_default();
                    o.stroke = Some(StrokeStyle { color: c, width: w, dash });
                }
            });
        }
        "stroke-w" => {
            let cur = st.ed.object().and_then(|o| o.stroke.as_ref().map(|s| s.width)).unwrap_or(0.0);
            if let Some(v) = value_or_step(arg, cur, 0.5, is_step) {
                let v = v.max(0.0);
                st.ed.edit_obj(&move |o| match &mut o.stroke {
                    Some(s) => s.width = v,
                    None if v > 0.0 => o.stroke = Some(StrokeStyle { color: Color::BLACK, width: v, dash: vec![] }),
                    None => {}
                });
            }
        }
        "dashed" => st.ed.edit_obj(&|o| {
            if let Some(s) = &mut o.stroke {
                s.dash = if s.dash.len() >= 2 { vec![] } else { vec![s.width * 3.0, s.width * 2.0] };
            } else {
                o.stroke = Some(StrokeStyle { color: Color::BLACK, width: 1.0, dash: vec![3.0, 2.0] });
            }
        }),
        "shadow" => st.ed.edit_obj(&|o| o.shadow = !o.shadow),
        "opacity" => {
            let v: f32 = arg.parse().unwrap_or(1.0);
            let id = st.ed.obj;
            if let Some(id) = id {
                if !matches!(st.drag, crate::input::Drag::None) {
                    return;
                }
                st.ed.begin(Kind::Typing);
                st.ed.drag_obj(id, &move |o| o.opacity = v.clamp(0.05, 1.0));
                st.ed.changed();
            }
        }
        "corner" => {
            let cur = st.ed.object().map(|o| o.corner).unwrap_or(0.0);
            if let Some(v) = value_or_step(arg, cur, 2.0, is_step) {
                st.ed.edit_obj(&move |o| o.corner = v.max(0.0));
            }
        }
        "padding" => {
            let cur = match st.ed.object().map(|o| &o.kind) {
                Some(ObjKind::Text { padding, .. }) => *padding,
                _ => 4.0,
            };
            if let Some(v) = value_or_step(arg, cur, 1.0, is_step) {
                st.ed.edit_obj(&move |o| {
                    if let ObjKind::Text { padding, .. } = &mut o.kind {
                        *padding = v.max(0.0);
                    }
                });
            }
        }
        "frame-cols" => {
            let cur = match st.ed.object().map(|o| &o.kind) {
                Some(ObjKind::Text { columns, .. }) => *columns as f32,
                _ => 1.0,
            };
            if let Some(v) = value_or_step(arg, cur, 1.0, is_step) {
                st.ed.edit_obj(&move |o| {
                    if let ObjKind::Text { columns, .. } = &mut o.kind {
                        *columns = v.round().clamp(1.0, 8.0) as u8;
                    }
                });
            }
        }
        "valign" => {
            let v = match arg {
                "1" => VAlign::Center,
                "2" => VAlign::Bottom,
                _ => VAlign::Top,
            };
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Text { valign, .. } = &mut o.kind {
                    *valign = v;
                }
            });
        }
        "image-fit" => {
            let cover = arg == "0";
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Image { fit, .. } = &mut o.kind {
                    *fit = if cover { ImageFit::Cover } else { ImageFit::Contain };
                }
            });
        }
        "link-frame" => {
            if !matches!(st.ed.object().map(|o| &o.kind), Some(ObjKind::Text { .. })) {
                toast(st, tr("Zaznacz ramkę tekstową"));
                return;
            }
            set_tool(st, 5);
            toast(st, tr("Kliknij ramkę, do której ma popłynąć dalszy tekst"));
            return;
        }
        "unlink-frame" => {
            let Some(id) = st.ed.obj else { return };
            st.ed.begin(Kind::Other);
            let head = st.ed.doc.chain_head(id);
            let chain = st.ed.doc.chain(head);
            for c in chain {
                if let Some(ObjKind::Text { next, .. }) = st.ed.doc.object_mut(c).map(|o| &mut o.kind) {
                    *next = None;
                }
            }
            st.ed.changed();
        }
        "bool" => {
            let op = match arg {
                "difference" => BoolOp::Difference,
                "intersect" => BoolOp::Intersect,
                "xor" => BoolOp::Xor,
                _ => BoolOp::Union,
            };
            if !st.ed.boolean(op) {
                toast(st, tr("Zaznacz dwa kształty na tej samej stronie (drugi z Shift)"));
            }
        }
        "node-mode" => {
            if st.ed.obj.is_some() {
                st.ed.node_mode = !st.ed.node_mode;
                st.ed.sel_node = None;
                if let StoryRef::Obj(_) = st.ed.story {
                    st.ed.story = StoryRef::Body;
                    st.ed.fix_selection();
                }
            }
        }
        "node-add" => st.ed.shape_edit(&|s, sel| {
            let (ci, ni) = sel.unwrap_or((0, 0));
            if s.contours.get(ci).is_some_and(|c| c.nodes.len() >= 2) {
                geom::insert_node(s, ci, ni, 0.5);
                Some((ci, ni + 1))
            } else {
                sel
            }
        }),
        "node-delete" => st.ed.shape_edit(&|s, sel| {
            if let Some((ci, ni)) = sel {
                if let Some(c) = s.contours.get_mut(ci) {
                    if c.nodes.len() > if c.closed { 3 } else { 2 } {
                        c.nodes.remove(ni);
                        return Some((ci, ni.min(c.nodes.len() - 1)));
                    }
                }
            }
            sel
        }),
        "node-smooth" => st.ed.shape_edit(&|s, sel| {
            if let Some((ci, ni)) = sel {
                geom::toggle_smooth(s, ci, ni);
            }
            sel
        }),
        "seg-line" | "seg-curve" => {
            let curve = base == "seg-curve";
            st.ed.shape_edit(&move |s, sel| {
                let Some((ci, ni)) = sel else { return sel };
                let Some(c) = s.contours.get_mut(ci) else { return sel };
                let n = c.nodes.len();
                let j = (ni + 1) % n;
                let (a, b) = (c.nodes[ni], c.nodes[j]);
                if curve {
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    c.nodes[ni].cout = Some((a.x + dx / 3.0 - dy * 0.25, a.y + dy / 3.0 + dx * 0.25));
                    c.nodes[j].cin = Some((a.x + dx * 2.0 / 3.0 - dy * 0.25, a.y + dy * 2.0 / 3.0 + dx * 0.25));
                } else {
                    c.nodes[ni].cout = None;
                    c.nodes[j].cin = None;
                }
                sel
            });
            if let Some(id) = st.ed.obj {
                st.ed.normalize_shape(id);
                st.ed.relayout();
            }
        }
        "chart-kind" => {
            let k = match arg {
                "1" => ChartKind::Bar,
                "2" => ChartKind::Line,
                "3" => ChartKind::Area,
                "4" => ChartKind::Pie,
                "5" => ChartKind::Doughnut,
                _ => ChartKind::Column,
            };
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Chart { chart } = &mut o.kind {
                    chart.kind = k;
                }
            });
        }
        "chart-title" => {
            let t = arg.to_string();
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Chart { chart } = &mut o.kind {
                    chart.title = t.clone();
                }
            });
        }
        "chart-legend" => st.ed.edit_obj(&|o| {
            if let ObjKind::Chart { chart } = &mut o.kind {
                chart.legend = !chart.legend;
            }
        }),
        "chart-values" => st.ed.edit_obj(&|o| {
            if let ObjKind::Chart { chart } = &mut o.kind {
                chart.values = !chart.values;
            }
        }),
        "chart-data" => {
            let (labels, series) = parse_chart(arg);
            if labels.is_empty() {
                toast(st, tr("Nie rozpoznano danych wykresu"));
                return;
            }
            st.ed.edit_obj(&move |o| {
                if let ObjKind::Chart { chart } = &mut o.kind {
                    chart.labels = labels.clone();
                    chart.series = series.clone();
                }
            });
            toast(st, tr("Dane wykresu zastosowane"));
        }
        "path-text-set" => {
            let t = arg.to_string();
            let Some(id) = st.ed.obj else { return };
            st.ed.begin(Kind::Typing);
            st.ed.drag_obj(id, &move |o| {
                if let ObjKind::PathText { story, .. } = &mut o.kind {
                    let p = &mut story.paras[0];
                    let fmt = p.runs.first().map(|r| r.fmt.clone()).unwrap_or_default();
                    p.runs = vec![Run { text: t.clone(), fmt, field: None }];
                }
            });
            st.ed.changed();
        }
        "path-offset" => {
            let v: f32 = arg.parse().unwrap_or(0.0);
            let Some(id) = st.ed.obj else { return };
            st.ed.begin(Kind::Typing);
            st.ed.drag_obj(id, &move |o| {
                if let ObjKind::PathText { offset, .. } = &mut o.kind {
                    *offset = v - 0.5;
                }
            });
            st.ed.changed();
        }

        // ---------------------------------------------------------- recenzja
        "spell-next" => {
            if !opendtp_core::spell::ready() {
                toast(st, tr("Słowniki jeszcze się wczytują…"));
                return;
            }
            if spell_next(st) {
                scroll = true;
            } else {
                toast(st, tr("Nie znaleziono błędów pisowni"));
            }
        }
        "spell-live" => {
            st.view.spell = !st.view.spell;
            st.prefs.spell_live = st.view.spell;
            st.prefs.save();
            st.view.invalidate();
        }
        "stats" => {
            let (w, ns, c, p, pg) = st.ed.stats();
            app.set_stats_text(opendtp_core::trf!("Strony: {pg}\nAkapity: {p}\nWyrazy: {w}\nZnaki (bez spacji): {ns}\nZnaki (ze spacjami): {c}").into());
            app.set_dialog("stats".into());
            return;
        }
        "user-dict" => {
            let words: Vec<slint::SharedString> = opendtp_core::spell::user_words().into_iter().map(Into::into).collect();
            app.set_user_words(ModelRc::new(VecModel::from(words)));
            app.set_left_panel("dict".into());
            app.set_backstage(false);
            return;
        }
        "dict-remove" => {
            opendtp_core::spell::remove_user_word(arg);
            st.view.invalidate();
            return run(st, "user-dict", "");
        }

        // ---------------------------------------------------------- widok
        "zoom" => {
            let z = match arg {
                "width" => st.view.fit_width(&st.ed),
                "page" => st.view.fit_page(&st.ed),
                v => parse_num(v).map(|p| p / 100.0).unwrap_or(1.0),
            };
            set_zoom(st, z, None);
            return;
        }
        "zoom-in" => {
            let z = st.view.zoom * 1.15;
            set_zoom(st, z, None);
            return;
        }
        "zoom-out" => {
            let z = st.view.zoom / 1.15;
            set_zoom(st, z, None);
            return;
        }
        "marks" => {
            st.view.marks = !st.view.marks;
            st.view.invalidate();
        }
        "guides" => {
            st.view.guides = !st.view.guides;
            st.view.invalidate();
        }
        "panel" => {
            let right = arg == "props";
            if right {
                let cur = app.get_right_panel();
                app.set_right_panel(if cur == "props" { "".into() } else { "props".into() });
                if let Some(o) = st.ed.object() {
                    if let ObjKind::Chart { chart } = &o.kind {
                        app.set_chart_data(chart_text(chart).into());
                        app.set_chart_title(chart.title.clone().into());
                    }
                }
            } else {
                let cur = app.get_left_panel();
                app.set_left_panel(if cur == arg { "".into() } else { arg.into() });
                if arg == "library" {
                    library_list(&app);
                }
            }
        }
        "lib-save" => {
            if st.ed.obj.is_none() && !st.ed.has_sel() {
                toast(st, tr("Zaznacz tekst albo obiekty, które chcesz zapisać w bibliotece"));
                return;
            }
            let name = match st.ed.object() {
                Some(o) if !o.name.is_empty() => o.name.clone(),
                Some(o) => kind_label(o),
                None => {
                    let t = opendtp_core::edit::fragment_text(&st.ed.fragment());
                    t.lines().next().unwrap_or("").chars().take(40).collect::<String>().trim().to_string()
                }
            };
            app.set_lib_name(name.into());
            app.set_dialog("lib-save".into());
            return;
        }
        "lib-save-apply" => {
            app.set_dialog("".into());
            let name = arg.trim();
            let res = if let Some(id) = st.ed.obj {
                let ids: Vec<u64> = std::iter::once(id).chain(st.ed.multi.iter().copied()).collect();
                opendtp_core::library::save_objects(name, &st.ed.doc, &ids)
            } else {
                opendtp_core::library::save_text(name, &st.ed.doc, st.ed.fragment())
            };
            match res {
                Ok(_) => toast(st, &opendtp_core::trf!("Zapisano w bibliotece: {}", name)),
                Err(e) => toast(st, &e),
            }
            library_list(&app);
            return;
        }
        "lib-insert" => {
            let doc = match opendtp_core::library::load(std::path::Path::new(arg)) {
                Ok(d) => d,
                Err(e) => {
                    toast(st, &e);
                    return;
                }
            };
            // Style elementu, których nie ma w dokumencie.
            let missing: Vec<Style> = doc.styles.iter().filter(|s| st.ed.doc.style(&s.id).is_none()).cloned().collect();
            if !missing.is_empty() {
                st.ed.begin(Kind::Other);
                st.ed.doc.styles.extend(missing);
                st.ed.changed();
            }
            if doc.objects.is_empty() {
                if st.ed.obj.is_some() {
                    st.ed.obj = None;
                }
                st.ed.paste(&doc.body.paras);
                scroll = true;
            } else {
                let ps = st.ed.doc.page.clone();
                let (page, y) = st.ed.caret_page().map(|c| (c.0, c.2)).unwrap_or((0, ps.margin_top));
                let mut map: Vec<(u64, u64)> = Vec::new();
                let mut first = None;
                for o in &doc.objects {
                    let mut c = o.clone();
                    c.page = page;
                    c.x += ps.margin_left;
                    c.y += y;
                    if let ObjKind::Image { asset, .. } = &mut c.kind {
                        let key = format!("img{}", st.ed.doc.new_id());
                        if let Some(a) = doc.assets.get(asset.as_str()) {
                            st.ed.doc.assets.insert(key.clone(), a.clone());
                        }
                        if let Some(l) = doc.links.get(asset.as_str()) {
                            st.ed.doc.links.insert(key.clone(), l.clone());
                        }
                        *asset = key;
                    }
                    let new = st.ed.add_object(c);
                    map.push((o.id, new));
                    first.get_or_insert(new);
                }
                // Łańcuchy ramek – nowe identyfikatory.
                for &(_, new) in &map {
                    if let Some(ObjKind::Text { next, .. }) = st.ed.doc.object_mut(new).map(|o| &mut o.kind) {
                        *next = next.and_then(|n| map.iter().find(|m| m.0 == n).map(|m| m.1));
                    }
                }
                st.ed.relayout();
                st.ed.obj = first;
                st.ed.multi = map.iter().map(|m| m.1).filter(|id| Some(*id) != first).collect();
            }
        }
        "lib-remove" => {
            if let Err(e) = opendtp_core::library::remove(std::path::Path::new(arg)) {
                toast(st, &e);
            }
            library_list(&app);
            return;
        }
        "preflight" => {
            let cur = app.get_left_panel();
            app.set_left_panel(if cur == "preflight" && arg.is_empty() { "".into() } else { "preflight".into() });
            crate::preflight_list(&app, &st.ed);
        }
        "preflight-goto" => {
            if let Some(id) = arg.strip_prefix("obj:") {
                let id = id.to_string();
                return run(st, "image-goto", &id);
            }
            if let Some(p) = arg.strip_prefix("page:").and_then(|p| p.parse::<usize>().ok()) {
                let (_, cy) = st.view.to_canvas(p, 0.0, 0.0);
                let vy = (cy - 20.0).max(0.0);
                app.set_view_y(-vy);
                st.view.vy = vy;
            }
        }
        "print-bleed" | "print-bleed-step" | "print-marks" | "print-cmyk" => {
            st.ed.begin(Kind::Other);
            let p = &mut st.ed.doc.print;
            match base {
                "print-bleed-step" => {
                    let d: f32 = arg.parse().unwrap_or(0.0);
                    p.bleed = (p.bleed / MM + d).clamp(0.0, 20.0) * MM;
                }
                "print-bleed" => {
                    if let Some(v) = parse_num(arg) {
                        p.bleed = (v.clamp(0.0, 20.0)) * MM;
                    }
                }
                "print-marks" => p.marks = !p.marks,
                _ => p.cmyk = !p.cmyk,
            }
            st.ed.changed();
        }
        "swatch-add-used" => {
            // Wszystkie kolory użyte w dokumencie (tekst, style, wypełnienia, obrysy) jako próbki.
            let d = &st.ed.doc;
            let mut used: Vec<Color> = Vec::new();
            let mut add = |c: Option<Color>| {
                if let Some(c) = c {
                    let c = Color::rgb(c.r, c.g, c.b);
                    if c != Color::WHITE && !used.contains(&c) {
                        used.push(c);
                    }
                }
            };
            for s in &d.styles {
                add(s.chars.color);
            }
            let mut stories: Vec<&Story> = vec![&d.body];
            stories.extend(d.objects.iter().filter_map(|o| o.story()));
            for s in stories {
                for p in &s.paras {
                    for r in &p.runs {
                        add(r.fmt.color);
                    }
                }
            }
            for o in &d.objects {
                match &o.fill {
                    Some(Fill::Solid { color }) => add(Some(*color)),
                    Some(Fill::Linear { from, to, .. } | Fill::Radial { from, to }) => {
                        add(Some(*from));
                        add(Some(*to));
                    }
                    None => {}
                }
                add(o.stroke.as_ref().map(|s| s.color));
            }
            let new: Vec<Swatch> = used
                .into_iter()
                .filter(|c| !d.swatches.iter().any(|s| s.color.r == c.r && s.color.g == c.g && s.color.b == c.b))
                .map(|c| Swatch { name: format!("#{}", c.hex6().trim_start_matches('#')), color: c, cmyk: None, spot: false })
                .collect();
            if new.is_empty() {
                toast(st, tr("Wszystkie użyte kolory są już na liście"));
                return;
            }
            let n = new.len();
            st.ed.begin(Kind::Other);
            st.ed.doc.swatches.extend(new);
            st.ed.changed();
            toast(st, &opendtp_core::trf!("Dodano kolory: {}", n));
        }
        "swatch-remove" | "swatch-spot" | "swatch-name" | "swatch-cmyk" => {
            let (i, val) = arg.split_once(':').unwrap_or((arg, ""));
            let Ok(i) = i.parse::<usize>() else { return };
            if i >= st.ed.doc.swatches.len() {
                return;
            }
            st.ed.begin(Kind::Other);
            match base {
                "swatch-remove" => {
                    st.ed.doc.swatches.remove(i);
                }
                "swatch-spot" => {
                    let s = &mut st.ed.doc.swatches[i];
                    s.spot = !s.spot;
                }
                "swatch-name" => {
                    if !val.trim().is_empty() {
                        st.ed.doc.swatches[i].name = val.trim().to_string();
                    }
                }
                _ => {
                    // „c m y k” – procenty, oddzielone spacjami albo przecinkami.
                    let v: Vec<u8> = val.split(|c: char| c == ' ' || c == ',' || c == ';').filter(|t| !t.is_empty()).filter_map(|t| t.trim_end_matches('%').parse::<f32>().ok()).map(|f| f.round().clamp(0.0, 100.0) as u8).collect();
                    if v.len() == 4 {
                        st.ed.doc.swatches[i].cmyk = Some([v[0], v[1], v[2], v[3]]);
                    } else {
                        toast(st, tr("Podaj cztery wartości CMYK w procentach, np. 0 85 100 0"));
                    }
                }
            }
            st.ed.changed();
        }
        "chapter-move" => {
            let (p, dir) = arg.split_once(':').unwrap_or((arg, "down"));
            let Ok(p) = p.parse::<usize>() else { return };
            st.ed.begin(Kind::Other);
            let moved = st.ed.doc.move_chapter(p, dir == "up");
            match moved {
                Some(np) => {
                    st.ed.obj = None;
                    st.ed.story = StoryRef::Body;
                    st.ed.anchor = At::new(np, 0);
                    st.ed.head = At::new(np, 0);
                    st.ed.changed();
                    scroll = true;
                }
                None => {
                    st.ed.changed();
                    toast(st, tr("Rozdziału nie da się przesunąć dalej w obrębie rozdziału nadrzędnego"));
                }
            }
        }
        "goto-para" => {
            let p: usize = arg.parse().unwrap_or(0);
            st.ed.obj = None;
            st.ed.set_caret(StoryRef::Body, At::new(p, 0), false);
            // Nagłówek u góry widoku.
            if let Some((page, _, top, _)) = st.ed.caret_page() {
                let (_, cy) = st.view.to_canvas(page, 0.0, top);
                let vy = (cy - 40.0).max(0.0);
                app.set_view_y(-vy);
                st.view.vy = vy;
            }
        }
        "theme" => {
            st.prefs.theme = arg.parse().unwrap_or(0);
            st.prefs.save();
            crate::apply_theme_prefs(st, &ui);
            return;
        }
        "accent" => {
            if Color::parse(arg).is_some() {
                st.prefs.accent = arg.to_string();
                st.prefs.save();
                crate::apply_theme_prefs(st, &ui);
            } else {
                toast(st, tr("Nieprawidłowy kolor – wpisz np. #5b8cff"));
            }
            return;
        }
        "corners" => {
            st.prefs.corners = arg.parse().unwrap_or(1);
            st.prefs.save();
            crate::apply_theme_prefs(st, &ui);
            return;
        }
        "transparency" => {
            st.prefs.transparency = (arg.parse::<f32>().unwrap_or(0.0).round() as i32).clamp(0, 60);
            st.prefs.save();
            crate::apply_theme_prefs(st, &ui);
            return;
        }
        "ui-scale" => {
            st.prefs.scale = arg.parse::<f32>().unwrap_or(1.0).clamp(0.85, 1.35);
            st.prefs.save();
            crate::apply_theme_prefs(st, &ui);
            return;
        }
        "dense" => {
            st.prefs.dense = !st.prefs.dense;
            st.prefs.save();
            crate::apply_theme_prefs(st, &ui);
            return;
        }
        "ribbon-collapse" => {
            st.prefs.ribbon_collapsed = !st.prefs.ribbon_collapsed;
            app.set_ribbon_collapsed(st.prefs.ribbon_collapsed);
            app.set_ribbon_peek(false);
            st.prefs.save();
            return;
        }
        "fullscreen" => {
            let on = !app.get_fullscreen();
            app.set_fullscreen(on);
            crate::win::fullscreen(&ui, on);
            return;
        }
        "palette" => {
            palette_query(st, "");
            app.set_palette_open(true);
            return;
        }
        "customize" => {
            if arg == "qat" {
                customize_list(st, "qat");
                app.set_dialog("customize-qat".into());
            } else {
                let i = arg.parse::<usize>().unwrap_or(0);
                if i >= st.prefs.tabs().len() {
                    return;
                }
                app.set_custom_edit(i as i32);
                app.set_custom_name(st.prefs.tabs()[i].name.clone().into());
                customize_list(st, "tab");
                app.set_dialog("customize-tab".into());
            }
            app.set_backstage(false);
            return;
        }
        "qat-toggle" | "tab-toggle" => {
            let i = app.get_custom_edit().max(0) as usize;
            let list = if base == "qat-toggle" {
                &mut st.prefs.qat
            } else {
                match st.prefs.tabs_mut().get_mut(i) {
                    Some(t) => &mut t.items,
                    None => return,
                }
            };
            if let Some(i) = list.iter().position(|x| x == arg) {
                list.remove(i);
            } else {
                list.push(arg.to_string());
            }
            st.prefs.save();
            sync_lists(st);
            customize_list(st, if base == "qat-toggle" { "qat" } else { "tab" });
            return;
        }
        "qat-reset" | "tab-reset" => {
            if base == "qat-reset" {
                st.prefs.qat = crate::settings::default_qat();
            } else if let Some(t) = st.prefs.tabs_mut().get_mut(app.get_custom_edit().max(0) as usize) {
                t.items = crate::settings::default_tab();
            }
            st.prefs.save();
            sync_lists(st);
            customize_list(st, if base == "qat-reset" { "qat" } else { "tab" });
            return;
        }
        // Karty skrótów: dodanie (od razu okno wyboru poleceń), zmiana nazwy, usunięcie.
        "tab-add" => {
            if st.prefs.tabs().len() >= crate::settings::MAX_TABS {
                return;
            }
            st.prefs.tabs_mut().push(crate::settings::CustomTab { name: String::new(), items: crate::settings::default_tab() });
            let i = st.prefs.tabs().len() - 1;
            st.prefs.save();
            sync_lists(st);
            app.set_tab(crate::FIXED_TABS + i as i32);
            return run(st, "customize", &i.to_string());
        }
        "tab-rename" => {
            if let Some(t) = st.prefs.tabs_mut().get_mut(app.get_custom_edit().max(0) as usize) {
                t.name = arg.trim().to_string();
            }
            st.prefs.save();
            sync_lists(st);
            return;
        }
        "tab-remove" => {
            let i = app.get_custom_edit().max(0) as usize;
            if i < st.prefs.tabs().len() {
                st.prefs.tabs_mut().remove(i);
            }
            st.prefs.save();
            sync_lists(st);
            let last = crate::FIXED_TABS - 1 + st.prefs.tabs().len() as i32;
            if app.get_tab() > last {
                app.set_tab(last);
            }
            app.set_dialog("".into());
            return;
        }

        // ---------------------------------------------------------- ustawienia
        "s-lang" => {
            st.prefs.lang = match arg {
                "1" => "pl".into(),
                "2" => "en".into(),
                _ => String::new(),
            };
            st.prefs.save();
            app.set_s_lang(arg.parse().unwrap_or(0));
            toast(st, tr("Język zmieni się po ponownym uruchomieniu"));
            return;
        }
        "s-unit" => {
            st.prefs.unit = arg.parse().unwrap_or(0);
            st.prefs.save();
            app.set_s_unit(st.prefs.unit);
        }
        "s-autosave" => {
            st.prefs.autosave = arg.parse().unwrap_or(2);
            st.prefs.save();
            app.set_s_autosave(st.prefs.autosave);
            return;
        }
        "s-update-check" | "s-update-auto" | "s-smart-quotes" | "s-autocorrect" | "s-capitalize" | "s-autolist" | "s-restore" | "s-snap" => {
            let p = &mut st.prefs;
            let v = match base {
                "s-update-check" => &mut p.update_check,
                "s-update-auto" => &mut p.update_auto,
                "s-smart-quotes" => &mut p.smart_quotes,
                "s-autocorrect" => &mut p.autocorrect,
                "s-capitalize" => &mut p.capitalize,
                "s-autolist" => &mut p.autolist,
                "s-restore" => &mut p.restore,
                _ => &mut p.snap,
            };
            *v = !*v;
            st.prefs.save();
            crate::apply_theme_prefs(st, &ui);
            return;
        }
        "s-author" => {
            st.prefs.author = arg.to_string();
            st.ed.doc.meta.author = arg.to_string();
            st.prefs.save();
            return;
        }
        "s-default-font" => {
            st.prefs.default_font = arg.to_string();
            st.prefs.save();
            return;
        }
        "s-default-size" => {
            if let Some(v) = parse_num(arg) {
                st.prefs.default_size = v.clamp(4.0, 96.0);
                st.prefs.save();
            }
            return;
        }

        // ---------------------------------------------------------- sztuczna inteligencja
        "ai" => {
            if st.prefs.ai_model.is_empty() || (opendtp_core::ai::provider(&st.prefs.ai_provider).needs_key && st.prefs.ai_key.is_empty()) {
                app.set_ai_status(tr("Najpierw ustaw dostawcę, klucz API i model").into());
            }
            app.set_backstage(false);
            app.set_dialog("ai".into());
            return;
        }
        "ai-provider" => {
            st.prefs.ai_provider = arg.to_string();
            st.prefs.ai_base = String::new();
            st.prefs.ai_model = String::new();
            st.prefs.save();
            app.set_ai_models(ModelRc::new(VecModel::from(Vec::<ItemV>::new())));
            app.set_ai_status("".into());
            crate::sync_ai(st, &ui);
            return;
        }
        "ai-key" => {
            if !arg.trim().is_empty() {
                st.prefs.ai_key = crate::settings::protect(arg.trim());
                st.prefs.save();
                app.set_ai_key("".into());
                app.set_ai_has_key(true);
                toast(st, tr("Klucz API zapisany (zaszyfrowany)"));
            }
            return;
        }
        "ai-base" => {
            st.prefs.ai_base = arg.trim().to_string();
            st.prefs.save();
            return;
        }
        "ai-model" => {
            st.prefs.ai_model = arg.trim().to_string();
            st.prefs.save();
            app.set_ai_model(st.prefs.ai_model.clone().into());
            app.set_ai_status(opendtp_core::trf!("Model: {}", st.prefs.ai_model).into());
            return;
        }
        "ai-temp" => {
            st.prefs.ai_temp = arg.parse::<f32>().unwrap_or(0.7).clamp(0.0, 2.0);
            st.prefs.save();
            app.set_ai_temp(st.prefs.ai_temp);
            return;
        }
        "ai-models" => {
            let cfg = ai_config(st);
            app.set_ai_busy(true);
            app.set_ai_status(tr("Pobieranie listy modeli…").into());
            std::thread::spawn(move || {
                let res = opendtp_core::ai::list_models(&cfg);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(sh) = crate::shared() else { return };
                    let st = sh.borrow();
                    let Some(ui) = st.ui.upgrade() else { return };
                    let app = ui.global::<App>();
                    app.set_ai_busy(false);
                    match res {
                        Ok(list) => {
                            let n = list.len();
                            let items: Vec<ItemV> = list.into_iter().map(|m| ItemV { id: m.clone().into(), label: m.into(), ..Default::default() }).collect();
                            app.set_ai_models(ModelRc::new(VecModel::from(items)));
                            app.set_ai_status(opendtp_core::trf!("Dostępne modele: {}", n).into());
                        }
                        Err(e) => app.set_ai_status(opendtp_core::trf!("Nie udało się pobrać modeli: {}", e).into()),
                    }
                });
            });
            return;
        }
        "ai-generate" => {
            let insert = app.get_ai_insert();
            if !insert && !files::may_replace(st, "ai-generate") {
                app.set_dialog("ai".into());
                return;
            }
            let prompt = opendtp_core::ai::prompt(&app.get_ai_prompt(), app.get_ai_length());
            let cfg = ai_config(st);
            let template = files::blank(&st.prefs);
            app.set_dialog("ai".into());
            app.set_ai_busy(true);
            std::thread::spawn(move || {
                let res = opendtp_core::ai::complete(&cfg, &prompt).map(|answer| opendtp_core::ai::build(&answer, &template));
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(sh) = crate::shared() else { return };
                    let mut st = sh.borrow_mut();
                    let Some(ui) = st.ui.upgrade() else { return };
                    let app = ui.global::<App>();
                    app.set_ai_busy(false);
                    match res {
                        Ok(doc) => {
                            app.set_dialog("".into());
                            if insert {
                                let paras = doc.body.paras.clone();
                                st.ed.insert_paragraphs_at_caret(paras);
                                refresh_scroll(&mut st);
                            } else {
                                files::set_doc(&mut st, doc, None);
                                st.ed.dirty = true;
                                refresh(&mut st);
                            }
                            toast(&st, tr("Dokument wygenerowany – możesz go dowolnie edytować"));
                        }
                        Err(e) => app.set_ai_status(opendtp_core::trf!("Błąd: {}", e).into()),
                    }
                });
            });
            return;
        }

        // ---------------------------------------------------------- pisownia z menu
        _ if id.starts_with("spell-fix:") => {
            let rest = &id["spell-fix:".len()..];
            let mut it = rest.splitn(3, ':');
            let a: usize = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let b: usize = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let word = it.next().unwrap_or("").to_string();
            let p = st.ed.head.para;
            st.ed.anchor = At::new(p, a);
            st.ed.head = At::new(p, b);
            st.ed.insert(&word);
        }
        _ if id.starts_with("spell-add:") => {
            opendtp_core::spell::add_to_user(&id["spell-add:".len()..]);
            st.view.invalidate();
        }
        _ if id.starts_with("spell-ignore:") => {
            opendtp_core::spell::ignore_word(&id["spell-ignore:".len()..]);
            st.view.invalidate();
        }
        _ => {
            crate::set_status(st, &opendtp_core::trf!("Nieznane polecenie: {}", id));
            return;
        }
    }
    if scroll {
        refresh_scroll(st);
    } else {
        refresh(st);
    }
}

/// Nowy obiekt wstawiony podczas pisania w treści głównej płynie z tekstem (kotwica w akapicie kursora).
fn anchor_new(st: &mut State, id: u64) {
    if st.ed.story != StoryRef::Body {
        return;
    }
    let p = st.ed.head.para;
    st.ed.anchor_at(id, p);
    st.ed.changed();
}

/// Komórka pod punktem (pt) w tabeli obiektu.
/// Pole w miejscu kursora (bieżący tekst), kursor za polem.
fn insert_field_at_caret(st: &mut State, f: Field) {
    st.ed.begin(Kind::Other);
    let fmt = st.ed.caret_fmt();
    let h = st.ed.head;
    let story = st.ed.story;
    if let Some(s) = st.ed.doc.story_mut(story) {
        let end = opendtp_core::edit::insert_field(s, h, f, &fmt);
        st.ed.anchor = end;
        st.ed.head = end;
    }
    st.ed.changed();
}

pub fn cell_at(o: &Object, x: f32, y: f32) -> Option<(usize, usize, opendtp_core::geom::Rect)> {
    let ObjKind::Table { table } = &o.kind else { return None };
    opendtp_core::paint::table_cells(table, opendtp_core::geom::Rect::new(o.x, o.y, o.w, o.h)).into_iter().find(|(_, _, r)| r.contains(x, y))
}

/// Pokazuje pole edycji komórki nad tabelą.
pub fn show_cell(st: &mut State) {
    let Some(ui) = st.ui.upgrade() else { return };
    let app = ui.global::<App>();
    let Some((id, row, col)) = st.ed.cell else {
        app.set_cell_on(false);
        return;
    };
    let Some(o) = st.ed.doc.object(id).cloned() else {
        app.set_cell_on(false);
        return;
    };
    let ObjKind::Table { table } = &o.kind else { return };
    let cells = opendtp_core::paint::table_cells(table, opendtp_core::geom::Rect::new(o.x, o.y, o.w, o.h));
    let Some((_, _, r)) = cells.into_iter().find(|(ri, c, _)| *ri == row && *c == col) else {
        app.set_cell_on(false);
        return;
    };
    let (x, y) = st.view.to_canvas(o.page, r.x, r.y);
    let k = st.view.k();
    app.set_cell_box(crate::BoxV { x, y, w: r.w * k, h: r.h.max(14.0) * k });
    app.set_cell_text(table.rows.get(row).and_then(|r| r.get(col)).cloned().unwrap_or_default().into());
    app.set_cell_on(true);
    app.set_cell_serial(app.get_cell_serial() + 1);
}

fn ai_config(st: &State) -> opendtp_core::ai::Config {
    opendtp_core::ai::Config {
        provider: st.prefs.ai_provider.clone(),
        key: crate::settings::unprotect(&st.prefs.ai_key),
        model: st.prefs.ai_model.clone(),
        temperature: st.prefs.ai_temp,
        base: st.prefs.ai_base.clone(),
    }
}

thread_local! {
    static OBJ_CLIP: std::cell::RefCell<Option<Object>> = const { std::cell::RefCell::new(None) };
}

/// Lista elementów biblioteki do panelu.
fn library_list(app: &App) {
    let items: Vec<ItemV> = opendtp_core::library::list()
        .into_iter()
        .map(|i| ItemV {
            id: i.path.to_string_lossy().to_string().into(),
            label: i.name.into(),
            hint: if i.objects { tr("obiekty") } else { tr("tekst") }.into(),
            icon: commands::icon(if i.objects { "shapes" } else { "file-text" }),
            ..Default::default()
        })
        .collect();
    app.set_library(ModelRc::new(VecModel::from(items)));
}

/// Opcje wyszukiwania z panelu Znajdź i zamień.
fn find_opts(app: &App) -> opendtp_core::edit::FindOpts {
    opendtp_core::edit::FindOpts { case: app.get_find_case(), whole: app.get_find_whole(), regex: app.get_find_regex(), style: app.get_find_style().to_string() }
}

fn find_info(st: &State) {
    if let Some(ui) = st.ui.upgrade() {
        let app = ui.global::<App>();
        let n = st.ed.find.len();
        let opts = find_opts(&app);
        let bad = if opts.regex { opts.pattern(&app.get_find_text()).err() } else { None };
        let txt = if app.get_find_text().is_empty() && opts.style.is_empty() {
            String::new()
        } else if let Some(e) = bad {
            opendtp_core::trf!("Błędny wzorzec: {}", e.lines().last().unwrap_or(""))
        } else if n == 0 {
            tr("Brak wyników").to_string()
        } else {
            opendtp_core::trf!("Wynik {} z {}", (st.ed.find_i % n) + 1, n)
        };
        app.set_find_info(txt.into());
    }
}

fn change_case(st: &mut State, how: &str) {
    if !st.ed.has_sel() {
        let t = st.ed.para_text(st.ed.head.para);
        let (a, b) = opendtp_core::edit::word_at(&t, st.ed.head.off);
        let p = st.ed.head.para;
        st.ed.anchor = At::new(p, a);
        st.ed.head = At::new(p, b);
    }
    let frag = st.ed.fragment();
    let conv = |s: &str| -> String {
        match how {
            "upper" => s.to_uppercase(),
            "lower" => s.to_lowercase(),
            "title" => s.split(' ').map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                    None => String::new(),
                }
            }).collect::<Vec<_>>().join(" "),
            _ => {
                let lower = s.to_lowercase();
                let mut c = lower.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            }
        }
    };
    let new: Vec<Paragraph> = frag.into_iter().map(|mut p| {
        for r in p.runs.iter_mut() {
            if r.field.is_none() {
                r.text = conv(&r.text);
            }
        }
        p
    }).collect();
    let (a, _) = st.ed.range();
    st.ed.paste(&new);
    let end = st.ed.head;
    st.ed.anchor = a;
    st.ed.head = end;
}

fn spell_next(st: &mut State) -> bool {
    use unicode_segmentation::UnicodeSegmentation;
    let start = st.ed.range().1;
    let story = st.ed.story;
    let paras: Vec<(usize, String, String)> = st.ed.story_ref().paras.iter().enumerate().map(|(i, p)| (i, p.text(), st.ed.doc.resolve_para(p).1.lang)).collect();
    let n = paras.len();
    for k in 0..=n {
        let (pi, text, lang) = &paras[(start.para + k) % n];
        for (i, w) in text.split_word_bound_indices() {
            if k == 0 && i < start.off {
                continue;
            }
            if w.chars().any(|c| c.is_alphabetic()) && opendtp_core::spell::misspelled(w, lang) {
                st.ed.set_caret(story, At::new(*pi, i), false);
                st.ed.head = At::new(*pi, i + w.len());
                return true;
            }
        }
    }
    false
}

pub fn set_zoom(st: &mut State, z: f32, anchor: Option<(f32, f32)>) {
    let z = z.clamp(0.25, 4.0);
    let old = st.view.zoom;
    if (z - old).abs() < 1e-4 {
        return;
    }
    // Punkt pod wskaźnikiem (albo środek widoku) zostaje w miejscu.
    let (ax, ay) = anchor.unwrap_or((st.view.vx + st.view.vw / 2.0, st.view.vy + st.view.vh / 2.0));
    let (page, px, py) = st.view.to_page(ax, ay);
    let (sx, sy) = (ax - st.view.vx, ay - st.view.vy);
    st.view.zoom = z;
    st.view.place(&st.ed);
    let (nx, ny) = st.view.to_canvas(page, px, py);
    let vx = (nx - sx).max(0.0);
    let vy = (ny - sy).max(0.0);
    st.view.vx = vx;
    st.view.vy = vy;
    if let Some(ui) = st.ui.upgrade() {
        let app = ui.global::<App>();
        app.set_content_w(st.view.content_w);
        app.set_content_h(st.view.content_h);
        app.set_view_x(-vx);
        app.set_view_y(-vy);
    }
    st.prefs.zoom = (z * 100.0).round() as i32;
    refresh(st);
}

pub fn palette_query(st: &State, q: &str) {
    let Some(ui) = st.ui.upgrade() else { return };
    let items: Vec<ItemV> = commands::ALL
        .iter()
        .filter(|c| q.trim().is_empty() || commands::matches(c, q))
        .take(60)
        .map(|c| ItemV { id: c.id.into(), label: commands::label(c).into(), hint: c.key.into(), icon: commands::icon(c.icon), checked: false, font: "".into() })
        .collect();
    ui.global::<App>().set_palette_items(ModelRc::new(VecModel::from(items)));
}

pub fn font_query(st: &State, q: &str) {
    let Some(ui) = st.ui.upgrade() else { return };
    let ql = q.to_lowercase();
    let items: Vec<ItemV> = st.fonts.iter().filter(|f| ql.is_empty() || f.to_lowercase().contains(&ql)).take(800).map(|f| ItemV { id: f.clone().into(), label: f.clone().into(), ..Default::default() }).collect();
    ui.global::<App>().set_fonts(ModelRc::new(VecModel::from(items)));
}

fn customize_list(st: &State, which: &str) {
    let Some(ui) = st.ui.upgrade() else { return };
    let tab = st.prefs.tabs().get(ui.global::<App>().get_custom_edit().max(0) as usize);
    let list: &[String] = if which == "qat" { &st.prefs.qat } else { tab.map(|t| t.items.as_slice()).unwrap_or(&[]) };
    let items: Vec<ItemV> = commands::ALL.iter().map(|c| ItemV { id: c.id.into(), label: commands::label(c).into(), hint: c.key.into(), icon: Default::default(), checked: list.iter().any(|x| x == c.id), font: "".into() }).collect();
    ui.global::<App>().set_all_commands(ModelRc::new(VecModel::from(items)));
}

pub fn object_selected(st: &mut State) {
    let Some(ui) = st.ui.upgrade() else { return };
    let app = ui.global::<App>();
    if let Some(o) = st.ed.object() {
        if let ObjKind::Chart { chart } = &o.kind {
            app.set_chart_data(chart_text(chart).into());
            app.set_chart_title(chart.title.clone().into());
        }
        if let ObjKind::PathText { story, .. } = &o.kind {
            app.set_path_text(story.text().into());
        }
        if let ObjKind::Table { table } = &o.kind {
            app.set_table_data(table.rows.iter().map(|r| r.join(";")).collect::<Vec<_>>().join("\n").into());
        }
    }
}

fn chart_text(ch: &Chart) -> String {
    let mut s = String::from(";");
    s.push_str(&ch.series.iter().map(|x| x.name.clone()).collect::<Vec<_>>().join(";"));
    for (i, l) in ch.labels.iter().enumerate() {
        s.push('\n');
        s.push_str(l);
        for se in &ch.series {
            s.push(';');
            s.push_str(&se.values.get(i).map(|v| num(*v)).unwrap_or_default());
        }
    }
    s
}

fn parse_chart(t: &str) -> (Vec<String>, Vec<Series>) {
    let rows: Vec<Vec<String>> = t.lines().filter(|l| !l.trim().is_empty()).map(|l| l.split([';', '\t']).map(|c| c.trim().to_string()).collect()).collect();
    if rows.len() < 2 {
        return (Vec::new(), Vec::new());
    }
    let names: Vec<String> = rows[0].iter().skip(1).cloned().collect();
    let mut series: Vec<Series> = names.iter().map(|n| Series { name: n.clone(), values: Vec::new(), color: None }).collect();
    let mut labels = Vec::new();
    for r in &rows[1..] {
        labels.push(r.first().cloned().unwrap_or_default());
        for (i, s) in series.iter_mut().enumerate() {
            s.values.push(r.get(i + 1).and_then(|v| parse_num(v)).unwrap_or(0.0));
        }
    }
    (labels, series)
}

fn new_obj(page: usize, x: f32, y: f32, w: f32, h: f32, shape: Shape, kind: ObjKind, wrap: WrapMode) -> Object {
    Object {
        id: 0,
        name: String::new(),
        kind,
        page,
        x,
        y,
        w,
        h,
        shape,
        wrap: Wrap { mode: wrap, dist: 9.0, contour: true },
        fill: None,
        stroke: None,
        opacity: 1.0,
        corner: 0.0,
        shadow: false,
        locked: false,
        anchor_dy: 4.0,
        cont_of: None,
        repeat: Default::default(),
    }
}

pub fn insert_image_bytes(st: &mut State, bytes: Vec<u8>) {
    let Some(asset) = io::odtp::asset_from(bytes) else {
        toast(st, tr("Nieobsługiwany format obrazu"));
        return;
    };
    insert_image(st, asset, None);
}

/// Klucz obrazu zaznaczonego obiektu.
fn selected_asset(st: &State) -> Option<String> {
    match &st.ed.object()?.kind {
        ObjKind::Image { asset, .. } => Some(asset.clone()),
        _ => None,
    }
}

/// Sprawdzenie plików obrazów linkowanych: zmienione wczytujemy, brakujące oznaczamy.
pub fn links_tick(st: &mut State, verbose: bool) {
    if st.ed.doc.links.is_empty() {
        if verbose {
            toast(st, tr("Dokument nie ma obrazów linkowanych"));
        }
        return;
    }
    let dir = st.ed.path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf());
    let changed = io::odtp::refresh_links(&mut st.ed.doc, dir.as_deref(), verbose);
    if changed.is_empty() {
        if verbose {
            toast(st, tr("Wszystkie obrazy linkowane są aktualne"));
        }
        return;
    }
    for k in &changed {
        st.view.forget_image(k);
    }
    st.ed.relayout();
    st.view.invalidate();
    let missing = changed.iter().filter(|k| st.ed.doc.missing.contains(*k)).count();
    if missing > 0 {
        toast(st, &opendtp_core::trf!("Nie znaleziono plików obrazów: {} – wskaż je w panelu Obrazy", missing));
    } else {
        toast(st, &opendtp_core::trf!("Odświeżono obrazy zmienione na dysku: {}", changed.len()));
    }
    refresh(st);
}

fn insert_image(st: &mut State, asset: Asset, link: Option<ImageLink>) {
    let ps = st.ed.doc.page.clone();
    let key = format!("img{}", st.ed.doc.new_id());
    let (iw, ih) = (asset.width.max(1) as f32 * 0.75, asset.height.max(1) as f32 * 0.75);
    let maxw = ps.content_width() * 0.6;
    let k = (maxw / iw).min(1.0).min(ps.height * 0.5 / ih);
    let (w, h) = (iw * k, ih * k);
    st.ed.doc.assets.insert(key.clone(), asset);
    if let Some(l) = link {
        st.ed.doc.links.insert(key.clone(), l);
    }
    let (page, y) = st.ed.caret_page().map(|c| (c.0, c.2)).unwrap_or((0, ps.margin_top));
    let x = ps.width - ps.margin_right - w;
    let o = new_obj(page, x, y, w, h, Shape::rect(), ObjKind::Image { asset: key, fit: ImageFit::Cover, contour: None }, WrapMode::Both);
    let id = st.ed.add_object(o);
    anchor_new(st, id);
    // Obraz z przezroczystością – od razu opływanie konturem.
    let id = st.ed.obj;
    if let Some(id) = id {
        let cont = st.ed.doc.object(id).and_then(|o| match &o.kind {
            ObjKind::Image { asset, .. } => st.ed.doc.assets.get(asset).cloned(),
            _ => None,
        });
        if let Some(a) = cont {
            if a.mime == "image/png" || a.mime == "image/webp" || a.mime == "image/gif" {
                if let Some(c) = image::load_from_memory(&a.data).ok().map(|i| i.to_rgba8()).and_then(|img| {
                    let (w, h) = img.dimensions();
                    let has_alpha = img.pixels().any(|p| p[3] < 250);
                    if has_alpha {
                        geom::image_contour(img.as_raw(), w, h)
                    } else {
                        None
                    }
                }) {
                    if let Some(ObjKind::Image { contour, .. }) = st.ed.doc.object_mut(id).map(|o| &mut o.kind) {
                        *contour = Some(c);
                    }
                    st.ed.relayout();
                }
            }
        }
    }
    refresh(st);
}

/// Koniec przeciągania narzędziem: ramka, kształt albo tekst na ścieżce.
pub fn create_from_drag(st: &mut State, page: usize, x: f32, y: f32, w: f32, h: f32) {
    let tool = st.ui.upgrade().map(|u| u.global::<App>().get_tool()).unwrap_or(0);
    let small = w < 8.0 && h < 8.0;
    let preset = st.shape_preset.clone();
    let shape = geom::preset(&preset).unwrap_or_else(Shape::rect);
    let open = shape.contours.iter().all(|c| !c.closed);
    let (w, h) = if small {
        match tool {
            1 => (220.0, 90.0),
            4 => (220.0, 90.0),
            _ if open => (160.0, 40.0),
            _ => (160.0, 160.0),
        }
    } else {
        (w.max(10.0), h.max(if open { 4.0 } else { 10.0 }))
    };
    let accent = Color::rgb(0x5b, 0x8c, 0xff);
    let o = match tool {
        1 => {
            let mut o = new_obj(page, x, y, w, h, Shape::rect(), ObjKind::Text { story: Story::default(), padding: 6.0, columns: 1, gap: 12.0, valign: VAlign::Top, next: None }, WrapMode::Both);
            o.wrap.dist = 8.0;
            o
        }
        4 => {
            let mut sp = Paragraph::with_text("normal", &tr("Tekst na ścieżce"));
            sp.fmt.align = Some(Align::Center);
            sp.runs[0].fmt.size = Some(18.0);
            new_obj(page, x, y, w, h, geom::arc_path(), ObjKind::PathText { story: Story { paras: vec![sp] }, offset: 0.0 }, WrapMode::None)
        }
        _ if st.text_shape && !open => {
            let mut sp = Paragraph::with_text("normal", "");
            sp.fmt.align = Some(Align::Center);
            let mut o = new_obj(page, x, y, w, h, shape, ObjKind::Text { story: Story { paras: vec![sp] }, padding: 8.0, columns: 1, gap: 12.0, valign: VAlign::Top, next: None }, WrapMode::Both);
            o.fill = Some(Fill::Solid { color: Color::rgb(0xee, 0xf3, 0xfc) });
            o.stroke = Some(StrokeStyle { color: accent, width: 1.0, dash: vec![] });
            o
        }
        _ => {
            let mut o = new_obj(page, x, y, w, h, shape, ObjKind::Shape, WrapMode::None);
            if open {
                o.stroke = Some(StrokeStyle { color: Color::rgb(0x30, 0x30, 0x30), width: 2.0, dash: vec![] });
            } else {
                o.fill = Some(Fill::Solid { color: accent });
            }
            o
        }
    };
    let is_text = matches!(o.kind, ObjKind::Text { .. });
    let id = st.ed.add_object(o);
    set_tool(st, 0);
    if is_text {
        st.ed.set_caret(StoryRef::Obj(id), At::new(0, 0), false);
        st.ed.obj = Some(id);
    }
    object_selected(st);
    refresh(st);
}

/// Zakończenie rysowania piórem: zamknięty kształt staje się kształtem tekstu.
pub fn finish_pen(st: &mut State, close: bool) {
    let pts = std::mem::take(&mut st.pen);
    if let Some(ui) = st.ui.upgrade() {
        ui.global::<App>().set_preview_path("".into());
    }
    set_tool(st, 0);
    if pts.len() < 2 || (close && pts.len() < 3) {
        refresh(st);
        return;
    }
    // Ramka: obwiednia węzłów i uchwytów.
    let mut x0 = f32::MAX;
    let mut y0 = f32::MAX;
    let mut x1 = f32::MIN;
    let mut y1 = f32::MIN;
    for &(x, y, h) in &pts {
        for (a, b) in [Some((x, y)), h, h.map(|(hx, hy)| (2.0 * x - hx, 2.0 * y - hy))].into_iter().flatten() {
            x0 = x0.min(a);
            y0 = y0.min(b);
            x1 = x1.max(a);
            y1 = y1.max(b);
        }
    }
    let (w, h) = ((x1 - x0).max(4.0), (y1 - y0).max(4.0));
    let nrm = |(a, b): (f32, f32)| ((a - x0) / w, (b - y0) / h);
    let nodes: Vec<Node> = pts
        .iter()
        .map(|&(x, y, hd)| {
            let (nx, ny) = nrm((x, y));
            Node { x: nx, y: ny, cout: hd.map(nrm), cin: hd.map(|(hx, hy)| nrm((2.0 * x - hx, 2.0 * y - hy))), smooth: hd.is_some() }
        })
        .collect();
    let shape = Shape { contours: vec![Contour { nodes, closed: close }] };
    let page = st.pen_page;
    let o = if close {
        let mut sp = Paragraph::with_text("normal", "");
        sp.fmt.align = Some(Align::Justify);
        let mut o = new_obj(page, x0, y0, w, h, shape, ObjKind::Text { story: Story { paras: vec![sp] }, padding: 6.0, columns: 1, gap: 12.0, valign: VAlign::Top, next: None }, WrapMode::Both);
        o.stroke = Some(StrokeStyle { color: Color::rgb(0x5b, 0x8c, 0xff), width: 0.75, dash: vec![] });
        o.fill = Some(Fill::Solid { color: Color::rgb(0xf4, 0xf7, 0xfd) });
        o
    } else {
        let mut sp = Paragraph::with_text("normal", &tr("Tekst na ścieżce"));
        sp.runs[0].fmt.size = Some(16.0);
        new_obj(page, x0, y0, w, h, shape, ObjKind::PathText { story: Story { paras: vec![sp] }, offset: 0.0 }, WrapMode::None)
    };
    let text = matches!(o.kind, ObjKind::Text { .. });
    let id = st.ed.add_object(o);
    if text {
        st.ed.set_caret(StoryRef::Obj(id), At::new(0, 0), false);
        st.ed.obj = Some(id);
        toast(st, tr("Kształt tekstu gotowy – pisz albo wklej tekst; dwuklik na krawędzi edytuje węzły"));
    }
    object_selected(st);
    refresh(st);
}

pub fn link_frames(st: &mut State, target: u64) {
    let Some(src) = st.ed.obj else { return };
    if src == target {
        return;
    }
    let ok_target = matches!(st.ed.doc.object(target).map(|o| &o.kind), Some(ObjKind::Text { .. }));
    if !ok_target {
        toast(st, tr("Wskaż ramkę tekstową"));
        return;
    }
    let head = st.ed.doc.chain_head(src);
    let chain = st.ed.doc.chain(head);
    if chain.contains(&target) || st.ed.doc.chain_head(target) != target || st.ed.doc.chain(target).len() > 1 {
        toast(st, tr("Ta ramka należy już do łańcucha"));
        return;
    }
    let last = *chain.last().unwrap();
    st.ed.begin(Kind::Other);
    // Tekst wskazanej ramki dołącza na koniec łańcucha.
    let extra = st.ed.doc.object(target).and_then(|o| o.story().cloned()).filter(|s| !s.text().trim().is_empty());
    if let Some(ObjKind::Text { next, .. }) = st.ed.doc.object_mut(last).map(|o| &mut o.kind) {
        *next = Some(target);
    }
    if let Some(extra) = extra {
        if let Some(ObjKind::Text { story, .. }) = st.ed.doc.object_mut(head).map(|o| &mut o.kind) {
            story.paras.extend(extra.paras);
        }
    }
    if let Some(ObjKind::Text { story, .. }) = st.ed.doc.object_mut(target).map(|o| &mut o.kind) {
        *story = Story::default();
    }
    st.ed.changed();
    toast(st, tr("Ramki połączone – tekst płynie dalej"));
}

pub fn apply_painter(st: &mut State) {
    let Some((f, para)) = st.ed.painter.clone() else { return };
    if st.ed.has_sel() {
        st.ed.format(&move |x| *x = f.clone());
    }
    if let Some((style, pf)) = para {
        st.ed.format_paras(&move |p| {
            p.style = style.clone();
            p.fmt = pf.clone();
        });
    }
    if st.ui.upgrade().is_some_and(|u| u.global::<App>().get_tool() == 6) {
        set_tool(st, 0);
        st.ed.painter = None;
    }
    refresh(st);
}

// ---------------------------------------------------------------- aktualizacje

fn set_update(text: &str, ready: bool, busy: bool) {
    let Some(sh) = crate::shared() else { return };
    let Ok(st) = sh.try_borrow() else { return };
    if let Some(ui) = st.ui.upgrade() {
        let app = ui.global::<App>();
        app.set_update_status(text.into());
        app.set_update_ready(ready);
        app.set_update_busy(busy);
    }
}

pub fn check_update(manual: bool) {
    let Some(sh) = crate::shared() else { return };
    {
        let Ok(st) = sh.try_borrow() else { return };
        if !manual && !st.prefs.update_check {
            return;
        }
        if st.update_busy {
            return;
        }
    }
    set_update(tr("Sprawdzanie aktualizacji…"), false, true);
    std::thread::spawn(move || {
        let res = opendtp_core::update::check();
        let _ = slint::invoke_from_event_loop(move || {
            let Some(sh) = crate::shared() else { return };
            let when = opendtp_core::util::now_time();
            match res {
                Ok(Some(rel)) => {
                    let v = rel.version.clone();
                    let auto = sh.borrow().prefs.update_auto;
                    sh.borrow_mut().update = Some(rel);
                    set_update(&opendtp_core::trf!("Dostępna wersja {v} (masz {}) · sprawdzono o {when}", opendtp_core::version()), true, false);
                    if auto {
                        let mut st = sh.borrow_mut();
                        download_update(&mut st, false);
                    } else {
                        let st = sh.borrow();
                        toast(&st, &opendtp_core::trf!("Jest nowa wersja OpenDTP {v} – Plik → Ustawienia → Zaktualizuj"));
                    }
                }
                Ok(None) => {
                    set_update(&opendtp_core::trf!("Masz najnowszą wersję {} · sprawdzono o {when}", opendtp_core::version()), false, false);
                    if manual {
                        toast(&sh.borrow(), tr("Masz najnowszą wersję"));
                    }
                }
                Err(e) => {
                    set_update(&opendtp_core::trf!("Nie udało się sprawdzić ({}) · {when}", e), false, false);
                    if manual {
                        toast(&sh.borrow(), tr("Nie udało się sprawdzić aktualizacji"));
                    }
                }
            }
        });
    });
}

/// „Zaktualizuj”: pobranie (jeśli trzeba) i instalacja od razu, z ponownym uruchomieniem programu.
pub fn install_update(st: &mut State) {
    if let Some(p) = st.pending_setup.clone() {
        run_setup(st, &p, true);
        return;
    }
    download_update(st, true);
}

fn run_setup(st: &mut State, path: &std::path::Path, now: bool) {
    if now && st.ed.dirty {
        let _ = files::save(st, false);
        if st.ed.dirty {
            toast(st, tr("Zapisz dokument przed aktualizacją"));
            return;
        }
    }
    let applied = if opendtp_core::update::installed() { opendtp_core::update::apply(path, now) } else { opendtp_core::update::apply_portable(path) };
    match applied {
        Ok(()) => {
            if now {
                st.pending_close = true;
                files::quit(st);
            }
        }
        Err(e) => set_update(&opendtp_core::trf!("Instalacja nie powiodła się: {}", e), true, false),
    }
}

/// Pobiera wydanie w tle. `now` – zainstaluj od razu; inaczej przy zamknięciu programu.
fn download_update(st: &mut State, now: bool) {
    let Some(rel) = st.update.clone() else {
        check_update(true);
        return;
    };
    if st.update_busy {
        return;
    }
    st.update_busy = true;
    set_update(tr("Pobieranie aktualizacji…"), true, true);
    std::thread::spawn(move || {
        let res = opendtp_core::update::download(&rel, |done, total| {
            let pct = if total > 0 { done * 100 / total } else { 0 };
            let _ = slint::invoke_from_event_loop(move || set_update(&opendtp_core::trf!("Pobieranie aktualizacji… {}%", pct), true, true));
        });
        let _ = slint::invoke_from_event_loop(move || {
            let Some(sh) = crate::shared() else { return };
            sh.borrow_mut().update_busy = false;
            match res {
                Ok(path) => {
                    let mut st = sh.borrow_mut();
                    st.pending_setup = Some(path.clone());
                    if now {
                        run_setup(&mut st, &path, true);
                    } else {
                        set_update(&opendtp_core::trf!("Wersja {} pobrana – zainstaluje się przy zamknięciu programu", rel.version), true, false);
                        toast(&st, tr("Aktualizacja pobrana – zainstaluje się po zamknięciu OpenDTP"));
                    }
                }
                Err(e) => set_update(&e, true, false),
            }
        });
    });
}

#[allow(dead_code)]
fn later(f: impl FnOnce() + 'static) {
    slint::Timer::single_shot(Duration::ZERO, f);
}

#[allow(dead_code)]
fn _unused(st: &State) -> String {
    len_str(0.0, st.prefs.unit)
}

/// Sekcja, w której stoi kursor (tylko treść główna).
fn section_of(st: &mut State) -> Option<&mut Section> {
    if st.ed.story != StoryRef::Body {
        return None;
    }
    let i = st.ed.doc.section_start(st.ed.head.para)?;
    st.ed.doc.body.paras[i].fmt.section.as_mut()
}
