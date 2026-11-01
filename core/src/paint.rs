//! Lista rysowania strony – jedna dla ekranu (tiny-skia) i dla PDF (krilla), więc wydruk do PDF
//! wygląda dokładnie jak na ekranie. Elementy pomocnicze (marginesy, kontury ramek, znaki
//! formatowania, podkreślenia pisowni) trafiają tu tylko w trybie ekranowym.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use crate::fonts::FontId;
use crate::geom::{self, Rect, Seg};
use crate::layout::{Layout, Line, PGlyph};
use crate::model::*;
use crate::text::{self, GKind};

#[derive(Clone, Copy, Debug)]
pub enum PathCmd {
    M(f32, f32),
    L(f32, f32),
    C(f32, f32, f32, f32, f32, f32),
    Z,
}

#[derive(Clone, Debug, Default)]
pub struct Path(pub Vec<PathCmd>);

impl Path {
    pub fn rect(r: Rect) -> Path {
        Path(vec![PathCmd::M(r.x, r.y), PathCmd::L(r.right(), r.y), PathCmd::L(r.right(), r.bottom()), PathCmd::L(r.x, r.bottom()), PathCmd::Z])
    }

    pub fn round_rect(r: Rect, rad: f32) -> Path {
        let rad = rad.min(r.w / 2.0).min(r.h / 2.0);
        if rad <= 0.01 {
            return Path::rect(r);
        }
        let k = rad * 0.447_715_2;
        let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
        Path(vec![
            PathCmd::M(x0 + rad, y0),
            PathCmd::L(x1 - rad, y0),
            PathCmd::C(x1 - k, y0, x1, y0 + k, x1, y0 + rad),
            PathCmd::L(x1, y1 - rad),
            PathCmd::C(x1, y1 - k, x1 - k, y1, x1 - rad, y1),
            PathCmd::L(x0 + rad, y1),
            PathCmd::C(x0 + k, y1, x0, y1 - k, x0, y1 - rad),
            PathCmd::L(x0, y0 + rad),
            PathCmd::C(x0, y0 + k, x0 + k, y0, x0 + rad, y0),
            PathCmd::Z,
        ])
    }

    pub fn line(x0: f32, y0: f32, x1: f32, y1: f32) -> Path {
        Path(vec![PathCmd::M(x0, y0), PathCmd::L(x1, y1)])
    }

    pub fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Path {
        let s = shape_path(&geom::ellipse(), Rect::new(cx - rx, cy - ry, rx * 2.0, ry * 2.0), 0.0);
        s
    }
}

/// Ścieżka kształtu obiektu w punktach strony.
pub fn shape_path(s: &Shape, r: Rect, corner: f32) -> Path {
    if corner > 0.0 && s.is_rect() {
        return Path::round_rect(r, corner);
    }
    let mut p = Vec::new();
    for c in &s.contours {
        let segs = geom::contour_segments(c, r);
        for (i, sg) in segs.iter().enumerate() {
            match *sg {
                Seg::Line(a, b) => {
                    if i == 0 {
                        p.push(PathCmd::M(a.x, a.y));
                    }
                    p.push(PathCmd::L(b.x, b.y));
                }
                Seg::Cubic(a, b, c2, d) => {
                    if i == 0 {
                        p.push(PathCmd::M(a.x, a.y));
                    }
                    p.push(PathCmd::C(b.x, b.y, c2.x, c2.y, d.x, d.y));
                }
            }
        }
        if c.closed && !segs.is_empty() {
            p.push(PathCmd::Z);
        }
    }
    Path(p)
}

#[derive(Clone, Debug)]
pub enum Paint {
    Solid(Color),
    Linear { x0: f32, y0: f32, x1: f32, y1: f32, from: Color, to: Color },
    Radial { cx: f32, cy: f32, r: f32, from: Color, to: Color },
}

#[derive(Clone, Debug)]
pub struct GlyphRun {
    pub font: FontId,
    pub size: f32,
    pub color: Color,
    pub fake_bold: bool,
    pub fake_italic: bool,
    /// Grubość obrysu (0 – zwykłe wypełnienie).
    pub outline: f32,
    /// (glif, x, y linii bazowej)
    pub glyphs: Vec<(u16, f32, f32)>,
    pub text: String,
    pub ranges: Vec<Range<usize>>,
    /// Obrót pojedynczego glifu (tekst na ścieżce), radiany.
    pub rot: f32,
    pub link: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Op {
    Fill { path: Path, paint: Paint, opacity: f32 },
    Stroke { path: Path, color: Color, width: f32, dash: Vec<f32>, opacity: f32 },
    Glyphs(GlyphRun),
    Image { asset: String, rect: Rect, clip: Option<Path>, opacity: f32 },
}

pub struct PaintOpts<'a> {
    /// Elementy tylko na ekran.
    pub screen: bool,
    pub marks: bool,
    pub guides: bool,
    /// Sprawdzanie pisowni: (słowo, język) → czy błędne.
    pub misspelled: Option<&'a dyn Fn(&str, &str) -> bool>,
    pub hide_obj: Option<u64>,
}

const PALETTE: [Color; 8] = [
    Color::rgb(0x44, 0x72, 0xc4),
    Color::rgb(0xed, 0x7d, 0x31),
    Color::rgb(0xa5, 0xa5, 0xa5),
    Color::rgb(0xff, 0xc0, 0x00),
    Color::rgb(0x5b, 0x9b, 0xd5),
    Color::rgb(0x70, 0xad, 0x47),
    Color::rgb(0x26, 0x44, 0x78),
    Color::rgb(0x9e, 0x48, 0x0e),
];

pub fn series_color(ch: &Chart, i: usize) -> Color {
    ch.series.get(i).and_then(|s| s.color).unwrap_or(PALETTE[i % PALETTE.len()])
}

fn obj_fill(o: &Object) -> Option<Paint> {
    let r = Rect::new(o.x, o.y, o.w, o.h);
    Some(match o.fill.as_ref()? {
        Fill::Solid { color } => Paint::Solid(*color),
        Fill::Linear { from, to, angle } => {
            let a = angle.to_radians();
            let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
            let half = (r.w * a.cos().abs() + r.h * a.sin().abs()) / 2.0;
            Paint::Linear { x0: cx - a.cos() * half, y0: cy - a.sin() * half, x1: cx + a.cos() * half, y1: cy + a.sin() * half, from: *from, to: *to }
        }
        Fill::Radial { from, to } => Paint::Radial { cx: r.x + r.w / 2.0, cy: r.y + r.h / 2.0, r: r.w.max(r.h) / 2.0, from: *from, to: *to },
    })
}

