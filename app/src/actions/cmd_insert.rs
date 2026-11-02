//! Polecenia wstawiania: obiekty, obrazy, tabele, wykresy, pola, odnośniki.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle(st: &mut State, ui: &crate::AppWindow, app: &App, unit: i32, uk: f32, is_step: bool, base: &str, id: &str, arg: &str, scroll: &mut bool) -> Flow {
    let _ = (ui, unit, uk, is_step, id, &scroll);
    match base {
        // ---------------------------------------------------------- wstawianie
        "page-break" => {
            st.ed.page_break(false);
            *scroll = true;
        }
        "column-break" => {
            st.ed.page_break(true);
            *scroll = true;
        }
        "page-blank" => {
            st.ed.begin(Kind::Other);
            let n = st.ed.layout.pages.len();
            st.ed.doc.min_pages = st.ed.doc.min_pages.max(n + 1);
            st.ed.changed();
            toast(st, tr("Dodano pustą stronę – wstaw na nią obrazy, kształty i ramki"));
        }
        "image" => {
            let Some(p) = rfd::FileDialog::new().set_title(tr("Wstaw obraz")).add_filter(tr("Obrazy"), &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"]).pick_file() else { return Flow::Skip };
            match std::fs::read(&p) {
                Ok(b) => insert_image_bytes(st, b),
                Err(e) => toast(st, &e.to_string()),
            }
            return Flow::Skip;
        }
        "image-link" => {
            let Some(p) = rfd::FileDialog::new().set_title(tr("Wstaw obraz jako łącze do pliku")).add_filter(tr("Obrazy"), &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"]).pick_file() else { return Flow::Skip };
            match io::odtp::link_file(&p) {
                Ok((asset, link)) => insert_image(st, asset, Some(link)),
                Err(e) => toast(st, &e),
            }
            return Flow::Skip;
        }
        "image-embed" | "image-to-link" | "image-relink" => {
            // arg: klucz obrazu (z panelu Obrazy) albo pusty – obraz zaznaczonego obiektu.
            let key = if arg.is_empty() { selected_asset(st) } else { Some(arg.to_string()) };
            let Some(key) = key else {
                toast(st, tr("Zaznacz obraz"));
                return Flow::Skip;
            };
            match base {
                "image-embed" => {
                    if st.ed.doc.missing.contains(&key) {
                        toast(st, tr("Brak pliku obrazu – najpierw wskaż go ponownie"));
                        return Flow::Skip;
                    }
                    st.ed.begin(Kind::Other);
                    st.ed.doc.links.remove(&key);
                    st.ed.changed();
                    toast(st, tr("Obraz osadzony w dokumencie"));
                }
                "image-to-link" => {
                    let Some(a) = st.ed.doc.assets.get(&key).cloned() else { return Flow::Skip };
                    let name = format!("{}.{}", files::doc_name(&st.ed), io::odtp::ext_of(&a.mime));
                    let mut dlg = rfd::FileDialog::new().set_title(tr("Zapisz obraz jako plik i połącz")).set_file_name(&name);
                    if let Some(dir) = st.ed.path.as_ref().and_then(|p| p.parent()) {
                        dlg = dlg.set_directory(dir);
                    }
                    let Some(p) = dlg.save_file() else { return Flow::Skip };
                    if let Err(e) = opendtp_core::fsx::write_atomic(&p, a.data.as_slice()) {
                        toast(st, &e.to_string());
                        return Flow::Skip;
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
                    let Some(p) = rfd::FileDialog::new().set_title(tr("Wskaż plik obrazu")).add_filter(tr("Obrazy"), &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"]).pick_file() else { return Flow::Skip };
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
                return Flow::Skip;
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
            return Flow::Skip;
        }
        "package" => {
            if st.ed.doc.links.is_empty() {
                toast(st, tr("Dokument nie ma obrazów linkowanych – wystarczy zwykły zapis"));
            }
            let Some(dir) = rfd::FileDialog::new().set_title(tr("Spakuj: wybierz pusty folder na dokument i obrazy")).pick_folder() else { return Flow::Skip };
            match io::odtp::package(&st.ed.doc, &dir, &files::doc_name(&st.ed)) {
                Ok(p) => toast(st, &opendtp_core::trf!("Spakowano do: {}", p.display())),
                Err(e) => toast(st, &e),
            }
            return Flow::Skip;
        }
        "image-goto" => {
            let Ok(id) = arg.parse::<u64>() else { return Flow::Skip };
            let Some((page, y)) = st.ed.doc.object(id).map(|o| (o.page, o.y)) else { return Flow::Skip };
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
                { run(st, "image-relink", &key); return Flow::Skip; }
            }
            let Some(p) = rfd::FileDialog::new().set_title(tr("Zmień obraz")).add_filter(tr("Obrazy"), &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"]).pick_file() else { return Flow::Skip };
            let Some(asset) = std::fs::read(&p).ok().and_then(io::odtp::asset_from) else {
                toast(st, tr("Nieobsługiwany format obrazu"));
                return Flow::Skip;
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
                { run(st, "draw-shape", ""); return Flow::Skip; }
            }
            st.shape_preset = if arg.is_empty() { "rect".into() } else { arg.to_string() };
            st.text_shape = base == "text-shape";
            // Kształt pojawia się od razu przy kursorze – potem przesuwasz go i zmieniasz rozmiar uchwytami.
            set_tool(st, 2);
            insert_at_caret(st);
            return Flow::Skip;
        }
        "text-frame" => {
            st.shape_preset = "rect".into();
            st.text_shape = true;
            set_tool(st, 1);
            insert_at_caret(st);
            return Flow::Skip;
        }
        "text-to-shape" => {
            text_to_shape(st, if arg.is_empty() { "ellipse" } else { arg });
            return Flow::Skip;
        }
        "shape-to-text" => {
            shape_to_text(st);
            return Flow::Skip;
        }
        "draw-shape" => {
            st.pen.clear();
            set_tool(st, 3);
            toast(st, tr("Klikaj, aby stawiać węzły (przeciągnij – krzywa). Kliknij pierwszy węzeł albo Enter, aby zamknąć."));
            return Flow::Skip;
        }
        "path-text" => {
            set_tool(st, 4);
            toast(st, tr("Przeciągnij na stronie – powstanie łuk z tekstem"));
            return Flow::Skip;
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
                return Flow::Skip;
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
            let Some(id) = st.ed.obj else { return Flow::Skip };
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
            let Some((_, row, col)) = st.ctx_cell else { return Flow::Skip };
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
        "ruler-v-toggle" => {
            st.prefs.ruler_v = !st.prefs.ruler_v;
            st.prefs.save();
        }
        "line-numbers" => {
            let n = arg.parse::<u8>().unwrap_or(0).min(100);
            st.ed.begin(Kind::Other);
            st.ed.doc.settings.line_numbers = n;
            st.ed.changed();
            st.view.invalidate();
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
            return Flow::Skip;
        }
        "f-step" => {
            let (k, d) = arg.split_once(';').unwrap_or(("", "0"));
            let d: f32 = d.parse().unwrap_or(0.0);
            match k {
                "tracking" => app.set_f_tracking(num(parse_num(&app.get_f_tracking()).unwrap_or(0.0) + d * 0.5).into()),
                "scale" => app.set_f_scale(num((parse_num(&app.get_f_scale()).unwrap_or(100.0) + d * 5.0).clamp(10.0, 400.0)).into()),
                _ => app.set_f_baseline(num(parse_num(&app.get_f_baseline()).unwrap_or(0.0) + d * 0.5).into()),
            }
            return Flow::Skip;
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
            return Flow::Skip;
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
            return Flow::Skip;
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
            // Styl: none, single, double, dashed, dotted; bez argumentu – przełącznik.
            st.ed.begin(Kind::Other);
            let p = &mut st.ed.doc.page;
            let kind = if arg.is_empty() { if p.border.is_some() { "none" } else { "single" } } else { arg };
            if kind == "none" {
                p.border = None;
                p.border_double = false;
            } else {
                let mut b = p.border.clone().unwrap_or(StrokeStyle { color: Color::rgb(0x40, 0x40, 0x40), width: 1.5, dash: vec![] });
                b.dash = match kind {
                    "dashed" => vec![b.width * 4.0, b.width * 2.5],
                    "dotted" => vec![b.width, b.width * 1.8],
                    _ => vec![],
                };
                p.border_double = kind == "double";
                p.border = Some(b);
            }
            st.ed.changed();
        }
        "page-border-w" | "page-border-inset" => {
            let p = st.ed.doc.page.clone();
            let Some(b) = p.border.clone() else {
                toast(st, tr("Najpierw wybierz styl obramowania"));
                return Flow::Skip;
            };
            let cur = if base == "page-border-w" { b.width } else { p.border_inset };
            let step = if base == "page-border-w" { 0.5 } else { 2.0 };
            if let Some(v) = value_or_step(arg, cur, step, is_step) {
                st.ed.begin(Kind::Other);
                let pg = &mut st.ed.doc.page;
                if base == "page-border-w" {
                    let v = v.clamp(0.25, 12.0);
                    let mut nb = b;
                    // Kreski i kropki skalują się z grubością linii.
                    let k = v / nb.width.max(0.01);
                    nb.dash.iter_mut().for_each(|d| *d *= k);
                    nb.width = v;
                    pg.border = Some(nb);
                } else {
                    pg.border_inset = v.clamp(0.0, 72.0);
                }
                st.ed.changed();
            }
        }
        "page-border-color" => {
            let c = color_of(app.get_page_border_color());
            st.ed.begin(Kind::Other);
            let pg = &mut st.ed.doc.page;
            let mut b = pg.border.clone().unwrap_or(StrokeStyle { color: c, width: 1.5, dash: vec![] });
            b.color = c;
            pg.border = Some(b);
            st.ed.changed();
        }
        "open-url" => {
            let _ = open::that(arg);
            return Flow::Skip;
        }
        "symbol" => {
            st.ed.insert(arg);
            *scroll = true;
        }
        "field" => {
            let f = match arg {
                "page-number" => Some(Field::PageNumber),
                "page-count" => Some(Field::PageCount),
                "date" => Some(Field::Date),
                "time" => Some(Field::Time),
                "title" => Some(Field::Title),
                "author" => Some(Field::Author),
                "subject" => Some(Field::Meta(MetaKey::Subject)),
                "keywords" => Some(Field::Meta(MetaKey::Keywords)),
                "created" => Some(Field::Meta(MetaKey::Created)),
                "modified" => Some(Field::Meta(MetaKey::Modified)),
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
            *scroll = true;
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
        "refs-update" => {
            // Spis treści i indeks (jeśli są) – numery stron, podpisy i odsyłacze liczą się przy składzie.
            let has_toc = st.ed.doc.body.paras.iter().any(|p| p.style.starts_with("toc"));
            let has_idx = st.ed.doc.body.paras.iter().any(|p| p.style.starts_with("index"));
            if has_toc {
                run(st, "toc-update", "");
            }
            if has_idx {
                run(st, "index-update", "");
            }
            // Cytowania, bibliografia i spisy ilustracji.
            crate::tools::refresh_refs(st);
            st.ed.relayout();
            st.view.invalidate();
            toast(st, tr("Zaktualizowano spisy, indeks, bibliografię, cytowania, numery podpisów i odsyłacze"));
        }
        "toc" | "toc-update" => {
            let paras = opendtp_core::layout::toc(&st.ed.doc, &st.ed.layout);
            if paras.is_empty() {
                toast(st, tr("Brak nagłówków – nadaj akapitom style „Nagłówek 1–3”"));
                return Flow::Skip;
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
            *scroll = true;
        }
        "bookmark" => {
            if st.ed.story != StoryRef::Body {
                toast(st, tr("Zakładki i hasła indeksu wstawia się w treści głównej"));
                return Flow::Skip;
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
            let Ok(id) = id.parse::<u64>() else { return Flow::Skip };
            let kind = match kind {
                "number" => RefKind::Number,
                "text" => RefKind::Text,
                _ => RefKind::Page,
            };
            insert_field_at_caret(st, Field::Ref(id, kind));
            *scroll = true;
        }
        "index-mark" => {
            if st.ed.story != StoryRef::Body {
                toast(st, tr("Zakładki i hasła indeksu wstawia się w treści głównej"));
                return Flow::Skip;
            }
            let sel: String = st.ed.fragment().iter().map(|p| p.text().replace(FIELD_CHAR, "")).collect::<Vec<_>>().join(" ");
            let sel = sel.trim().to_string();
            if sel.is_empty() {
                toast(st, tr("Zaznacz wyraz albo wyrażenie, które ma trafić do indeksu"));
                return Flow::Skip;
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
                return Flow::Skip;
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
                return Flow::Skip;
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
                    return Flow::Skip;
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
            *scroll = true;
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
                    return Flow::Skip;
                }
            }
            st.ed.edit_obj(&move |o| o.repeat = r);
        }

        _ => return Flow::Unhandled,
    }
    Flow::Done
}
