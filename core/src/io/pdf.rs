//! Eksport do PDF (krilla): ta sama lista rysowania co na ekranie, czcionki osadzone jako
//! podzbiory z mapą znaków (tekst da się zaznaczać i wyszukiwać), obrazy w oryginalnej jakości.

use std::collections::HashMap;

use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point, Size, Transform};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule, Stroke, StrokeDash};
use krilla::text::{Font, GlyphId, KrillaGlyph};
use krilla::Document as PdfDoc;

use crate::layout::Layout;
use crate::model::{Color, Document};
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

fn col(c: Color) -> rgb::Color {
    rgb::Color::new(c.r, c.g, c.b)
}

fn alpha(c: Color, op: f32) -> NormalizedF32 {
    NormalizedF32::new((c.a as f32 / 255.0 * op).clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE)
}

fn fill_of(p: &Paint, op: f32) -> Fill {
    match p {
        Paint::Solid(c) => Fill { paint: col(*c).into(), opacity: alpha(*c, op), rule: FillRule::EvenOdd },
        Paint::Linear { x0, y0, x1, y1, from, to } => {
            let g = krilla::paint::LinearGradient {
                x1: *x0,
                y1: *y0,
                x2: *x1,
                y2: *y1,
                transform: Transform::identity(),
                spread_method: krilla::paint::SpreadMethod::Pad,
                stops: vec![krilla::paint::Stop { offset: NormalizedF32::ZERO, color: col(*from).into(), opacity: alpha(*from, 1.0) }, krilla::paint::Stop { offset: NormalizedF32::ONE, color: col(*to).into(), opacity: alpha(*to, 1.0) }],
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
                stops: vec![krilla::paint::Stop { offset: NormalizedF32::ZERO, color: col(*from).into(), opacity: alpha(*from, 1.0) }, krilla::paint::Stop { offset: NormalizedF32::ONE, color: col(*to).into(), opacity: alpha(*to, 1.0) }],
                anti_alias: true,
            };
            Fill { paint: g.into(), opacity: NormalizedF32::new(op.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE), rule: FillRule::EvenOdd }
        }
    }
}

pub fn export(doc: &Document, layout: &Layout) -> Result<Vec<u8>, String> {
    let mut pdf = PdfDoc::new();
    let mut fonts: HashMap<u32, Option<Font>> = HashMap::new();
    let mut images: HashMap<String, Option<krilla::image::Image>> = HashMap::new();
    let opts = paint::PaintOpts { screen: false, marks: false, guides: false, misspelled: None, hide_obj: None };
    for page in 0..layout.pages.len().max(1) {
        let ops = paint::page_ops(doc, layout, page, &opts);
        let size = Size::from_wh(doc.page.width, doc.page.height).ok_or("zły rozmiar strony")?;
        let mut pg = pdf.start_page_with(PageSettings::new(size));
        let mut s = pg.surface();
        for op in &ops {
            match op {
                Op::Fill { path: p, paint, opacity } => {
                    let Some(kp) = path(p) else { continue };
                    s.set_fill(Some(fill_of(paint, *opacity)));
                    s.set_stroke(None);
                    s.draw_path(&kp);
                }
                Op::Stroke { path: p, color, width, dash, opacity } => {
                    let Some(kp) = path(p) else { continue };
                    s.set_fill(None);
                    s.set_stroke(Some(Stroke {
                        paint: col(*color).into(),
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
                    s.set_fill(Some(Fill { paint: col(run.color).into(), opacity: alpha(run.color, 1.0), rule: FillRule::NonZero }));
                    let stroke = if run.outline > 0.0 {
                        Some(Stroke { paint: col(run.color).into(), width: run.outline, ..Stroke::default() })
                    } else if run.fake_bold {
                        Some(Stroke { paint: col(run.color).into(), width: run.size * 0.035, ..Stroke::default() })
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
        s.finish();
        pg.finish();
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