/// Lista rysowania strony.
pub fn page_ops(doc: &Document, layout: &Layout, page: usize, o: &PaintOpts) -> Vec<Op> {
    let mut ops = Vec::new();
    let ps = layout.setup(doc, page);
    ops.push(Op::Fill { path: Path::rect(Rect::new(0.0, 0.0, ps.width, ps.height)), paint: Paint::Solid(ps.color), opacity: 1.0 });
    if let Some(b) = &ps.border {
        if b.width > 0.0 {
            let i = ps.border_inset;
            ops.push(Op::Stroke { path: Path::rect(Rect::new(i, i, ps.width - 2.0 * i, ps.height - 2.0 * i)), color: b.color, width: b.width, dash: b.dash.clone(), opacity: 1.0 });
            if ps.border_double {
                // Druga linia do środka – odstęp jak dwie grubości.
                let j = i + b.width * 3.0;
                ops.push(Op::Stroke { path: Path::rect(Rect::new(j, j, ps.width - 2.0 * j, ps.height - 2.0 * j)), color: b.color, width: b.width, dash: b.dash.clone(), opacity: 1.0 });
            }
        }
    }
    if !ps.watermark.trim().is_empty() {
        watermark_ops(&ps.watermark, ps, &mut ops);
    }
    let Some(info) = layout.pages.get(page) else { return ops };
    let objs: Vec<&Object> = doc.objects.iter().filter(|ob| ob.on_page(page, doc.page.first_number) && Some(ob.id) != o.hide_obj).collect();

    for ob in objs.iter().filter(|ob| ob.wrap.mode == WrapMode::Behind) {
        object_ops(doc, layout, ob, page, o, &mut ops);
    }
    if o.guides && o.screen {
        let c = Color::rgb(0x9a, 0xb4, 0xd8).with_alpha(110);
        let r = Rect::new(ps.margin_left, ps.margin_top, ps.content_width(), ps.height - ps.margin_top - ps.margin_bottom);
        ops.push(Op::Stroke { path: Path::rect(r), color: c, width: 0.5, dash: vec![2.0, 2.0], opacity: 1.0 });
        if ps.columns > 1 {
            for (x0, x1) in ps.column_spans() {
                ops.push(Op::Stroke { path: Path::rect(Rect::new(x0, r.y, x1 - x0, r.h)), color: c, width: 0.4, dash: vec![1.0, 3.0], opacity: 1.0 });
            }
        }
    }
    if ps.columns > 1 && ps.column_rule {
        let spans = ps.column_spans();
        for w in spans.windows(2) {
            let x = (w[0].1 + w[1].0) / 2.0;
            ops.push(Op::Stroke { path: Path::line(x, ps.margin_top, x, ps.height - ps.margin_bottom), color: Color::rgb(0xb0, 0xb0, 0xb0), width: 0.6, dash: vec![], opacity: 1.0 });
        }
    }
    let free: Vec<&Line> = info.lines.iter().map(|&i| &layout.lines[i]).filter(|l| l.obj.is_none()).collect();
    lines_ops(doc, &free, o, &mut ops);
    // Numeracja wierszy treści głównej – szare numery przed lewą krawędzią tekstu, od 1 na stronie.
    let step = doc.settings.line_numbers as u32;
    if step > 0 {
        let mut n = 0u32;
        for l in free.iter().filter(|l| l.story == StoryRef::Body) {
            n += 1;
            if n % step == 0 || step == 1 {
                let y = l.glyphs.first().map(|g| g.y).unwrap_or(l.top + l.height * 0.8);
                label(&mut ops, &n.to_string(), l.area.0 - 10.0, y, 8.0, Color::rgb(0x8a, 0x8a, 0x8a), "Calibri", 1.0, false);
            }
        }
    }
    if let Some((y, x, w)) = info.note_rule {
        ops.push(Op::Stroke { path: Path::line(x, y, x + w, y), color: Color::rgb(0x80, 0x80, 0x80), width: 0.6, dash: vec![], opacity: 1.0 });
    }
    for ob in objs.iter().filter(|ob| ob.wrap.mode != WrapMode::Behind) {
        object_ops(doc, layout, ob, page, o, &mut ops);
    }
    // Komentarze (tylko na ekranie): żółty dymek w miejscu komentarza.
    if o.screen && doc.comments.iter().any(|c| !c.resolved) {
        for (pi, p) in doc.body.paras.iter().enumerate() {
            let mut off = 0;
            for r in &p.runs {
                if let Some(Field::Comment(id)) = r.field {
                    let open = doc.comments.iter().any(|c| c.id == id && !c.resolved);
                    if let Some(c) = layout.caret(crate::model::StoryRef::Body, pi, off).filter(|c| c.page == page && open) {
                        let (x, y) = (c.x - 1.0, c.top - 5.0);
                        ops.push(Op::Fill { path: Path::round_rect(Rect::new(x, y, 9.0, 7.0), 1.5), paint: Paint::Solid(Color::rgb(0xf2, 0xb8, 0x22)), opacity: 1.0 });
                        let mut tail = Path(Vec::new());
                        tail.0.extend([PathCmd::M(x + 1.5, y + 6.5), PathCmd::L(x + 1.5, y + 9.0), PathCmd::L(x + 4.0, y + 6.5), PathCmd::Z]);
                        ops.push(Op::Fill { path: tail, paint: Paint::Solid(Color::rgb(0xf2, 0xb8, 0x22)), opacity: 1.0 });
                    }
                }
                off += r.text.len();
            }
        }
    }
    // Znaczek obrazu linkowanego (tylko na ekranie): ogniwo łącza, czerwone – brak pliku.
    if o.screen {
        for ob in &objs {
            let ObjKind::Image { asset, .. } = &ob.kind else { continue };
            if !doc.links.contains_key(asset) {
                continue;
            }
            let missing = doc.missing.contains(asset);
            let (cx, cy) = (ob.x + ob.w - 9.0, ob.y + 9.0);
            let bg = if missing { Color::rgb(0xd9, 0x3f, 0x3f) } else { Color::rgb(0x2f, 0x6f, 0xd6) };
            ops.push(Op::Fill { path: Path::ellipse(cx, cy, 6.5, 6.5), paint: Paint::Solid(bg), opacity: 0.92 });
            ops.push(Op::Stroke { path: Path::round_rect(Rect::new(cx - 4.2, cy - 1.6, 4.6, 3.2), 1.6), color: Color::WHITE, width: 0.9, dash: vec![], opacity: 1.0 });
            ops.push(Op::Stroke { path: Path::round_rect(Rect::new(cx - 0.4, cy - 1.6, 4.6, 3.2), 1.6), color: Color::WHITE, width: 0.9, dash: vec![], opacity: 1.0 });
        }
    }
    ops
}

