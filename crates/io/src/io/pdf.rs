//! Eksport do PDF (krilla): ta sama lista rysowania co na ekranie, czcionki osadzone jako
//! podzbiory z mapą znaków (tekst da się zaznaczać i wyszukiwać), obrazy w oryginalnej jakości,
//! zakładki z nagłówków (panel „Zakładki” czytnika) i klikalne łącza – jak w WordCraft.

use std::collections::HashMap;

use krilla::color::{cmyk, rgb, separation};
use krilla::geom::{PathBuilder, Point, Size, Transform};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule, Stroke, StrokeDash};
use krilla::text::{Font, GlyphId, KrillaGlyph};
use krilla::Document as PdfDoc;
use krilla::action::{Action, LinkAction};
use krilla::annotation::{Annotation, LinkAnnotation, Target};
use krilla::destination::XyzDestination;
use krilla::outline::{Outline, OutlineNode};

use crate::layout::Layout;
use crate::model::{Color, Document, Swatch};
use crate::paint::{self, Op, Paint, PathCmd};

fn path(p: &paint::Path) -> Option<krilla::geom::Path> {
    let mut pb = PathBuilder::new();
    for c in &p.0 {
        match *c {
            PathCmd::M(x, y) => pb.move_to(x, y),
            PathCmd::L(x, y) => pb.line_to(x, y),
            PathCmd::C(a, b, c2, d, e, f) => pb.cubic_to(a, b, c2, d, e, f),
            PathCmd::Z => pb.close(),
        }
    }
    pb.finish()
}

/// Sposób zapisu kolorów: RGB albo CMYK, z próbkami dokumentu (dokładne CMYK, kolory dodatkowe).
struct Cx<'a> {
    cmyk: bool,
    swatches: &'a [Swatch],
}

fn pct(v: u8) -> u8 {
    (v.min(100) as u16 * 255 / 100) as u8
}

fn col(c: Color, cx: &Cx) -> krilla::color::Color {
    let sw = cx.swatches.iter().find(|s| s.color.r == c.r && s.color.g == c.g && s.color.b == c.b);
    let k = sw.and_then(|s| s.cmyk).unwrap_or_else(|| crate::model::rgb_to_cmyk(c));
    let cm = cmyk::Color::new(pct(k[0]), pct(k[1]), pct(k[2]), pct(k[3]));
    match sw {
        Some(s) if s.spot && cx.cmyk => {
            let space = separation::SeparationSpace::new(separation::SeparationColorant::Custom(s.name.clone()), cm.into());
            separation::Color::new(255, space).into()
        }
        _ if cx.cmyk => cm.into(),
        _ => rgb::Color::new(c.r, c.g, c.b).into(),
    }
}

fn alpha(c: Color, op: f32) -> NormalizedF32 {
    NormalizedF32::new((c.a as f32 / 255.0 * op).clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE)
}

fn fill_of(p: &Paint, op: f32, k: &Cx) -> Fill {
    match p {
        Paint::Solid(c) => Fill { paint: col(*c, k).into(), opacity: alpha(*c, op), rule: FillRule::EvenOdd },
        Paint::Linear { x0, y0, x1, y1, from, to } => {
            let g = krilla::paint::LinearGradient {
                x1: *x0,
                y1: *y0,
                x2: *x1,
                y2: *y1,
                transform: Transform::identity(),
                spread_method: krilla::paint::SpreadMethod::Pad,
                stops: vec![krilla::paint::Stop { offset: NormalizedF32::ZERO, color: col(*from, k).into(), opacity: alpha(*from, 1.0) }, krilla::paint::Stop { offset: NormalizedF32::ONE, color: col(*to, k).into(), opacity: alpha(*to, 1.0) }],
                anti_alias: true,
            };
            Fill { paint: g.into(), opacity: NormalizedF32::new(op.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE), rule: FillRule::EvenOdd }
        }
        Paint::Radial { cx, cy, r, from, to } => {
            let g = krilla::paint::RadialGradient {
                fx: *cx,
                fy: *cy,
                fr: 0.0,
                cx: *cx,
                cy: *cy,
                cr: *r,
                transform: Transform::identity(),
                spread_method: krilla::paint::SpreadMethod::Pad,
                stops: vec![krilla::paint::Stop { offset: NormalizedF32::ZERO, color: col(*from, k).into(), opacity: alpha(*from, 1.0) }, krilla::paint::Stop { offset: NormalizedF32::ONE, color: col(*to, k).into(), opacity: alpha(*to, 1.0) }],
                anti_alias: true,
            };
            Fill { paint: g.into(), opacity: NormalizedF32::new(op.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE), rule: FillRule::EvenOdd }
        }
    }
}

