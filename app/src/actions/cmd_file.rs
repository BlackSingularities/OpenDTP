//! Polecenia pliku i edycji: nowy, otwórz, zapisz, eksport, schowek, cofanie.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle(st: &mut State, ui: &crate::AppWindow, app: &App, unit: i32, uk: f32, is_step: bool, base: &str, id: &str, arg: &str, scroll: &mut bool) -> Flow {
    let _ = (ui, unit, uk, is_step, id, &scroll);
    match base {
        // ---------------------------------------------------------- plik
        "backstage" => {
            app.set_backstage_page(0);
            app.set_backstage(true);
            let (w, ns, c, p, pg) = st.ed.stats();
            app.set_stats_text(opendtp_core::trf!("Strony: {pg}\nAkapity: {p}\nWyrazy: {w}\nZnaki (bez spacji): {ns}\nZnaki (ze spacjami): {c}").into());
            return Flow::Skip;
        }
        "new" => {
            let t = if arg.is_empty() { "blank" } else { arg };
            if !files::may_replace(st, &format!("new:{t}")) {
                return Flow::Skip;
            }
            let doc = if t == "blank" { files::blank(&st.prefs) } else { opendtp_core::sample::template(t) };
            files::set_doc(st, doc, None);
            return Flow::Skip;
        }
        "open" => {
            if !files::may_replace(st, "open") {
                return Flow::Skip;
            }
            files::open_dialog(st);
            return Flow::Skip;
        }
        "open-path" => {
            if !files::may_replace(st, &format!("open-path:{arg}")) {
                return Flow::Skip;
            }
            files::open_path(st, arg);
            return Flow::Skip;
        }
        "recent-remove" => {
            st.prefs.recent.retain(|(p, _)| p != arg);
            st.prefs.save();
            sync_lists(st);
            return Flow::Skip;
        }
        "save" => {
            files::save(st, false);
            app.set_backstage(false);
            return Flow::Skip;
        }
        "save-as" => {
            files::save(st, true);
            app.set_backstage(false);
            return Flow::Skip;
        }
        "export" => {
            files::export(st, arg);
            return Flow::Skip;
        }
        "print" => {
            files::print(st);
            return Flow::Skip;
        }
        "doc-info" => {
            let (w, ns, c, p, pg) = st.ed.stats();
            app.set_stats_text(opendtp_core::trf!("Strony: {pg}\nAkapity: {p}\nWyrazy: {w}\nZnaki (bez spacji): {ns}\nZnaki (ze spacjami): {c}").into());
            app.set_meta_subject(st.ed.doc.meta.subject.clone().into());
            app.set_meta_keywords(st.ed.doc.meta.keywords.clone().into());
            return Flow::Skip;
        }
        "meta-title" | "meta-author" | "meta-subject" | "meta-keywords" => {
            let m = &mut st.ed.doc.meta;
            let v = arg.to_string();
            match base {
                "meta-title" => m.title = v,
                "meta-author" => m.author = v,
                "meta-subject" => m.subject = v,
                _ => m.keywords = v,
            }
            st.ed.dirty = true;
            // Pola metadanych w tekście pokazują nową wartość.
            st.ed.relayout();
        }
        "code" => {
            app.set_dialog("code".into());
            return Flow::Skip;
        }
        "code-insert" => {
            let text = app.get_code_text().to_string();
            let qr = app.get_code_kind() == 0;
            if text.trim().is_empty() {
                toast(st, tr("Wpisz treść kodu"));
                return Flow::Skip;
            }
            match code_svg(qr, &text) {
                Some(svg) => {
                    app.set_dialog("".into());
                    insert_image_bytes(st, svg.into_bytes());
                }
                None => {
                    toast(st, tr("Kod kreskowy przyjmuje tylko znaki ASCII (litery bez polskich znaków, cyfry, interpunkcję)"));
                    return Flow::Skip;
                }
            }
        }
        "cover" => {
            let paras = cover_paras(&st.ed.doc, arg);
            st.ed.insert_cover(paras);
            *scroll = true;
        }
        "meta-edit" => {
            app.set_meta_subject(st.ed.doc.meta.subject.clone().into());
            app.set_meta_keywords(st.ed.doc.meta.keywords.clone().into());
            app.set_backstage_page(5);
            app.set_backstage(true);
            return Flow::Skip;
        }
        "confirm-save" | "confirm-discard" => {
            if base == "confirm-save" && !files::save(st, false) {
                return Flow::Skip;
            }
            st.ed.dirty = false;
            let then = arg.to_string();
            if then == "exit" {
                st.pending_close = true;
                files::quit(st);
                return Flow::Skip;
            }
            let (a, b) = then.split_once(':').unwrap_or((then.as_str(), ""));
            let (a, b) = (a.to_string(), b.to_string());
            { run(st, &a, &b); return Flow::Skip; }
        }
        "settings" => {
            app.set_backstage_page(6);
            app.set_backstage(true);
            return Flow::Skip;
        }
        "about" => {
            app.set_backstage_page(7);
            app.set_backstage(true);
            return Flow::Skip;
        }
        "website" => {
            let _ = open::that(opendtp_core::AUTHOR_URL);
            return Flow::Skip;
        }
        "email" => {
            let _ = open::that("mailto:kontakt@redzikowski.com");
            return Flow::Skip;
        }
        "support" => {
            let _ = open::that(opendtp_core::SUPPORT_URL);
            return Flow::Skip;
        }
        "check-update" => {
            check_update(true);
            return Flow::Skip;
        }
        "feedback" => {
            crate::feedback::open_form();
            return Flow::Skip;
        }
        "install-update" => {
            install_update(st);
            return Flow::Skip;
        }

        // ---------------------------------------------------------- edycja
        "undo" => {
            if !st.ed.undo() {
                toast(st, tr("Nie ma czego cofnąć"));
            }
            st.view.invalidate();
            *scroll = true;
        }
        "redo" => {
            st.ed.redo();
            st.view.invalidate();
            *scroll = true;
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
                    return Flow::Skip;
                }
            }
            if base == "paste" {
                if let Some((t, frag)) = &st.clip {
                    if sys.as_deref() == Some(t.as_str()) && !frag.is_empty() {
                        let f = frag.clone();
                        st.ed.paste(&f);
                        refresh_scroll(st);
                        return Flow::Skip;
                    }
                }
                // Z innych programów (Word, przeglądarka, Outlook…): HTML z pełnym formatowaniem.
                if let Some(html) = arboard::Clipboard::new().ok().and_then(|mut c| c.get().html().ok()) {
                    let frag = io::html_in::fragment_from_html(&html);
                    if !frag.is_empty() {
                        st.ed.paste_rich(&frag);
                        refresh_scroll(st);
                        return Flow::Skip;
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
                        return Flow::Skip;
                    }
                }
            }
            if let Some(t) = sys {
                if !t.is_empty() {
                    st.ed.paste_text(&t);
                    *scroll = true;
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
            return Flow::Skip;
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
            *scroll = true;
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
            *scroll = true;
        }
        "replace-all" => {
            let n = st.ed.replace_all(&app.get_find_text(), &app.get_replace_text(), &find_opts(&app));
            toast(st, &opendtp_core::trf!("Zamieniono: {}", n));
            st.ed.find.clear();
            find_info(st);
        }

        _ => return Flow::Unhandled,
    }
    Flow::Done
}