fn object_ops(doc: &Document, layout: &Layout, ob: &Object, page: usize, o: &PaintOpts, ops: &mut Vec<Op>) {
    let r = Rect::new(ob.x, ob.y, ob.w, ob.h);
    let path = shape_path(&ob.shape, r, ob.corner);
    let closed = ob.shape.contours.iter().any(|c| c.closed);
    if ob.shadow && closed {
        for (i, a) in [(4.0, 18u8), (3.0, 26), (2.0, 34)] {
            let mut p = path.clone();
            offset(&mut p, i, i);
            ops.push(Op::Fill { path: p, paint: Paint::Solid(Color::BLACK.with_alpha(a)), opacity: ob.opacity });
        }
    }
    if closed {
        if let Some(paint) = obj_fill(ob) {
            ops.push(Op::Fill { path: path.clone(), paint, opacity: ob.opacity });
        }
    }
    match &ob.kind {
        ObjKind::Image { asset, fit, .. } => {
            if let Some(a) = doc.assets.get(asset) {
                let (iw, ih) = (a.width.max(1) as f32, a.height.max(1) as f32);
                let rect = match fit {
                    ImageFit::Stretch => r,
                    ImageFit::Cover | ImageFit::Contain => {
                        let k = if *fit == ImageFit::Cover { (r.w / iw).max(r.h / ih) } else { (r.w / iw).min(r.h / ih) };
                        let (w, h) = (iw * k, ih * k);
                        Rect::new(r.x + (r.w - w) / 2.0, r.y + (r.h - h) / 2.0, w, h)
                    }
                };
                let clip = if ob.shape.is_rect() && ob.corner <= 0.0 && *fit != ImageFit::Cover { None } else { Some(path.clone()) };
                ops.push(Op::Image { asset: asset.clone(), rect, clip, opacity: ob.opacity });
            } else if o.screen {
                ops.push(Op::Fill { path: path.clone(), paint: Paint::Solid(Color::rgb(0xe8, 0xe8, 0xe8)), opacity: 1.0 });
            }
        }
        ObjKind::Chart { chart } => chart_ops(chart, r, ops),
        ObjKind::Table { table } => table_ops(table, r, ops),
        _ => {}
    }
    if let Some(s) = &ob.stroke {
        if s.width > 0.0 {
            ops.push(Op::Stroke { path: path.clone(), color: s.color, width: s.width, dash: s.dash.clone(), opacity: ob.opacity });
        }
    } else if o.guides && o.screen && matches!(ob.kind, ObjKind::Text { .. } | ObjKind::PathText { .. }) {
        ops.push(Op::Stroke { path: path.clone(), color: Color::rgb(0x7f, 0x9c, 0xc8).with_alpha(150), width: 0.5, dash: vec![3.0, 2.0], opacity: 1.0 });
    }
    if matches!(ob.kind, ObjKind::Text { .. }) {
        let mine: Vec<&Line> = layout.pages.get(page).map(|p| p.lines.iter().map(|&i| &layout.lines[i]).filter(|l| l.obj == Some(ob.id)).collect()).unwrap_or_default();
        lines_ops(doc, &mine, o, ops);
        if o.screen && layout.overflow.contains(&ob.id) {
            let s = 7.0;
            let b = Rect::new(r.right() - s - 2.0, r.bottom() - s - 2.0, s, s);
            ops.push(Op::Fill { path: Path::rect(b), paint: Paint::Solid(Color::rgb(0xe0, 0x3a, 0x3a)), opacity: 1.0 });
            let (cx, cy) = (b.x + s / 2.0, b.y + s / 2.0);
            ops.push(Op::Stroke { path: Path(vec![PathCmd::M(cx - 2.2, cy), PathCmd::L(cx + 2.2, cy), PathCmd::M(cx, cy - 2.2), PathCmd::L(cx, cy + 2.2)]), color: Color::WHITE, width: 1.0, dash: vec![], opacity: 1.0 });
        }
    }
    if let ObjKind::PathText { story, .. } = &ob.kind {
        if let Some(gs) = layout.path_glyphs.get(&ob.id) {
            let base = story.paras.first().map(|p| doc.resolve_para(p).1).unwrap_or_default();
            for pg in gs {
                let st = story.paras.first().map(|p| {
                    let (ri, _) = p.run_at(pg.g.cluster + 1);
                    let mut c = base.clone();
                    if let Some(r) = p.runs.get(ri) {
                        c.apply(&r.fmt);
                    }
                    c
                }).unwrap_or_else(|| base.clone());
                ops.push(Op::Glyphs(GlyphRun {
                    font: pg.g.font,
                    size: pg.g.size,
                    color: st.color,
                    fake_bold: pg.g.fake_bold,
                    fake_italic: pg.g.fake_italic,
                    outline: st.outline,
                    glyphs: vec![(pg.g.gid, pg.x, pg.y)],
                    text: String::new(),
                    ranges: vec![0..0],
                    rot: pg.angle,
                    link: None,
                }));
            }
        }
    }
}

fn offset(p: &mut Path, dx: f32, dy: f32) {
    for c in p.0.iter_mut() {
        match c {
            PathCmd::M(x, y) | PathCmd::L(x, y) => {
                *x += dx;
                *y += dy;
            }
            PathCmd::C(a, b, c2, d, e, f) => {
                *a += dx;
                *b += dy;
                *c2 += dx;
                *d += dy;
                *e += dx;
                *f += dy;
            }
            PathCmd::Z => {}
        }
    }
}

fn lines_ops(doc: &Document, lines: &[&Line], o: &PaintOpts, ops: &mut Vec<Op>) {
    // Tła i ramki akapitów (sąsiednie wiersze tego samego akapitu razem).
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        let story = doc.story(l.story);
        let para = story.and_then(|s| s.paras.get(l.para));
        let Some(para) = para else {
            i += 1;
            continue;
        };
        let (pp, _) = doc.resolve_para(para);
        let mut j = i + 1;
        while j < lines.len() && lines[j].para == l.para && lines[j].story == l.story && (lines[j].area.0 - l.area.0).abs() < 0.5 {
            j += 1;
        }
        if !pp.shading.is_none() || pp.border.is_some_and(|b| b.width > 0.0) {
            let pad = pp.border.map(|b| b.padding).unwrap_or(2.0);
            let top = l.top - pad;
            let bottom = lines[j - 1].bottom() + pad;
            let r = Rect::new(l.area.0 - pad, top, l.area.1 - l.area.0 + 2.0 * pad, bottom - top);
            let rad = pp.border.map(|b| b.radius).unwrap_or(0.0);
            if !pp.shading.is_none() {
                ops.push(Op::Fill { path: Path::round_rect(r, rad), paint: Paint::Solid(pp.shading), opacity: 1.0 });
            }
            if let Some(b) = pp.border {
                if b.width > 0.0 {
                    ops.push(Op::Stroke { path: Path::round_rect(r, rad), color: b.color, width: b.width, dash: vec![], opacity: 1.0 });
                }
            }
        }
        i = j;
    }
    for l in lines {
        line_ops(doc, l, o, ops);
    }
}