/// Znaczniki cięcia w kolorze rejestracyjnym (wszystkie farby), odsunięte poza spad.
fn crop_marks(s: &mut krilla::surface::Surface, o: f32, bleed: f32, pw: f32, ph: f32) {
    let reg: krilla::color::Color = separation::Color::new(255, separation::SeparationSpace::new(separation::SeparationColorant::AllColorants, cmyk::Color::new(255, 255, 255, 255).into())).into();
    s.set_fill(None);
    s.set_stroke(Some(Stroke { paint: reg.into(), width: 0.25, ..Stroke::default() }));
    let gap = bleed + 3.0;
    let len = 14.0;
    let (x0, y0, x1, y1) = (o, o, o + pw, o + ph);
    let mut pb = PathBuilder::new();
    for (x, y, dx, dy) in [(x0, y0, -1.0, -1.0), (x1, y0, 1.0, -1.0), (x0, y1, -1.0, 1.0), (x1, y1, 1.0, 1.0)] {
        // Pozioma kreska na wysokości krawędzi i pionowa na linii krawędzi.
        pb.move_to(x + dx * gap, y);
        pb.line_to(x + dx * (gap + len), y);
        pb.move_to(x, y + dy * gap);
        pb.line_to(x, y + dy * (gap + len));
    }
    if let Some(p) = pb.finish() {
        s.draw_path(&p);
    }
    s.set_stroke(None);
}

pub fn export(doc: &Document, layout: &Layout) -> Result<Vec<u8>, String> {
    export_with(doc, layout, false)
}

