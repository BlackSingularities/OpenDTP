//! Rysowanie listy operacji na ekran (tiny-skia). Glify trafiają do pamięci podręcznej jako
//! maski krycia w danym rozmiarze i z przesunięciem podpikselowym, obrazy – jako zdekodowane mapy.

use std::collections::HashMap;
use std::sync::Arc;

use tiny_skia::{FillRule, Mask, Paint as SkPaint, PathBuilder, Pixmap, PixmapPaint, Shader, Stroke, StrokeDash, Transform};

use crate::fonts;
use crate::model::{Color, Document};
use crate::paint::{GlyphRun, Op, Paint, Path, PathCmd};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GKey {
    font: u32,
    gid: u16,
    size: u32,
    sub: u8,
    bold: bool,
    italic: bool,
}

struct GMask {
    left: i32,
    top: i32,
    w: u32,
    h: u32,
    data: Vec<u8>,
}

#[derive(Default)]
pub struct Raster {
    glyphs: HashMap<GKey, Option<Arc<GMask>>>,
    images: HashMap<String, Option<Arc<Pixmap>>>,
    svgs: HashMap<(String, u32, u32), Option<Arc<Pixmap>>>,
}

/// SVG wyrenderowany do mapy pikseli `w`×`h` (resvg ma własną wersję tiny-skia – kopiujemy piksele).
pub fn render_svg(data: &[u8], w: u32, h: u32) -> Option<Pixmap> {
    let tree = usvg::Tree::from_data(data, &usvg::Options::default()).ok()?;
    let (w, h) = (w.clamp(1, 8000), h.clamp(1, 8000));
    let mut pm = resvg::tiny_skia::Pixmap::new(w, h)?;
    let s = tree.size();
    let t = resvg::tiny_skia::Transform::from_scale(w as f32 / s.width().max(1.0), h as f32 / s.height().max(1.0));
    resvg::render(&tree, t, &mut pm.as_mut());
    Pixmap::from_vec(pm.take(), tiny_skia::IntSize::from_wh(w, h)?)
}

struct Outline<'a> {
    pb: &'a mut PathBuilder,
    k: f32,
    ox: f32,
    oy: f32,
    skew: f32,
}

impl Outline<'_> {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        (self.ox + (x + y * self.skew) * self.k, self.oy - y * self.k)
    }
}

impl ttf_parser::OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (a, b) = self.p(x, y);
        self.pb.move_to(a, b);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (a, b) = self.p(x, y);
        self.pb.line_to(a, b);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (a, b) = self.p(x1, y1);
        let (c, d) = self.p(x, y);
        self.pb.quad_to(a, b, c, d);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b) = self.p(x1, y1);
        let (c, d) = self.p(x2, y2);
        let (e, f) = self.p(x, y);
        self.pb.cubic_to(a, b, c, d, e, f);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// Kontur glifu w pikselach (początek pióra w `ox, oy`).
pub fn glyph_path(font: u32, gid: u16, px: f32, ox: f32, oy: f32, italic: bool) -> Option<tiny_skia::Path> {
    let f = fonts::get(font)?;
    let mut pb = PathBuilder::new();
    let mut o = Outline { pb: &mut pb, k: px / f.upem, ox, oy, skew: if italic { 0.2 } else { 0.0 } };
    f.face.outline_glyph(ttf_parser::GlyphId(gid), &mut o)?;
    pb.finish()
}