fn line_ops(_doc: &Document, l: &Line, o: &PaintOpts, ops: &mut Vec<Op>) {
    let sp = &l.shaped;
    // Wyróżnienia tła pod tekstem.
    for g in &l.glyphs {
        let st = sp.style(&g.g);
        if !st.highlight.is_none() {
            let (a, d, _) = text::vmetrics(g.g.font, st.size);
            ops.push(Op::Fill { path: Path::rect(Rect::new(g.x, l.baseline - a, g.adv + 0.2, a + d)), paint: Paint::Solid(st.highlight), opacity: 1.0 });
        }
    }
    // Znacznik listy / inicjał.
    if !l.marker.is_empty() {
        let st = l.marker_style.clone().unwrap_or_default();
        push_runs(&l.marker, &st, "", ops, |_| st.clone());
    }
    let styles = &sp.styles;
    let text: &str = &sp.text;
    push_runs(&l.glyphs, &styles[0], text, ops, |si| styles[si].clone());

    // Podkreślenia i przekreślenia (ciągami w jednym stylu).
    let mut k = 0;
    while k < l.glyphs.len() {
        let si = l.glyphs[k].g.style;
        let mut e = k + 1;
        while e < l.glyphs.len() && l.glyphs[e].g.style == si {
            e += 1;
        }
        let st = &styles[si as usize];
        // Bez spacji na końcu wiersza.
        let mut e2 = e;
        while e2 > k && matches!(l.glyphs[e2 - 1].g.kind, GKind::Space | GKind::Newline) && e2 == l.glyphs.len() {
            e2 -= 1;
        }
        if e2 > k && (st.underline != Underline::None || st.strike) {
            let x0 = l.glyphs[k].x;
            let x1 = l.glyphs[e2 - 1].x + l.glyphs[e2 - 1].adv;
            let base = l.glyphs[k].y;
            if let Some(f) = crate::fonts::get(l.glyphs[k].g.font) {
                let s = l.glyphs[k].g.size / f.upem;
                if st.underline != Underline::None {
                    let y = base - f.underline_pos * s + f.underline_thick * s / 2.0;
                    let t = (f.underline_thick * s).max(0.4);
                    deco(ops, st.underline, x0, x1, y, t, st.color);
                }
                if st.strike {
                    let y = base - f.strike_pos * s;
                    let t = (f.strike_thick * s).max(0.4);
                    ops.push(Op::Fill { path: Path::rect(Rect::new(x0, y - t / 2.0, x1 - x0, t)), paint: Paint::Solid(st.color), opacity: 1.0 });
                }
            }
        }
        k = e;
    }
    // Kropki wiodące tabulatorów.
    for g in &l.glyphs {
        if g.leader && g.adv > 8.0 {
            let st = sp.style(&g.g);
            let mut x = g.x + 3.0;
            let mut p = Vec::new();
            while x < g.x + g.adv - 3.0 {
                p.push(PathCmd::M(x, g.y - 0.6));
                p.push(PathCmd::L(x + 0.8, g.y - 0.6));
                x += 3.5;
            }
            ops.push(Op::Stroke { path: Path(p), color: st.color, width: 0.9, dash: vec![], opacity: 1.0 });
        }
    }
    if !o.screen {
        return;
    }
    // Pisownia: falista czerwona linia pod błędnymi wyrazami.
    if let Some(check) = o.misspelled {
        let lo = l.start.min(text.len());
        let hi = l.end.min(text.len());
        if lo < hi {
            for (i, w) in text[lo..hi].split_word_bound_indices() {
                if !w.chars().any(|c| c.is_alphabetic()) || w.chars().any(|c| c.is_ascii_digit()) || w.contains(FIELD_CHAR) {
                    continue;
                }
                let at = lo + i;
                let lang = l.glyphs.iter().find(|g| g.g.cluster >= at).map(|g| sp.style(&g.g).lang.clone()).unwrap_or_else(|| "pl".into());
                if check(w, &lang) {
                    let x0 = l.x_at(at);
                    let x1 = if at + w.len() >= l.end { l.content_right() } else { l.x_at(at + w.len()) };
                    let y = l.baseline + 2.2;
                    wave(ops, x0, x1, y, 1.0, Color::rgb(0xe5, 0x2b, 0x2b));
                }
            }
        }
    }
    if o.marks {
        let mc = Color::rgb(0x4a, 0x7a, 0xc0);
        for g in &l.glyphs {
            match g.g.kind {
                GKind::Space => {
                    let cx = g.x + g.adv / 2.0;
                    let cy = g.y - g.g.size * 0.25;
                    ops.push(Op::Fill { path: Path::ellipse(cx, cy, 0.6, 0.6), paint: Paint::Solid(mc), opacity: 1.0 });
                }
                GKind::Tab => {
                    let cy = g.y - g.g.size * 0.28;
                    let x1 = g.x + g.adv - 1.5;
                    ops.push(Op::Stroke { path: Path(vec![PathCmd::M(g.x + 1.0, cy), PathCmd::L(x1, cy), PathCmd::M(x1 - 2.0, cy - 2.0), PathCmd::L(x1, cy), PathCmd::L(x1 - 2.0, cy + 2.0)]), color: mc, width: 0.5, dash: vec![], opacity: 1.0 });
                }
                GKind::Newline => {
                    let cy = g.y - g.g.size * 0.3;
                    ops.push(Op::Stroke { path: Path(vec![PathCmd::M(g.x + 5.0, cy - 3.0), PathCmd::L(g.x + 5.0, cy), PathCmd::L(g.x + 1.0, cy), PathCmd::M(g.x + 2.6, cy - 1.6), PathCmd::L(g.x + 1.0, cy), PathCmd::L(g.x + 2.6, cy + 1.6)]), color: mc, width: 0.5, dash: vec![], opacity: 1.0 });
                }
                _ => {}
            }
        }
        if l.last {
            let x = l.content_right() + 1.5;
            let st = sp.styles[sp.end_style as usize].clone();
            let (gs, _) = text::measure("¶", &CharProps { color: mc, size: st.size, font: st.font.clone(), ..CharProps::default() });
            let mut px = x;
            let mut run = Vec::new();
            for g in gs {
                run.push(PGlyph { g, x: px, y: l.baseline, adv: g.adv, leader: false });
                px += g.adv;
            }
            let ms = CharProps { color: mc, ..CharProps::default() };
            push_runs(&run, &ms, "", ops, |_| ms.clone());
        }
    }
}