/// `print` – PDF dla drukarni: kolory CMYK (z próbkami), spady, ramki TrimBox/BleedBox i znaczniki cięcia
/// według `doc.print`.
pub fn export_with(doc: &Document, layout: &Layout, print: bool) -> Result<Vec<u8>, String> {
    let ps = &doc.print;
    let cx = &Cx { cmyk: print && ps.cmyk, swatches: &doc.swatches };
    let bleed = if print { ps.bleed.max(0.0) } else { 0.0 };
    // Margines wokół spadu na znaczniki cięcia.
    let slug = if print && ps.marks { 20.0 } else { 0.0 };
    let o = bleed + slug;
    let mut pdf = PdfDoc::new();
    let mut fonts: HashMap<u32, Option<Font>> = HashMap::new();
    let mut images: HashMap<String, Option<krilla::image::Image>> = HashMap::new();
    let opts = paint::PaintOpts { screen: false, marks: false, guides: false, misspelled: None, hide_obj: None };
    for page in 0..layout.pages.len().max(1) {
        let ops = paint::page_ops(doc, layout, page, &opts);
        let (pw, ph) = layout.page_size(doc, page);
        let size = Size::from_wh(pw + 2.0 * o, ph + 2.0 * o).ok_or("zły rozmiar strony")?;
        let mut settings = PageSettings::new(size);
        if print {
            settings = settings
                .with_trim_box(krilla::geom::Rect::from_xywh(o, o, pw, ph))
                .with_bleed_box(krilla::geom::Rect::from_xywh(o - bleed, o - bleed, pw + 2.0 * bleed, ph + 2.0 * bleed));
        }
        let mut pg = pdf.start_page_with(settings);
        // Łącza z tej strony (prostokąt w układzie strony, adres) – adnotacje dodajemy po rysowaniu.
        let mut links: Vec<(krilla::geom::Rect, String)> = Vec::new();
        let mut s = pg.surface();
        if print {
            s.push_transform(&Transform::from_translate(o, o));
            // Tło strony sięga do krawędzi spadu.
            let bg = layout.setup(doc, page).color;
            if let Some(kp) = path(&paint::Path::rect(crate::geom::Rect::new(-bleed, -bleed, pw + 2.0 * bleed, ph + 2.0 * bleed))) {
                s.set_fill(Some(fill_of(&Paint::Solid(bg), 1.0, cx)));
                s.set_stroke(None);
                s.draw_path(&kp);
            }
        }
        for op in &ops {
            match op {
                Op::Fill { path: p, paint, opacity } => {
                    let Some(kp) = path(p) else { continue };
                    s.set_fill(Some(fill_of(paint, *opacity, cx)));
                    s.set_stroke(None);
                    s.draw_path(&kp);
                }
                Op::Stroke { path: p, color, width, dash, opacity } => {
                    let Some(kp) = path(p) else { continue };
                    s.set_fill(None);
                    s.set_stroke(Some(Stroke {
                        paint: col(*color, cx).into(),
                        width: *width,
                        opacity: alpha(*color, *opacity),
                        dash: if dash.len() >= 2 { Some(StrokeDash { array: dash.clone(), offset: 0.0 }) } else { None },
                        ..Stroke::default()
                    }));
                    s.draw_path(&kp);
                    s.set_stroke(None);
                }
                Op::Glyphs(run) => {
                    let font = fonts
                        .entry(run.font)
                        .or_insert_with(|| crate::fonts::get(run.font).and_then(|l| Font::new(krilla::Data::from(l.data), l.index)))
                        .clone();
                    let Some(font) = font else { continue };
                    let Some(l) = crate::fonts::get(run.font) else { continue };
                    let upem = l.upem;
                    s.set_fill(Some(Fill { paint: col(run.color, cx).into(), opacity: alpha(run.color, 1.0), rule: FillRule::NonZero }));
                    let stroke = if run.outline > 0.0 {
                        Some(Stroke { paint: col(run.color, cx).into(), width: run.outline, ..Stroke::default() })
                    } else if run.fake_bold {
                        Some(Stroke { paint: col(run.color, cx).into(), width: run.size * 0.035, ..Stroke::default() })
                    } else {
                        None
                    };
                    if run.outline > 0.0 {
                        s.set_fill(None);
                    }
                    s.set_stroke(stroke);
                    let text: &str = &run.text;
                    // Kolejne glify w przebiegu – przesunięcia z położeń bezwzględnych.
                    let Some(&(_, x0, y0)) = run.glyphs.first() else { continue };
                    if let Some(url) = run.link.as_ref().filter(|u| web_link(u)) {
                        if run.rot.abs() < 1e-4 {
                            let x1 = run.glyphs.last().map(|g| g.1).unwrap_or(x0) + run.size * 0.6;
                            if let Some(r) = krilla::geom::Rect::from_xywh(x0 + o, y0 - run.size * 0.8 + o, (x1 - x0).max(1.0), run.size * 1.05) {
                                links.push((r, url.clone()));
                            }
                        }
                    }
                    let mut gs = Vec::with_capacity(run.glyphs.len());
                    for (i, &(gid, x, _)) in run.glyphs.iter().enumerate() {
                        let next = run.glyphs.get(i + 1).map(|g| g.1).unwrap_or_else(|| x + l.face.glyph_hor_advance(ttf_parser::GlyphId(gid)).unwrap_or(0) as f32 * run.size / upem);
                        let adv = (next - x) / run.size;
                        let range = run.ranges.get(i).cloned().unwrap_or(0..0);
                        let range = if range.end <= text.len() && range.start <= range.end { range } else { 0..0 };
                        gs.push(KrillaGlyph::new(GlyphId::new(gid as u32), adv, 0.0, 0.0, 0.0, range, None));
                    }
                    let mut pushed = 0;
                    if run.rot.abs() > 1e-4 {
                        s.push_transform(&Transform::from_row(run.rot.cos(), run.rot.sin(), -run.rot.sin(), run.rot.cos(), x0, y0));
                        pushed += 1;
                    } else if run.fake_italic {
                        s.push_transform(&Transform::from_row(1.0, 0.0, -0.2, 1.0, 0.2 * y0, 0.0));
                        pushed += 1;
                    }
                    let start = if run.rot.abs() > 1e-4 { Point::from_xy(0.0, 0.0) } else { Point::from_xy(x0, y0) };
                    s.draw_glyphs(start, &gs, font, text, run.size, false);
                    for _ in 0..pushed {
                        s.pop();
                    }
                    s.set_stroke(None);
                }
                Op::Image { asset, rect, clip, opacity } if doc.assets.get(asset).is_some_and(|a| a.mime == "image/svg+xml") => {
                    use krilla_svg::{SurfaceExt, SvgSettings};
                    let Some(tree) = doc.assets.get(asset).and_then(|a| usvg::Tree::from_data(&a.data, &usvg::Options::default()).ok()) else { continue };
                    let mut pushed = 0;
                    if let Some(c) = clip.as_ref().and_then(path) {
                        s.push_clip_path(&c, &FillRule::EvenOdd);
                        pushed += 1;
                    }
                    if *opacity < 0.999 {
                        s.push_opacity(NormalizedF32::new(opacity.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE));
                        pushed += 1;
                    }
                    s.push_transform(&Transform::from_translate(rect.x, rect.y));
                    pushed += 1;
                    if let Some(sz) = Size::from_wh(rect.w.max(0.1), rect.h.max(0.1)) {
                        let _ = s.draw_svg(&tree, sz, SvgSettings::default());
                    }
                    for _ in 0..pushed {
                        s.pop();
                    }
                }
                Op::Image { asset, rect, clip, opacity } => {
                    let img = images
                        .entry(asset.clone())
                        .or_insert_with(|| {
                            let a = doc.assets.get(asset)?;
                            let data = krilla::Data::from(a.data.as_ref().clone());
                            match a.mime.as_str() {
                                "image/png" => krilla::image::Image::from_png(data, true).ok(),
                                "image/jpeg" => krilla::image::Image::from_jpeg(data, true).ok(),
                                "image/gif" => krilla::image::Image::from_gif(data, true).ok(),
                                "image/webp" => krilla::image::Image::from_webp(data, true).ok(),
                                _ => {
                                    let i = image::load_from_memory(&a.data).ok()?.to_rgba8();
                                    let (w, h) = i.dimensions();
                                    Some(krilla::image::Image::from_rgba8(i.into_raw(), w, h))
                                }
                            }
                        })
                        .clone();
                    let Some(img) = img else { continue };
                    let mut pushed = 0;
                    if let Some(c) = clip.as_ref().and_then(path) {
                        s.push_clip_path(&c, &FillRule::EvenOdd);
                        pushed += 1;
                    }
                    if *opacity < 0.999 {
                        s.push_opacity(NormalizedF32::new(opacity.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE));
                        pushed += 1;
                    }
                    s.push_transform(&Transform::from_translate(rect.x, rect.y));
                    pushed += 1;
                    if let Some(sz) = Size::from_wh(rect.w.max(0.1), rect.h.max(0.1)) {
                        s.draw_image(img, sz);
                    }
                    for _ in 0..pushed {
                        s.pop();
                    }
                }
            }
        }
        if print {
            s.pop();
            if ps.marks {
                crop_marks(&mut s, o, bleed, pw, ph);
            }
        }
        s.finish();
        for (rect, url) in links {
            let target = Target::Action(Action::Link(LinkAction::new(url.clone())));
            pg.add_annotation(Annotation::new_link(LinkAnnotation::new(rect, target), Some(url)));
        }
        pg.finish();
    }
    if let Some(outline) = outline(doc, layout, o) {
        pdf.set_outline(outline);
    }
    let mut meta = krilla::metadata::Metadata::new().creator("OpenDTP".to_string());
    if !doc.meta.title.is_empty() {
        meta = meta.title(doc.meta.title.clone());
    }
    if !doc.meta.author.is_empty() {
        meta = meta.authors(vec![doc.meta.author.clone()]);
    }
    pdf.set_metadata(meta);
    pdf.finish().map_err(|e| format!("{e:?}"))
}