fn sk_color(c: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

pub fn sk_path(p: &Path, scale: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for c in &p.0 {
        match *c {
            PathCmd::M(x, y) => pb.move_to(x * scale, y * scale),
            PathCmd::L(x, y) => pb.line_to(x * scale, y * scale),
            PathCmd::C(a, b, c2, d, e, f) => pb.cubic_to(a * scale, b * scale, c2 * scale, d * scale, e * scale, f * scale),
            PathCmd::Z => pb.close(),
        }
    }
    pb.finish()
}

fn shader(p: &Paint, scale: f32, opacity: f32) -> Shader<'static> {
    let op = |c: Color| sk_color(c.with_alpha((c.a as f32 * opacity) as u8));
    match p {
        Paint::Solid(c) => Shader::SolidColor(op(*c)),
        Paint::Linear { x0, y0, x1, y1, from, to } => tiny_skia::LinearGradient::new(
            tiny_skia::Point::from_xy(x0 * scale, y0 * scale),
            tiny_skia::Point::from_xy(x1 * scale, y1 * scale),
            vec![tiny_skia::GradientStop::new(0.0, op(*from)), tiny_skia::GradientStop::new(1.0, op(*to))],
            tiny_skia::SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap_or(Shader::SolidColor(op(*from))),
        Paint::Radial { cx, cy, r, from, to } => tiny_skia::RadialGradient::new(
            tiny_skia::Point::from_xy(cx * scale, cy * scale),
            tiny_skia::Point::from_xy(cx * scale, cy * scale),
            r * scale,
            vec![tiny_skia::GradientStop::new(0.0, op(*from)), tiny_skia::GradientStop::new(1.0, op(*to))],
            tiny_skia::SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap_or(Shader::SolidColor(op(*from))),
    }
}

impl Raster {
    pub fn new() -> Raster {
        Raster::default()
    }

    /// Obraz z dokumentu jako mapa tiny-skia (z pamięci podręcznej).
    /// SVG w rozmiarze w pikselach (z pamięci podręcznej).
    pub fn svg(&mut self, doc: &Document, id: &str, w: u32, h: u32) -> Option<Arc<Pixmap>> {
        let key = (id.to_string(), w, h);
        if let Some(v) = self.svgs.get(&key) {
            return v.clone();
        }
        let pm = doc.assets.get(id).and_then(|a| render_svg(&a.data, w, h)).map(Arc::new);
        if self.svgs.len() > 64 {
            self.svgs.clear();
        }
        self.svgs.insert(key, pm.clone());
        pm
    }

    pub fn image(&mut self, doc: &Document, id: &str) -> Option<Arc<Pixmap>> {
        if let Some(v) = self.images.get(id) {
            return v.clone();
        }
        let pm = doc.assets.get(id).and_then(|a| {
            let img = image::load_from_memory(&a.data).ok()?.to_rgba8();
            let (w, h) = img.dimensions();
            let mut data = img.into_raw();
            for px in data.chunks_exact_mut(4) {
                let a = px[3] as u16;
                px[0] = ((px[0] as u16 * a + 127) / 255) as u8;
                px[1] = ((px[1] as u16 * a + 127) / 255) as u8;
                px[2] = ((px[2] as u16 * a + 127) / 255) as u8;
            }
            Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(w, h)?).map(Arc::new)
        });
        self.images.insert(id.to_string(), pm.clone());
        pm
    }

    pub fn forget_image(&mut self, id: &str) {
        self.images.remove(id);
    }

    fn mask(&mut self, key: GKey) -> Option<Arc<GMask>> {
        if let Some(m) = self.glyphs.get(&key) {
            return m.clone();
        }
        let px = key.size as f32 / 8.0;
        let sub = key.sub as f32 / 4.0;
        let made = (|| {
            let path = glyph_path(key.font, key.gid, px, sub, 0.0, key.italic)?;
            let b = path.bounds();
            let embolden = if key.bold { (px * 0.035).max(0.35) } else { 0.0 };
            let left = (b.left() - 1.0 - embolden).floor() as i32;
            let top = (b.top() - 1.0 - embolden).floor() as i32;
            let w = ((b.right() + 1.0 + embolden).ceil() as i32 - left).max(1) as u32;
            let h = ((b.bottom() + 1.0 + embolden).ceil() as i32 - top).max(1) as u32;
            if w > 4096 || h > 4096 {
                return None;
            }
            let mut pm = Pixmap::new(w, h)?;
            let mut paint = SkPaint::default();
            paint.set_color(tiny_skia::Color::WHITE);
            paint.anti_alias = true;
            let t = Transform::from_translate(-left as f32, -top as f32);
            pm.fill_path(&path, &paint, FillRule::Winding, t, None);
            if embolden > 0.0 {
                let st = Stroke { width: embolden * 2.0, ..Stroke::default() };
                pm.stroke_path(&path, &paint, &st, t, None);
            }
            let data: Vec<u8> = pm.data().chunks_exact(4).map(|p| p[3]).collect();
            Some(Arc::new(GMask { left, top, w, h, data }))
        })();
        if self.glyphs.len() > 60_000 {
            self.glyphs.clear();
        }
        self.glyphs.insert(key, made.clone());
        made
    }

    fn blit(pm: &mut Pixmap, m: &GMask, x: i32, y: i32, c: Color) {
        let (pw, ph) = (pm.width() as i32, pm.height() as i32);
        let data = pm.data_mut();
        for row in 0..m.h as i32 {
            let dy = y + m.top + row;
            if dy < 0 || dy >= ph {
                continue;
            }
            for col in 0..m.w as i32 {
                let dx = x + m.left + col;
                if dx < 0 || dx >= pw {
                    continue;
                }
                let cov = m.data[(row * m.w as i32 + col) as usize] as u32;
                if cov == 0 {
                    continue;
                }
                let sa = cov * c.a as u32 / 255;
                let i = ((dy * pw + dx) * 4) as usize;
                let inv = 255 - sa;
                data[i] = ((c.r as u32 * sa + data[i] as u32 * inv) / 255) as u8;
                data[i + 1] = ((c.g as u32 * sa + data[i + 1] as u32 * inv) / 255) as u8;
                data[i + 2] = ((c.b as u32 * sa + data[i + 2] as u32 * inv) / 255) as u8;
                data[i + 3] = (sa + data[i + 3] as u32 * inv / 255) as u8;
            }
        }
    }

    fn glyphs(&mut self, pm: &mut Pixmap, run: &GlyphRun, scale: f32) {
        let px = run.size * scale;
        if px < 0.3 {
            return;
        }
        if run.rot.abs() > 1e-4 || run.outline > 0.0 || px > 400.0 {
            // Obrót, obrys albo bardzo duży rozmiar – rysujemy kontur bezpośrednio.
            for &(gid, x, y) in &run.glyphs {
                let Some(path) = glyph_path(run.font, gid, px, 0.0, 0.0, run.fake_italic) else { continue };
                let t = Transform::from_translate(x * scale, y * scale).pre_concat(Transform::from_rotate(run.rot.to_degrees()));
                let mut paint = SkPaint::default();
                paint.set_color(sk_color(run.color));
                paint.anti_alias = true;
                if run.outline > 0.0 {
                    let st = Stroke { width: run.outline * scale, ..Stroke::default() };
                    pm.stroke_path(&path, &paint, &st, t, None);
                } else {
                    pm.fill_path(&path, &paint, FillRule::Winding, t, None);
                    if run.fake_bold {
                        let st = Stroke { width: (px * 0.07).max(0.7), ..Stroke::default() };
                        pm.stroke_path(&path, &paint, &st, t, None);
                    }
                }
            }
            return;
        }
        for &(gid, x, y) in &run.glyphs {
            let fx = x * scale;
            let ix = fx.floor();
            let sub = ((fx - ix) * 4.0).floor() as u8 % 4;
            let key = GKey { font: run.font, gid, size: (px * 8.0).round() as u32, sub, bold: run.fake_bold, italic: run.fake_italic };
            if let Some(m) = self.mask(key) {
                Self::blit(pm, &m, ix as i32, (y * scale).round() as i32, run.color);
            }
        }
    }

    /// Rysuje operacje w skali `scale` (piksele na punkt).
    pub fn draw(&mut self, doc: &Document, ops: &[Op], pm: &mut Pixmap, scale: f32) {
        for op in ops {
            match op {
                Op::Fill { path, paint, opacity } => {
                    let Some(p) = sk_path(path, scale) else { continue };
                    let sp = SkPaint { shader: shader(paint, scale, *opacity), anti_alias: true, ..SkPaint::default() };
                    pm.fill_path(&p, &sp, FillRule::EvenOdd, Transform::identity(), None);
                }
                Op::Stroke { path, color, width, dash, opacity } => {
                    let Some(p) = sk_path(path, scale) else { continue };
                    let c = color.with_alpha((color.a as f32 * opacity) as u8);
                    let mut sp = SkPaint::default();
                    sp.set_color(sk_color(c));
                    sp.anti_alias = true;
                    let mut st = Stroke { width: (width * scale).max(0.6), ..Stroke::default() };
                    if dash.len() >= 2 {
                        st.dash = StrokeDash::new(dash.iter().map(|d| d * scale).collect(), 0.0);
                    }
                    pm.stroke_path(&p, &sp, &st, Transform::identity(), None);
                }
                Op::Glyphs(run) => self.glyphs(pm, run, scale),
                Op::Image { asset, rect, clip, opacity } => {
                    let is_svg = doc.assets.get(asset).is_some_and(|a| a.mime == "image/svg+xml");
                    let img = if is_svg {
                        self.svg(doc, asset, (rect.w * scale).ceil() as u32, (rect.h * scale).ceil() as u32)
                    } else {
                        self.image(doc, asset)
                    };
                    let Some(img) = img else { continue };
                    let mask = clip.as_ref().and_then(|c| {
                        let p = sk_path(c, scale)?;
                        let mut m = Mask::new(pm.width(), pm.height())?;
                        m.fill_path(&p, FillRule::EvenOdd, true, Transform::identity());
                        Some(m)
                    });
                    let sx = rect.w * scale / img.width() as f32;
                    let sy = rect.h * scale / img.height() as f32;
                    let t = Transform::from_row(sx, 0.0, 0.0, sy, rect.x * scale, rect.y * scale);
                    let paint = PixmapPaint { opacity: *opacity, quality: if sx < 0.5 { tiny_skia::FilterQuality::Bilinear } else { tiny_skia::FilterQuality::Bicubic }, ..PixmapPaint::default() };
                    pm.draw_pixmap(0, 0, img.as_ref().as_ref(), &paint, t, mask.as_ref());
                }
            }
        }
    }
}

/// Mapa pikseli do formatu RGBA bez mnożenia przez alfę (Slint, PNG).
pub fn unpremultiply(pm: &Pixmap) -> Vec<u8> {
    let mut out = pm.data().to_vec();
    for px in out.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            px[0] = (px[0] as u32 * 255 / a).min(255) as u8;
            px[1] = (px[1] as u32 * 255 / a).min(255) as u8;
            px[2] = (px[2] as u32 * 255 / a).min(255) as u8;
        }
    }
    out
}