fn deco(ops: &mut Vec<Op>, u: Underline, x0: f32, x1: f32, y: f32, t: f32, c: Color) {
    match u {
        Underline::None => {}
        Underline::Single => ops.push(Op::Fill { path: Path::rect(Rect::new(x0, y - t / 2.0, x1 - x0, t)), paint: Paint::Solid(c), opacity: 1.0 }),
        Underline::Double => {
            ops.push(Op::Fill { path: Path::rect(Rect::new(x0, y - t / 2.0, x1 - x0, t * 0.8)), paint: Paint::Solid(c), opacity: 1.0 });
            ops.push(Op::Fill { path: Path::rect(Rect::new(x0, y + t * 1.4, x1 - x0, t * 0.8)), paint: Paint::Solid(c), opacity: 1.0 });
        }
        Underline::Dotted => ops.push(Op::Stroke { path: Path::line(x0, y, x1, y), color: c, width: t, dash: vec![t, t * 1.5], opacity: 1.0 }),
        Underline::Wave => wave(ops, x0, x1, y + t, t * 0.9, c),
    }
}

fn wave(ops: &mut Vec<Op>, x0: f32, x1: f32, y: f32, w: f32, c: Color) {
    let mut p = vec![PathCmd::M(x0, y)];
    let step = 1.6;
    let mut x = x0;
    let mut up = true;
    while x < x1 {
        x += step;
        p.push(PathCmd::L(x.min(x1), if up { y - 1.0 } else { y + 0.4 }));
        up = !up;
    }
    ops.push(Op::Stroke { path: Path(p), color: c, width: w * 0.8, dash: vec![], opacity: 1.0 });
}

/// Glify wiersza jako przebiegi o wspólnym kroju, rozmiarze, kolorze i linii bazowej.
fn push_runs(gs: &[PGlyph], _base: &CharProps, text: &str, ops: &mut Vec<Op>, style: impl Fn(usize) -> CharProps) {
    let mut k = 0;
    let cluster_end = |i: usize| -> usize {
        let c = gs[i].g.cluster;
        gs[i + 1..].iter().map(|g| g.g.cluster).find(|&n| n > c).unwrap_or(text.len()).min(text.len()).max(c.min(text.len()))
    };
    while k < gs.len() {
        let g0 = &gs[k];
        if matches!(g0.g.kind, GKind::Newline | GKind::Tab) {
            k += 1;
            continue;
        }
        let st = style(g0.g.style as usize);
        let mut run = GlyphRun {
            font: g0.g.font,
            size: g0.g.size,
            color: st.color,
            fake_bold: g0.g.fake_bold,
            fake_italic: g0.g.fake_italic,
            outline: st.outline,
            glyphs: Vec::new(),
            text: text.to_string(),
            ranges: Vec::new(),
            rot: 0.0,
            link: st.link.clone(),
        };
        let mut e = k;
        while e < gs.len() {
            let g = &gs[e];
            if g.g.font != g0.g.font || g.g.size != g0.g.size || g.g.style != g0.g.style || (g.y - g0.y).abs() > 0.01 || matches!(g.g.kind, GKind::Newline | GKind::Tab) {
                break;
            }
            run.glyphs.push((g.g.gid, g.x + g.g.dx, g.y));
            let c = g.g.cluster.min(text.len());
            run.ranges.push(c..cluster_end(e));
            e += 1;
        }
        if text.is_empty() {
            run.ranges = vec![0..0; run.glyphs.len()];
        }
        if !run.glyphs.is_empty() {
            push_effect(&run, st.effect, ops);
            ops.push(Op::Glyphs(run));
        }
        k = e.max(k + 1);
    }
}

/// Cień (przesunięta, półprzezroczysta kopia) albo poświata (gruby, miękki obrys) pod przebiegiem.
fn push_effect(run: &GlyphRun, effect: TextEffect, ops: &mut Vec<Op>) {
    let base = || GlyphRun { text: String::new(), ranges: vec![0..0; run.glyphs.len()], link: None, ..run.clone() };
    match effect {
        TextEffect::None => {}
        TextEffect::Shadow => {
            let d = (run.size * 0.06).max(0.6);
            let mut sh = base();
            sh.color = Color::rgb(0, 0, 0).with_alpha(90);
            sh.glyphs.iter_mut().for_each(|g| {
                g.1 += d;
                g.2 += d;
            });
            ops.push(Op::Glyphs(sh));
        }
        TextEffect::Glow => {
            // Kilka obrysów o rosnącej grubości – miękkie przejście bez rozmycia.
            for (w, a) in [(0.32, 30), (0.2, 45), (0.1, 70)] {
                let mut gl = base();
                gl.outline = run.size * w;
                gl.color = run.color.with_alpha(a);
                ops.push(Op::Glyphs(gl));
            }
        }
    }
}

fn label(ops: &mut Vec<Op>, s: &str, x: f32, y: f32, size: f32, color: Color, font: &str, anchor: f32, bold: bool) {
    let props = CharProps { font: font.into(), size, color, bold, ..CharProps::default() };
    let (gs, w) = text::measure(s, &props);
    let mut px = x - w * anchor;
    let mut run = Vec::new();
    for g in gs {
        run.push(PGlyph { g, x: px, y, adv: g.adv, leader: false });
        px += g.adv;
    }
    push_runs(&run, &props, "", ops, |_| props.clone());
}

fn fmt_num(v: f32) -> String {
    if (v - v.round()).abs() < 0.001 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}").replace('.', ",")
    }
}