/// Adresy, które czytnik PDF otworzy (odsyłacze wewnątrz dokumentu pomijamy).
fn web_link(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("http://") || u.starts_with("https://") || u.starts_with("mailto:") || u.starts_with("ftp://")
}

/// Zakładki PDF z nagłówków treści głównej (poziomy zagnieżdżone jak w spisie treści).
fn outline(doc: &Document, layout: &Layout, offset: f32) -> Option<Outline> {
    let mut items: Vec<(u8, String, XyzDestination)> = Vec::new();
    for (i, p) in doc.body.paras.iter().enumerate() {
        let Some(level) = doc.outline_level(p) else { continue };
        let text: String = p.text().chars().filter(|c| *c != crate::model::FIELD_CHAR).collect::<String>().trim().to_string();
        if text.is_empty() {
            continue;
        }
        let line = layout.lines.iter().find(|l| l.story == crate::model::StoryRef::Body && l.para == i);
        let page = line.map(|l| l.page).or_else(|| layout.body_pages.get(i).copied()).unwrap_or(0).min(layout.pages.len().max(1) - 1);
        // (strona spoza zakresu zatrzymałaby eksport – krilla wtedy panikuje)
        let y = line.map(|l| l.top).unwrap_or(0.0);
        items.push((level.max(1), text, XyzDestination::new(page, Point::from_xy(0.0, (y + offset - 6.0).max(0.0)))));
    }
    if items.is_empty() {
        return None;
    }
    let mut outline = Outline::new();
    for node in nest(&items, &mut 0, 0) {
        outline.push_child(node);
    }
    Some(outline)
}

/// Drzewo zakładek: kolejne pozycje o wyższym poziomie stają się dziećmi poprzedniej.
fn nest(items: &[(u8, String, XyzDestination)], i: &mut usize, parent: u8) -> Vec<OutlineNode> {
    let mut out = Vec::new();
    while *i < items.len() && items[*i].0 > parent {
        let (level, text, dest) = items[*i].clone();
        *i += 1;
        let mut node = OutlineNode::new(text, dest);
        for child in nest(items, i, level) {
            node.push_child(child);
        }
        out.push(node.with_open(level < 2));
    }
    out
}
