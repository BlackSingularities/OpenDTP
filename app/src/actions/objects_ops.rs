//! Tworzenie i przekształcanie obiektów: wstawianie, obrazy, kody, kształty, ramki.

use super::*;

/// Nowy obiekt wstawiony podczas pisania w treści głównej płynie z tekstem (kotwica w akapicie kursora).
pub fn anchor_new(st: &mut State, id: u64) {
    if st.ed.story != StoryRef::Body {
        return;
    }
    let p = st.ed.head.para;
    st.ed.anchor_at(id, p);
    st.ed.changed();
}
/// Komórka pod punktem (pt) w tabeli obiektu.
/// Pole w miejscu kursora (bieżący tekst), kursor za polem.
pub(super) fn insert_field_at_caret(st: &mut State, f: Field) {
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
pub fn new_obj(page: usize, x: f32, y: f32, w: f32, h: f32, shape: Shape, kind: ObjKind, wrap: WrapMode) -> Object {
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
pub(super) fn selected_asset(st: &State) -> Option<String> {
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
pub(super) fn insert_image(st: &mut State, asset: Asset, link: Option<ImageLink>) {
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
/// Kod QR albo kod kreskowy (Code 128) jako SVG – czarne moduły na białym tle z marginesem ciszy.
pub(super) fn code_svg(qr: bool, text: &str) -> Option<String> {
    let mut rects = String::new();
    let (w, h) = if qr {
        let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
        let n = code.width();
        let colors = code.to_colors();
        for y in 0..n {
            let mut x = 0;
            while x < n {
                if colors[y * n + x] == qrcode::Color::Dark {
                    let s = x;
                    while x < n && colors[y * n + x] == qrcode::Color::Dark {
                        x += 1;
                    }
                    rects.push_str(&format!(r#"<rect x="{}" y="{}" width="{}" height="1"/>"#, s + 4, y + 4, x - s));
                } else {
                    x += 1;
                }
            }
        }
        (n + 8, n + 8)
    } else {
        // Zestaw znaków B („Ɓ” na początku) – litery, cyfry i interpunkcja ASCII.
        if !text.is_ascii() {
            return None;
        }
        let bars = barcoders::sym::code128::Code128::new(format!("Ɓ{text}")).ok()?.encode();
        let n = bars.len();
        let mut x = 0;
        while x < n {
            if bars[x] == 1 {
                let s = x;
                while x < n && bars[x] == 1 {
                    x += 1;
                }
                rects.push_str(&format!(r#"<rect x="{}" y="0" width="{}" height="60"/>"#, s + 10, x - s));
            } else {
                x += 1;
            }
        }
        (n + 20, 60)
    };
    let k = if qr { 8 } else { 2 };
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {w} {h}" shape-rendering="crispEdges"><rect width="{w}" height="{h}" fill="#fff"/><g fill="#000">{rects}</g></svg>"##,
        w * k,
        h * k
    ))
}
/// Akapity strony tytułowej. Tytuł, temat i autor to pola metadanych (zmienisz je w Informacjach);
/// gdy metadana jest pusta, wstawiamy tekst do nadpisania.
pub(super) fn cover_paras(doc: &Document, variant: &str) -> Vec<Paragraph> {
    let accent = doc.style("heading1").and_then(|s| s.chars.color).unwrap_or(Color::rgb(0x1f, 0x4e, 0x79));
    let m = &doc.meta;
    let line = |style: &str, field: Option<Field>, placeholder: &str, size: f32, color: Option<Color>, bold: bool| {
        let fmt = CharFmt { size: Some(size), color, bold: bold.then_some(true), ..Default::default() };
        let mut s = Story { paras: vec![Paragraph::with_text(style, "")] };
        match field {
            Some(f) => {
                opendtp_core::edit::insert_field(&mut s, At::new(0, 0), f, &fmt);
            }
            None => {
                s.paras[0].runs = vec![Run { text: placeholder.to_string(), fmt, field: None }];
            }
        }
        s.paras.remove(0)
    };
    let pick = |v: &str, f: Field| (!v.is_empty()).then_some(f);
    let mut title = line("title", pick(&m.title, Field::Title), &tr("Tytuł dokumentu"), 36.0, Some(accent), true);
    let mut sub = line("subtitle", pick(&m.subject, Field::Meta(MetaKey::Subject)), &tr("Podtytuł lub temat"), 16.0, None, false);
    let mut author = line("normal", pick(&m.author, Field::Author), &tr("Autor"), 13.0, None, true);
    let mut date = line("normal", None, &opendtp_core::util::today(), 11.0, Some(Color::rgb(0x60, 0x60, 0x60)), false);
    match variant {
        // Pas w kolorze akcentu z białym tytułem u góry strony.
        "band" => {
            for p in [&mut title, &mut sub] {
                p.fmt.shading = Some(accent);
                p.fmt.indent_left = Some(12.0);
                p.fmt.indent_right = Some(12.0);
                for r in p.runs.iter_mut() {
                    r.fmt.color = Some(Color::WHITE);
                }
            }
            title.fmt.space_before = Some(0.0);
            title.fmt.space_after = Some(0.0);
            sub.fmt.space_after = Some(220.0);
            author.fmt.space_before = Some(24.0);
            vec![title, sub, author, date]
        }
        // Tytuł nisko na stronie, wyrównany do lewej.
        "minimal" => {
            title.fmt.space_before = Some(380.0);
            sub.fmt.space_after = Some(30.0);
            for p in [&mut title, &mut sub, &mut author, &mut date] {
                p.fmt.align = Some(Align::Left);
            }
            vec![title, sub, author, date]
        }
        // Klasyczna: wszystko wyśrodkowane, tytuł w jednej trzeciej wysokości.
        _ => {
            title.fmt.space_before = Some(200.0);
            sub.fmt.space_after = Some(200.0);
            for p in [&mut title, &mut sub, &mut author, &mut date] {
                p.fmt.align = Some(Align::Center);
            }
            vec![title, sub, author, date]
        }
    }
}
/// Zwykły tekst → kształt tekstu: zaznaczenie (albo cały bieżący akapit) wyjmujemy z treści
/// i wkładamy do kształtu w tym samym miejscu strony; tekst opływa go z obu stron.
pub(super) fn text_to_shape(st: &mut State, preset: &str) {
    if st.ed.story != StoryRef::Body {
        toast(st, tr("Zaznacz tekst w treści dokumentu (poza ramkami)"));
        return;
    }
    if !st.ed.has_sel() {
        let p = st.ed.head.para;
        let len = st.ed.doc.body.paras.get(p).map(|x| x.len()).unwrap_or(0);
        if len == 0 {
            toast(st, tr("Zaznacz tekst albo ustaw kursor w akapicie, który ma się stać kształtem"));
            return;
        }
        st.ed.anchor = At::new(p, 0);
        st.ed.head = At::new(p, len);
    }
    let (a, _) = st.ed.range();
    let Some(c) = st.ed.layout.caret(StoryRef::Body, a.para, a.off) else { return };
    let frag = st.ed.fragment();
    let chars: usize = frag.iter().map(|p| p.text().chars().count()).sum();
    let size = st.ed.caret_props().0.size.max(6.0);
    let ps = st.ed.layout.setup(&st.ed.doc, c.page).clone();
    let shape = geom::preset(preset).unwrap_or_else(Shape::rect);
    // Rozmiar z ilości tekstu: kształty inne niż prostokąt mają mniej miejsca w środku.
    let k = if preset == "rect" || preset == "rounded" { 1.0 } else { 1.45 };
    let w = (ps.content_width() * 0.55).max(140.0);
    let lines = ((chars as f32 * size * 0.5) / (w - 16.0)).ceil().max(1.0) + frag.len() as f32 - 1.0;
    let h = ((lines * size * 1.3 * k * k) + 16.0).clamp(60.0, ps.height - ps.margin_top - ps.margin_bottom);
    let w = w * k.sqrt();
    let x = (ps.width - w) / 2.0;
    let y = c.top.min(ps.height - ps.margin_bottom - h).max(ps.margin_top);
    st.ed.delete_sel();
    let mut o = new_obj(c.page, x, y, w, h, shape, ObjKind::Text { story: Story { paras: frag }, padding: 8.0, columns: 1, gap: 12.0, valign: VAlign::Center, next: None }, WrapMode::Both);
    o.wrap.dist = 10.0;
    let id = st.ed.add_object(o);
    anchor_new(st, id);
    st.ed.obj = Some(id);
    object_selected(st);
    refresh(st);
    toast(st, tr("Tekst jest teraz kształtem – zmień kształt węzłami, dodaj wypełnienie albo obrys"));
}
/// Kształt tekstu → zwykły tekst: akapity wracają do treści za akapitem kotwicy, kształt znika.
pub(super) fn shape_to_text(st: &mut State) {
    let Some(o) = st.ed.object().cloned() else {
        toast(st, tr("Zaznacz kształt albo ramkę z tekstem"));
        return;
    };
    let Some(story) = o.story().cloned() else {
        toast(st, tr("Ten obiekt nie zawiera tekstu"));
        return;
    };
    let paras: Vec<Paragraph> = story.paras.into_iter().filter(|p| !p.is_empty()).collect();
    if paras.is_empty() {
        st.ed.delete_object();
        refresh(st);
        return;
    }
    let anchor = st.ed.doc.body.paras.iter().position(|p| p.runs.iter().any(|r| r.field == Some(Field::Anchor(o.id))));
    st.ed.delete_object();
    st.ed.begin(Kind::Other);
    let at = anchor.map(|i| i + 1).unwrap_or(st.ed.doc.body.paras.len()).min(st.ed.doc.body.paras.len());
    let n = paras.len();
    st.ed.doc.body.paras.splice(at..at, paras);
    st.ed.obj = None;
    st.ed.set_caret(StoryRef::Body, At::new(at, 0), false);
    st.ed.set_caret(StoryRef::Body, At::new(at + n - 1, st.ed.doc.body.paras[at + n - 1].len()), true);
    st.ed.changed();
    refresh(st);
}
/// Wstawia obiekt bieżącego narzędzia w domyślnym rozmiarze: wyśrodkowany na stronie kursora,
/// na wysokości kursora (jak tabela).
pub(super) fn insert_at_caret(st: &mut State) {
    let ps = st.ed.doc.page.clone();
    let (page, y) = st.ed.caret_page().map(|c| (c.0, c.2)).unwrap_or((0, ps.margin_top));
    let tool = st.ui.upgrade().map(|u| u.global::<App>().get_tool()).unwrap_or(0);
    let w = if tool == 2 { 160.0 } else { 220.0 };
    let pw = st.ed.layout.page_size(&st.ed.doc, page).0;
    create_from_drag(st, page, (pw - w) / 2.0, y.min(ps.height - ps.margin_bottom - 160.0).max(ps.margin_top), 0.0, 0.0);
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