/// Wykres w prostokącie ramki.
pub fn chart_ops(ch: &Chart, r: Rect, ops: &mut Vec<Op>) {
    let font = ch.font.as_str();
    let fs = (r.h / 26.0).clamp(6.0, 11.0);
    let text_c = Color::rgb(0x40, 0x40, 0x40);
    let mut top = r.y + 4.0;
    if !ch.title.is_empty() {
        label(ops, &ch.title, r.x + r.w / 2.0, top + fs * 1.4, fs * 1.35, Color::rgb(0x20, 0x20, 0x20), font, 0.5, true);
        top += fs * 2.2;
    }
    let mut bottom = r.bottom() - 4.0;
    if ch.legend && !ch.series.is_empty() && !matches!(ch.kind, ChartKind::Pie | ChartKind::Doughnut) {
        let y = bottom - fs * 0.4;
        let mut total = 0.0;
        let items: Vec<(String, f32)> = ch.series.iter().map(|s| {
            let w = text::measure(&s.name, &CharProps { font: font.into(), size: fs, ..CharProps::default() }).1;
            total += w + fs * 2.6;
            (s.name.clone(), w)
        }).collect();
        let mut x = r.x + (r.w - total) / 2.0;
        for (i, (name, w)) in items.iter().enumerate() {
            ops.push(Op::Fill { path: Path::rect(Rect::new(x, y - fs * 0.75, fs * 0.8, fs * 0.8)), paint: Paint::Solid(series_color(ch, i)), opacity: 1.0 });
            label(ops, name, x + fs * 1.2, y, fs, text_c, font, 0.0, false);
            x += w + fs * 2.6;
        }
        bottom -= fs * 2.0;
    }
    let n = ch.labels.len().max(ch.series.iter().map(|s| s.values.len()).max().unwrap_or(0));
    if n == 0 {
        return;
    }
    match ch.kind {
        ChartKind::Pie | ChartKind::Doughnut => {
            let vals: Vec<f32> = (0..n).map(|i| ch.series.first().and_then(|s| s.values.get(i)).copied().unwrap_or(0.0).max(0.0)).collect();
            let sum: f32 = vals.iter().sum::<f32>().max(1e-6);
            let legend_w = if ch.legend { r.w * 0.32 } else { 0.0 };
            let rad = ((r.w - legend_w).min(bottom - top) / 2.0 - 6.0).max(4.0);
            let (cx, cy) = (r.x + (r.w - legend_w) / 2.0, (top + bottom) / 2.0);
            let mut a0 = -std::f32::consts::FRAC_PI_2;
            for (i, v) in vals.iter().enumerate() {
                let a1 = a0 + v / sum * std::f32::consts::TAU;
                let mut p = vec![PathCmd::M(cx, cy)];
                let steps = ((a1 - a0) * 24.0).ceil().max(2.0) as usize;
                for s in 0..=steps {
                    let a = a0 + (a1 - a0) * s as f32 / steps as f32;
                    p.push(PathCmd::L(cx + rad * a.cos(), cy + rad * a.sin()));
                }
                p.push(PathCmd::Z);
                let col = PALETTE[i % PALETTE.len()];
                ops.push(Op::Fill { path: Path(p.clone()), paint: Paint::Solid(col), opacity: 1.0 });
                ops.push(Op::Stroke { path: Path(p), color: Color::WHITE, width: 1.0, dash: vec![], opacity: 1.0 });
                if ch.values && v / sum > 0.04 {
                    let am = (a0 + a1) / 2.0;
                    let k = if ch.kind == ChartKind::Doughnut { 0.78 } else { 0.62 };
                    label(ops, &format!("{}%", fmt_num((v / sum * 1000.0).round() / 10.0)), cx + rad * k * am.cos(), cy + rad * k * am.sin() + fs * 0.35, fs, Color::WHITE, font, 0.5, true);
                }
                a0 = a1;
            }
            if ch.kind == ChartKind::Doughnut {
                ops.push(Op::Fill { path: Path::ellipse(cx, cy, rad * 0.55, rad * 0.55), paint: Paint::Solid(Color::WHITE), opacity: 1.0 });
            }
            if ch.legend {
                let mut y = cy - (n as f32 * fs * 1.6) / 2.0 + fs;
                let x = r.right() - legend_w + 4.0;
                for i in 0..n {
                    ops.push(Op::Fill { path: Path::rect(Rect::new(x, y - fs * 0.75, fs * 0.8, fs * 0.8)), paint: Paint::Solid(PALETTE[i % PALETTE.len()]), opacity: 1.0 });
                    label(ops, ch.labels.get(i).map(|s| s.as_str()).unwrap_or(""), x + fs * 1.2, y, fs, text_c, font, 0.0, false);
                    y += fs * 1.6;
                }
            }
        }
        _ => {
            let max = ch.series.iter().flat_map(|s| s.values.iter()).copied().fold(0.0f32, f32::max).max(1e-6);
            let min = ch.series.iter().flat_map(|s| s.values.iter()).copied().fold(0.0f32, f32::min);
            let span = nice(max - min);
            let step = span / 5.0;
            let lo = (min / step).floor() * step;
            let hi = lo + span;
            let axis_w = fs * 3.2;
            let horizontal = ch.kind == ChartKind::Bar;
            let plot = Rect::new(r.x + axis_w, top + fs * 0.5, r.w - axis_w - 6.0, bottom - top - fs * 2.2);
            let grid_c = Color::rgb(0xd9, 0xd9, 0xd9);
            for k in 0..=5 {
                let v = lo + step * k as f32;
                if horizontal {
                    let x = plot.x + plot.w * (v - lo) / (hi - lo);
                    if ch.grid {
                        ops.push(Op::Stroke { path: Path::line(x, plot.y, x, plot.bottom()), color: grid_c, width: 0.5, dash: vec![], opacity: 1.0 });
                    }
                    label(ops, &fmt_num(v), x, plot.bottom() + fs * 1.2, fs * 0.9, text_c, font, 0.5, false);
                } else {
                    let y = plot.bottom() - plot.h * (v - lo) / (hi - lo);
                    if ch.grid {
                        ops.push(Op::Stroke { path: Path::line(plot.x, y, plot.right(), y), color: grid_c, width: 0.5, dash: vec![], opacity: 1.0 });
                    }
                    label(ops, &fmt_num(v), plot.x - 4.0, y + fs * 0.35, fs * 0.9, text_c, font, 1.0, false);
                }
            }
            let ns = ch.series.len().max(1) as f32;
            let slot = if horizontal { plot.h / n as f32 } else { plot.w / n as f32 };
            for i in 0..n {
                let lab = ch.labels.get(i).cloned().unwrap_or_default();
                if horizontal {
                    label(ops, &lab, plot.x - 4.0, plot.y + slot * (i as f32 + 0.5) + fs * 0.35, fs * 0.9, text_c, font, 1.0, false);
                } else {
                    label(ops, &lab, plot.x + slot * (i as f32 + 0.5), plot.bottom() + fs * 1.3, fs * 0.9, text_c, font, 0.5, false);
                }
            }
            let zero_y = plot.bottom() - plot.h * (0.0f32.clamp(lo, hi) - lo) / (hi - lo);
            let zero_x = plot.x + plot.w * (0.0f32.clamp(lo, hi) - lo) / (hi - lo);
            match ch.kind {
                ChartKind::Column | ChartKind::Bar => {
                    let bw = slot * 0.72 / ns;
                    for (si, s) in ch.series.iter().enumerate() {
                        let col = series_color(ch, si);
                        for (i, v) in s.values.iter().enumerate().take(n) {
                            let off = slot * i as f32 + slot * 0.14 + bw * si as f32;
                            let rr = if horizontal {
                                let x = plot.x + plot.w * (v - lo) / (hi - lo);
                                Rect::new(zero_x.min(x), plot.y + off, (x - zero_x).abs(), bw * 0.92)
                            } else {
                                let y = plot.bottom() - plot.h * (v - lo) / (hi - lo);
                                Rect::new(plot.x + off, zero_y.min(y), bw * 0.92, (zero_y - y).abs())
                            };
                            ops.push(Op::Fill { path: Path::rect(rr), paint: Paint::Solid(col), opacity: 1.0 });
                            if ch.values {
                                if horizontal {
                                    label(ops, &fmt_num(*v), rr.right() + 3.0, rr.y + rr.h / 2.0 + fs * 0.3, fs * 0.8, text_c, font, 0.0, false);
                                } else {
                                    label(ops, &fmt_num(*v), rr.x + rr.w / 2.0, rr.y - 2.5, fs * 0.8, text_c, font, 0.5, false);
                                }
                            }
                        }
                    }
                }
                ChartKind::Line | ChartKind::Area => {
                    for (si, s) in ch.series.iter().enumerate() {
                        let col = series_color(ch, si);
                        let pts: Vec<(f32, f32)> = s.values.iter().enumerate().take(n).map(|(i, v)| (plot.x + slot * (i as f32 + 0.5), plot.bottom() - plot.h * (v - lo) / (hi - lo))).collect();
                        if pts.is_empty() {
                            continue;
                        }
                        let mut p: Vec<PathCmd> = pts.iter().enumerate().map(|(i, &(x, y))| if i == 0 { PathCmd::M(x, y) } else { PathCmd::L(x, y) }).collect();
                        if ch.kind == ChartKind::Area {
                            let mut a = p.clone();
                            a.push(PathCmd::L(pts.last().unwrap().0, zero_y));
                            a.push(PathCmd::L(pts[0].0, zero_y));
                            a.push(PathCmd::Z);
                            ops.push(Op::Fill { path: Path(a), paint: Paint::Solid(col.with_alpha(90)), opacity: 1.0 });
                        }
                        ops.push(Op::Stroke { path: Path(std::mem::take(&mut p)), color: col, width: 2.0, dash: vec![], opacity: 1.0 });
                        for (i, &(x, y)) in pts.iter().enumerate() {
                            ops.push(Op::Fill { path: Path::ellipse(x, y, 2.6, 2.6), paint: Paint::Solid(col), opacity: 1.0 });
                            if ch.values {
                                label(ops, &fmt_num(s.values[i]), x, y - 5.0, fs * 0.8, text_c, font, 0.5, false);
                            }
                        }
                    }
                }
                _ => {}
            }
            let ac = Color::rgb(0x8c, 0x8c, 0x8c);
            if horizontal {
                ops.push(Op::Stroke { path: Path::line(zero_x, plot.y, zero_x, plot.bottom()), color: ac, width: 0.8, dash: vec![], opacity: 1.0 });
            } else {
                ops.push(Op::Stroke { path: Path::line(plot.x, zero_y, plot.right(), zero_y), color: ac, width: 0.8, dash: vec![], opacity: 1.0 });
            }
        }
    }
}

