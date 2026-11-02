//! Polecenia recenzji i widoku: komentarze, wersje, powiększenie, panele.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle(st: &mut State, ui: &crate::AppWindow, app: &App, unit: i32, uk: f32, is_step: bool, base: &str, id: &str, arg: &str, scroll: &mut bool) -> Flow {
    let _ = (ui, unit, uk, is_step, id, &scroll);
    match base {
        // ---------------------------------------------------------- recenzja
        "spell-next" => {
            if !opendtp_core::spell::ready() {
                toast(st, tr("Słowniki jeszcze się wczytują…"));
                return Flow::Skip;
            }
            if spell_next(st) {
                *scroll = true;
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
            return Flow::Skip;
        }
        "user-dict" => {
            let words: Vec<slint::SharedString> = opendtp_core::spell::user_words().into_iter().map(Into::into).collect();
            app.set_user_words(ModelRc::new(VecModel::from(words)));
            app.set_left_panel("dict".into());
            app.set_backstage(false);
            return Flow::Skip;
        }
        "dict-remove" => {
            opendtp_core::spell::remove_user_word(arg);
            st.view.invalidate();
            { run(st, "user-dict", ""); return Flow::Skip; }
        }

        // ---------------------------------------------------------- widok
        "zoom" => {
            let z = match arg {
                "width" => st.view.fit_width(&st.ed),
                "page" => st.view.fit_page(&st.ed),
                v => parse_num(v).map(|p| p / 100.0).unwrap_or(1.0),
            };
            zoom_smooth(st, z, None);
            return Flow::Skip;
        }
        "zoom-in" => {
            let z = zoom_target(st) * 1.15;
            zoom_smooth(st, z, None);
            return Flow::Skip;
        }
        "zoom-out" => {
            let z = zoom_target(st) / 1.15;
            zoom_smooth(st, z, None);
            return Flow::Skip;
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
                return Flow::Skip;
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
            return Flow::Skip;
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
            return Flow::Skip;
        }
        "lib-insert" => {
            let doc = match opendtp_core::library::load(std::path::Path::new(arg)) {
                Ok(d) => d,
                Err(e) => {
                    toast(st, &e);
                    return Flow::Skip;
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
                *scroll = true;
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
            return Flow::Skip;
        }
        "preflight" => {
            let cur = app.get_left_panel();
            app.set_left_panel(if cur == "preflight" && arg.is_empty() { "".into() } else { "preflight".into() });
            crate::preflight_list(&app, &st.ed);
        }
        "preflight-goto" => {
            if let Some(id) = arg.strip_prefix("obj:") {
                let id = id.to_string();
                { run(st, "image-goto", &id); return Flow::Skip; }
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
                return Flow::Skip;
            }
            let n = new.len();
            st.ed.begin(Kind::Other);
            st.ed.doc.swatches.extend(new);
            st.ed.changed();
            toast(st, &opendtp_core::trf!("Dodano kolory: {}", n));
        }
        "swatch-remove" | "swatch-spot" | "swatch-name" | "swatch-cmyk" => {
            let (i, val) = arg.split_once(':').unwrap_or((arg, ""));
            let Ok(i) = i.parse::<usize>() else { return Flow::Skip };
            if i >= st.ed.doc.swatches.len() {
                return Flow::Skip;
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
        "track-toggle" => {
            st.ed.doc.track = !st.ed.doc.track;
            st.ed.dirty = true;
            toast(st, if st.ed.doc.track { tr("Śledzenie zmian włączone – nowy tekst i usunięcia są oznaczane") } else { tr("Śledzenie zmian wyłączone") });
        }
        "rev-accept" | "rev-reject" => {
            if !st.ed.resolve_rev_here(base == "rev-accept") {
                toast(st, tr("W miejscu kursora nie ma śledzonej zmiany – „Następna” przechodzi do kolejnej"));
            }
        }
        "rev-accept-all" | "rev-reject-all" => {
            let n = st.ed.doc.rev_count();
            if n == 0 {
                toast(st, tr("Brak śledzonych zmian"));
                return Flow::Skip;
            }
            st.ed.begin(Kind::Other);
            st.ed.doc.resolve_revs(base == "rev-accept-all");
            st.ed.fix_selection();
            st.ed.changed();
            toast(st, &if base == "rev-accept-all" { opendtp_core::trf!("Zaakceptowano zmiany: {}", n) } else { opendtp_core::trf!("Odrzucono zmiany: {}", n) });
        }
        "rev-next" => {
            if !st.ed.next_rev() {
                toast(st, tr("Brak śledzonych zmian"));
            }
            *scroll = true;
        }
        "comment" => {
            if st.ed.story != StoryRef::Body {
                toast(st, tr("Komentarze wstawia się w treści głównej"));
                return Flow::Skip;
            }
            app.set_comment_text("".into());
            app.set_dialog("comment".into());
            return Flow::Skip;
        }
        "comment-apply" => {
            app.set_dialog("".into());
            let text = arg.trim().to_string();
            if text.is_empty() {
                return Flow::Skip;
            }
            let quote: String = opendtp_core::edit::fragment_text(&st.ed.fragment()).replace(FIELD_CHAR, "").chars().take(80).collect();
            let id = st.ed.doc.new_id();
            let author = std::env::var("USERNAME").unwrap_or_default();
            st.ed.begin(Kind::Other);
            st.ed.doc.comments.push(Comment { id, author, date: opendtp_core::util::iso_now(), text, quote: quote.trim().to_string(), resolved: false, replies: Vec::new() });
            let (_, b) = st.ed.range();
            st.ed.anchor = b;
            st.ed.head = b;
            insert_field_at_caret(st, Field::Comment(id));
            app.set_left_panel("comments".into());
        }
        "comment-reply" | "comment-resolve" | "comment-delete" | "comment-goto" => {
            let (id, text) = arg.split_once(':').unwrap_or((arg, ""));
            let Ok(id) = id.parse::<u64>() else { return Flow::Skip };
            match base {
                "comment-goto" => {
                    let pos = st.ed.doc.body.paras.iter().enumerate().find_map(|(pi, p)| {
                        let mut off = 0;
                        for r in &p.runs {
                            if r.field == Some(Field::Comment(id)) {
                                return Some(At::new(pi, off));
                            }
                            off += r.text.len();
                        }
                        None
                    });
                    if let Some(at) = pos {
                        st.ed.obj = None;
                        st.ed.story = StoryRef::Body;
                        st.ed.anchor = at;
                        st.ed.head = at;
                        *scroll = true;
                    }
                }
                _ => {
                    st.ed.begin(Kind::Other);
                    match base {
                        "comment-reply" => {
                            if let Some(c) = st.ed.doc.comments.iter_mut().find(|c| c.id == id) {
                                if !text.trim().is_empty() {
                                    c.replies.push(Reply { author: std::env::var("USERNAME").unwrap_or_default(), date: opendtp_core::util::iso_now(), text: text.trim().to_string() });
                                }
                            }
                        }
                        "comment-resolve" => {
                            if let Some(c) = st.ed.doc.comments.iter_mut().find(|c| c.id == id) {
                                c.resolved = !c.resolved;
                            }
                        }
                        _ => {
                            st.ed.doc.comments.retain(|c| c.id != id);
                            for p in st.ed.doc.body.paras.iter_mut() {
                                p.runs.retain(|r| r.field != Some(Field::Comment(id)));
                                if p.runs.is_empty() {
                                    p.runs.push(Run::default());
                                }
                            }
                            st.ed.fix_selection();
                        }
                    }
                    st.ed.changed();
                }
            }
        }
        "version-save" => {
            app.set_ver_name(opendtp_core::trf!("Wersja {}", st.ed.doc.versions.len() + 1).into());
            app.set_dialog("version".into());
            return Flow::Skip;
        }
        "version-save-apply" => {
            app.set_dialog("".into());
            match io::odtp::snapshot(&st.ed.doc) {
                Ok(json) => {
                    let id = st.ed.doc.new_id();
                    let name = if arg.trim().is_empty() { opendtp_core::trf!("Wersja {}", st.ed.doc.versions.len() + 1) } else { arg.trim().to_string() };
                    st.ed.doc.version_data.insert(id, std::sync::Arc::new(json));
                    st.ed.doc.versions.push(Version { id, name: name.clone(), date: opendtp_core::util::iso_now() });
                    st.ed.dirty = true;
                    st.ed.version += 1;
                    toast(st, &opendtp_core::trf!("Zapisano wersję „{}” – trafi do pliku przy zapisie dokumentu", name));
                }
                Err(e) => toast(st, &e),
            }
        }
        "version-restore" => {
            let Ok(id) = arg.parse::<u64>() else { return Flow::Skip };
            // Bieżący stan najpierw jako wersja – przywrócenie da się cofnąć także z historii.
            if let Ok(json) = io::odtp::snapshot(&st.ed.doc) {
                let nid = st.ed.doc.new_id();
                st.ed.doc.version_data.insert(nid, std::sync::Arc::new(json));
                st.ed.doc.versions.push(Version { id: nid, name: tr("Przed przywróceniem").to_string(), date: opendtp_core::util::iso_now() });
            }
            match io::odtp::restore(&st.ed.doc, id) {
                Ok(d) => {
                    st.ed.begin(Kind::Other);
                    st.ed.doc = d;
                    st.ed.obj = None;
                    st.ed.story = StoryRef::Body;
                    st.ed.fix_selection();
                    st.ed.changed();
                    st.view.invalidate();
                    toast(st, tr("Przywrócono wersję (Ctrl+Z cofa)"));
                }
                Err(e) => toast(st, &e),
            }
        }
        "version-delete" => {
            let Ok(id) = arg.parse::<u64>() else { return Flow::Skip };
            st.ed.doc.versions.retain(|v| v.id != id);
            st.ed.doc.version_data.remove(&id);
            st.ed.dirty = true;
            st.ed.version += 1;
        }
        "chapter-move" => {
            let (p, dir) = arg.split_once(':').unwrap_or((arg, "down"));
            let Ok(p) = p.parse::<usize>() else { return Flow::Skip };
            st.ed.begin(Kind::Other);
            let moved = st.ed.doc.move_chapter(p, dir == "up");
            match moved {
                Some(np) => {
                    st.ed.obj = None;
                    st.ed.story = StoryRef::Body;
                    st.ed.anchor = At::new(np, 0);
                    st.ed.head = At::new(np, 0);
                    st.ed.changed();
                    *scroll = true;
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
            return Flow::Skip;
        }
        "accent" => {
            if Color::parse(arg).is_some() {
                st.prefs.accent = arg.to_string();
                st.prefs.save();
                crate::apply_theme_prefs(st, &ui);
            } else {
                toast(st, tr("Nieprawidłowy kolor – wpisz np. #5b8cff"));
            }
            return Flow::Skip;
        }
        "corners" => {
            st.prefs.corners = arg.parse().unwrap_or(1);
            st.prefs.save();
            crate::apply_theme_prefs(st, &ui);
            return Flow::Skip;
        }
        "transparency" => {
            st.prefs.transparency = (arg.parse::<f32>().unwrap_or(0.0).round() as i32).clamp(0, 60);
            st.prefs.save();
            crate::apply_theme_prefs(st, &ui);
            return Flow::Skip;
        }
        "ui-scale" => {
            st.prefs.scale = arg.parse::<f32>().unwrap_or(1.0).clamp(0.85, 1.35);
            st.prefs.save();
            crate::apply_theme_prefs(st, &ui);
            return Flow::Skip;
        }
        "dense" => {
            st.prefs.dense = !st.prefs.dense;
            st.prefs.save();
            crate::apply_theme_prefs(st, &ui);
            return Flow::Skip;
        }
        "ribbon-collapse" => {
            st.prefs.ribbon_collapsed = !st.prefs.ribbon_collapsed;
            app.set_ribbon_collapsed(st.prefs.ribbon_collapsed);
            app.set_ribbon_peek(false);
            st.prefs.save();
            return Flow::Skip;
        }
        "fullscreen" => {
            let on = !app.get_fullscreen();
            app.set_fullscreen(on);
            crate::win::fullscreen(&ui, on);
            return Flow::Skip;
        }
        "palette" => {
            palette_query(st, "");
            app.set_palette_open(true);
            return Flow::Skip;
        }
        "customize" => {
            if arg == "qat" {
                customize_list(st, "qat");
                app.set_dialog("customize-qat".into());
            } else {
                let i = arg.parse::<usize>().unwrap_or(0);
                if i >= st.prefs.tabs().len() {
                    return Flow::Skip;
                }
                app.set_custom_edit(i as i32);
                app.set_custom_name(st.prefs.tabs()[i].name.clone().into());
                customize_list(st, "tab");
                app.set_dialog("customize-tab".into());
            }
            app.set_backstage(false);
            return Flow::Skip;
        }
        "qat-toggle" | "tab-toggle" => {
            let i = app.get_custom_edit().max(0) as usize;
            let list = if base == "qat-toggle" {
                &mut st.prefs.qat
            } else {
                match st.prefs.tabs_mut().get_mut(i) {
                    Some(t) => &mut t.items,
                    None => return Flow::Skip,
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
            return Flow::Skip;
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
            return Flow::Skip;
        }
        // Karty skrótów: dodanie (od razu okno wyboru poleceń), zmiana nazwy, usunięcie.
        "tab-add" => {
            if st.prefs.tabs().len() >= crate::settings::MAX_TABS {
                return Flow::Skip;
            }
            st.prefs.tabs_mut().push(crate::settings::CustomTab { name: String::new(), items: crate::settings::default_tab() });
            let i = st.prefs.tabs().len() - 1;
            st.prefs.save();
            sync_lists(st);
            app.set_tab(crate::FIXED_TABS + i as i32);
            { run(st, "customize", &i.to_string()); return Flow::Skip; }
        }
        "tab-rename" => {
            if let Some(t) = st.prefs.tabs_mut().get_mut(app.get_custom_edit().max(0) as usize) {
                t.name = arg.trim().to_string();
            }
            st.prefs.save();
            sync_lists(st);
            return Flow::Skip;
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
            return Flow::Skip;
        }

        _ => return Flow::Unhandled,
    }
    Flow::Done
}