/// Znak wodny: napis po przekątnej strony, jasny i półprzezroczysty, za treścią.
fn watermark_ops(text: &str, ps: &PageSetup, ops: &mut Vec<Op>) {
    let ang = -(ps.height.atan2(ps.width));
    let diag = (ps.width * ps.width + ps.height * ps.height).sqrt();
    let mut props = CharProps { font: "Segoe UI".into(), size: 100.0, bold: true, color: Color::rgb(0xc8, 0xc8, 0xc8).with_alpha(110), ..CharProps::default() };
    let (_, w0) = text::measure(text, &props);
    props.size = (100.0 * diag * 0.7 / w0.max(1.0)).clamp(24.0, 160.0);
    let (gs, w) = text::measure(text, &props);
    let (cx, cy) = (ps.width / 2.0, ps.height / 2.0);
    let (sin, cos) = ang.sin_cos();
    let mut s = -w / 2.0;
    let rise = props.size * 0.35;
    for g in gs {
        let x = cx + cos * s + sin * rise;
        let y = cy + sin * s - cos * rise;
        ops.push(Op::Glyphs(GlyphRun { font: g.font, size: g.size, color: props.color, fake_bold: g.fake_bold, fake_italic: false, outline: 0.0, glyphs: vec![(g.gid, x, y)], text: String::new(), ranges: vec![0..0], rot: ang, link: None }));
        s += g.adv;
    }
}

struct CellText {
    sp: crate::text::ShapedPara,
    lines: Vec<(usize, usize, f32)>,
    lh: f32,
    asc: f32,
}

/// Tekst komórki (z wyróżnieniami **, *, ==) złamany do szerokości `w`.
fn cell_text(text: &str, base: &CharProps, w: f32) -> CellText {
    let p = Paragraph { style: String::new(), fmt: ParaFmt::default(), runs: crate::io::plain::md_inline(text) };
    let fv = crate::text::FieldValues { page: 1, pages: 1, title: "", author: "", notes: &[], ext: None, running: "" };
    let sp = crate::text::shape(&p, base, &fv, true);
    let n = sp.glyphs.len();
    let mut starts = vec![0usize];
    starts.extend(sp.breaks.iter().copied());
    starts.push(n);
    let mut lines = Vec::new();
    let (mut ls, mut lw) = (0usize, 0.0f32);
    for wi in 0..starts.len() - 1 {
        let (a, b) = (starts[wi], starts[wi + 1]);
        let ww: f32 = sp.glyphs[a..b].iter().map(|g| g.adv).sum();
        let trim: f32 = sp.glyphs[a..b].iter().rev().take_while(|g| g.kind == GKind::Space).map(|g| g.adv).sum();
        if lw > 0.0 && lw + ww - trim > w {
            lines.push((ls, a, lw));
            ls = a;
            lw = 0.0;
        }
        lw += ww;
    }
    if ls < n || lines.is_empty() {
        lines.push((ls, n, lw));
    }
    let (asc, desc, _) = text::vmetrics(sp.glyphs.first().map(|g| g.font).unwrap_or(0), base.size);
    CellText { sp, lines, lh: (asc + desc) * 1.12, asc }
}

fn table_geometry(t: &Table, width: f32) -> (Vec<f32>, f32, CharProps) {
    let cols = t.cols();
    let props = CharProps { font: t.font.clone(), size: t.size, ..CharProps::default() };
    if t.widths.len() == cols && t.widths.iter().all(|w| *w > 0.0) {
        let total: f32 = t.widths.iter().sum();
        return (t.widths.iter().map(|w| w / total * width).collect(), t.size * 0.45, props);
    }
    // Szerokości kolumn proporcjonalne do treści, z minimum i bez przesady dla długich komórek.
    let mut want = vec![30.0f32; cols];
    for row in &t.rows {
        for (c, cell) in row.iter().enumerate() {
            if cell == MERGED {
                continue;
            }
            let w = text::measure(&cell.replace("**", "").replace("==", ""), &props).1.min(width * 0.6) + t.size * 1.4;
            want[c] = want[c].max(w);
        }
    }
    let total: f32 = want.iter().sum();
    (want.iter().map(|w| w / total * width).collect(), t.size * 0.45, props)
}

/// Komórki wiersza z uwzględnieniem scalania: (kolumna początkowa, liczba kolumn).
fn row_spans(row: &[String], cols: usize) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for c in 0..cols {
        let merged = row.get(c).is_some_and(|v| v == MERGED) && !out.is_empty();
        if merged {
            out.last_mut().unwrap().1 += 1;
        } else {
            out.push((c, 1));
        }
    }
    out
}

/// Wysokości pokazanych wierszy.
pub fn table_row_heights(t: &Table, width: f32) -> Vec<f32> {
    let (widths, pad, props) = table_geometry(t, width);
    let cols = t.cols();
    t.rows
        .iter()
        .take(t.take.unwrap_or(usize::MAX))
        .enumerate()
        .map(|(ri, row)| {
            let mut base = props.clone();
            base.bold = t.header && ri == 0;
            let mut rh = t.size * 1.4;
            for (c0, span) in row_spans(row, cols) {
                let w: f32 = widths[c0..c0 + span].iter().sum();
                let ct = cell_text(row.get(c0).map(|s| s.as_str()).unwrap_or(""), &base, w - 2.0 * pad * 1.4);
                rh = rh.max(ct.lines.len() as f32 * ct.lh);
            }
            rh + 2.0 * pad
        })
        .collect()
}

/// Wysokość tabeli dla szerokości `width` (wiersze dopasowane do treści).
pub fn table_height(t: &Table, width: f32) -> f32 {
    table_row_heights(t, width).iter().sum::<f32>().max(10.0)
}

/// Prostokąty komórek: (wiersz, kolumna, prostokąt) – do edycji komórek myszą.
pub fn table_cells(t: &Table, r: Rect) -> Vec<(usize, usize, Rect)> {
    let (widths, _, _) = table_geometry(t, r.w);
    let heights = table_row_heights(t, r.w);
    let cols = t.cols();
    let mut out = Vec::new();
    let mut y = r.y;
    for (ri, row) in t.shown().iter().enumerate() {
        let h = heights.get(ri).copied().unwrap_or(10.0);
        for (c0, span) in row_spans(row, cols) {
            let x: f32 = r.x + widths[..c0].iter().sum::<f32>();
            let w: f32 = widths[c0..c0 + span].iter().sum();
            out.push((ri, c0, Rect::new(x, y, w, h)));
        }
        y += h;
    }
    out
}

/// Granice kolumn (x) – do zmiany szerokości myszą.
pub fn table_col_edges(t: &Table, r: Rect) -> Vec<f32> {
    let (widths, _, _) = table_geometry(t, r.w);
    let mut x = r.x;
    widths.iter().take(widths.len().saturating_sub(1)).map(|w| {
        x += w;
        x
    }).collect()
}

/// Tabela w prostokącie ramki: kolumny wg treści albo ustawione, scalone komórki, tekst zawijany.
pub fn table_ops(t: &Table, r: Rect, ops: &mut Vec<Op>) {
    let cols = t.cols();
    let (widths, pad, props) = table_geometry(t, r.w);
    let heights = table_row_heights(t, r.w);
    let numeric = |c: usize| t.numbers_right && t.rows.iter().skip(if t.header { 1 } else { 0 }).all(|row| row.get(c).is_none_or(|v| v.trim().is_empty() || v == MERGED || v.trim().replace([',', '.', ' ', '%', '-', '+'], "").chars().all(|ch| ch.is_ascii_digit() || "złeur$€".contains(ch))));
    let mix = |c: Color, k: f32| Color::rgb((c.r as f32 * k + 255.0 * (1.0 - k)) as u8, (c.g as f32 * k + 255.0 * (1.0 - k)) as u8, (c.b as f32 * k + 255.0 * (1.0 - k)) as u8);
    let mut y = r.y;
    let mut grid = Vec::new();
    for (ri, row) in t.shown().iter().enumerate() {
        let head = t.header && ri == 0;
        let rh = heights.get(ri).copied().unwrap_or(10.0);
        let mut base = props.clone();
        base.bold = head;
        base.color = if head { Color::WHITE } else { Color::rgb(0x22, 0x22, 0x22) };
        let bg = if head { Some(t.accent) } else if t.stripes && ri % 2 == 0 { Some(mix(t.accent, 0.07)) } else { None };
        if let Some(bg) = bg {
            ops.push(Op::Fill { path: Path::rect(Rect::new(r.x, y, r.w, rh)), paint: Paint::Solid(bg), opacity: 1.0 });
        }
        for (c0, span) in row_spans(row, cols) {
            let x = r.x + widths[..c0].iter().sum::<f32>();
            let w: f32 = widths[c0..c0 + span].iter().sum();
            if c0 > 0 {
                grid.push(PathCmd::M(x, y));
                grid.push(PathCmd::L(x, y + rh));
            }
            let inner = pad * 1.4;
            let ct = cell_text(row.get(c0).map(|s| s.as_str()).unwrap_or(""), &base, w - 2.0 * inner);
            let right = numeric(c0) && !head && span == 1;
            let total_h = ct.lines.len() as f32 * ct.lh;
            let mut ly = y + (rh - total_h) / 2.0 + ct.asc;
            for &(a, b, lw) in &ct.lines {
                let mut px = if right { x + w - inner - lw } else { x + inner };
                let mut run = Vec::new();
                for g in &ct.sp.glyphs[a..b] {
                    run.push(PGlyph { g: *g, x: px, y: ly - ct.sp.style(g).rise(), adv: g.adv, leader: false });
                    px += g.adv;
                }
                let styles = &ct.sp.styles;
                push_runs(&run, &base, "", ops, |si| styles[si].clone());
                ly += ct.lh;
            }
        }
        y += rh;
        if ri + 1 < t.shown().len() {
            grid.push(PathCmd::M(r.x, y));
            grid.push(PathCmd::L(r.right(), y));
        }
    }
    ops.push(Op::Stroke { path: Path(grid), color: t.line, width: 0.5, dash: vec![], opacity: 1.0 });
    ops.push(Op::Stroke { path: Path::rect(Rect::new(r.x, r.y, r.w, y - r.y)), color: t.line, width: 0.75, dash: vec![], opacity: 1.0 });
}

fn nice(span: f32) -> f32 {
    let span = span.max(1e-6);
    let mag = 10f32.powf(span.log10().floor());
    for m in [1.0, 1.2, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0] {
        if mag * m >= span {
            return mag * m;
        }
    }
    mag * 10.0
}
