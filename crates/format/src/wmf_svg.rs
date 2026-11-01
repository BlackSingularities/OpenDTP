//! WMF → SVG: metaplik Windows (np. wzory MathType, cliparty) jako grafika wektorowa.
//! Tekst zamieniamy na krzywe glifów z naszych czcionek – SVG nie potrzebuje wtedy fontów
//! i wygląda tak samo na ekranie, w PDF i przy każdym powiększeniu.

use std::fmt::Write as _;

use crate::model::CharProps;
use crate::{fonts, text};

#[derive(Clone)]
enum Obj {
    Pen { style: u16, width: f32, color: u32 },
    Brush { style: u16, color: u32 },
    Font { height: f32, weight: i16, italic: bool, face: String, charset: u8, escapement: f32 },
    Other,
}

#[derive(Clone)]
struct Dc {
    org: (f32, f32),
    ext: (f32, f32),
    pen: Obj,
    brush: Obj,
    font: Obj,
    text_color: u32,
    text_align: u16,
    poly_fill: u16,
    cur: (f32, f32),
}

impl Default for Dc {
    fn default() -> Self {
        Dc {
            org: (0.0, 0.0),
            ext: (1.0, 1.0),
            pen: Obj::Pen { style: 0, width: 0.0, color: 0 },
            brush: Obj::Brush { style: 0, color: 0xffffff },
            font: Obj::Font { height: -12.0, weight: 400, italic: false, face: "Times New Roman".into(), charset: 0, escapement: 0.0 },
            text_color: 0,
            text_align: 0,
            poly_fill: 1,
            cur: (0.0, 0.0),
        }
    }
}

fn u16_at(d: &[u8], i: usize) -> u16 {
    d.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).unwrap_or(0)
}

fn i16_at(d: &[u8], i: usize) -> f32 {
    u16_at(d, i) as i16 as f32
}

fn u32_at(d: &[u8], i: usize) -> u32 {
    d.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0)
}

fn css(c: u32) -> String {
    format!("#{:02x}{:02x}{:02x}", c & 0xff, (c >> 8) & 0xff, (c >> 16) & 0xff)
}

/// WMF → (SVG, szerokość pt, wysokość pt). `None` dla innych danych albo pustego rysunku.
pub fn to_svg(data: &[u8]) -> Option<(String, f32, f32)> {
    let placeable = data.starts_with(&[0xd7, 0xcd, 0xc6, 0x9a]);
    let (body, bbox, inch) = if placeable && data.len() > 22 {
        let bb = (i16_at(data, 6), i16_at(data, 8), i16_at(data, 10), i16_at(data, 12));
        (&data[22..], Some(bb), u16_at(data, 14).max(1) as f32)
    } else {
        (data, None, 1440.0)
    };
    if body.len() < 18 || !matches!(body[0], 1 | 2) || body[2] != 9 {
        return None;
    }
    let mut dc = Dc::default();
    if let Some((l, t, r, b)) = bbox {
        dc.org = (l, t);
        dc.ext = (r - l, b - t);
    }
    // Rozmiar w punktach z nagłówka; bez niego – z pierwszego SETWINDOWEXT (przyjmujemy 1440 j./cal).
    let mut size = bbox.map(|(l, t, r, b)| ((r - l).abs() / inch * 72.0, (b - t).abs() / inch * 72.0));
    let mut objs: Vec<Option<Obj>> = Vec::new();
    let mut stack: Vec<Dc> = Vec::new();
    let mut out = String::new();
    let mut i = 18;
    let mut drawn = 0usize;
    while i + 6 <= body.len() {
        let words = u32_at(body, i) as usize;
        let func = u16_at(body, i + 4);
        if words < 3 || i + words * 2 > body.len() {
            break;
        }
        let p = &body[i + 6..i + words * 2];
        i += words * 2;
        let (w, h) = size.unwrap_or((dc.ext.0.abs() / inch * 72.0, dc.ext.1.abs() / inch * 72.0));
        // Punkt logiczny → SVG (punkty).
        let tx = |dc: &Dc, x: f32| (x - dc.org.0) / dc.ext.0 * w;
        let ty = |dc: &Dc, y: f32| (y - dc.org.1) / dc.ext.1 * h;
        let kx = (w / dc.ext.0).abs();
        match func {
            0x0000 => break,
            0x020b => dc.org = (i16_at(p, 2), i16_at(p, 0)),
            0x020c => {
                dc.ext = (i16_at(p, 2), i16_at(p, 0));
                if size.is_none() {
                    size = Some((dc.ext.0.abs() / inch * 72.0, dc.ext.1.abs() / inch * 72.0));
                }
            }
            0x0209 => dc.text_color = u32_at(p, 0),
            0x012e => dc.text_align = u16_at(p, 0),
            0x0106 => dc.poly_fill = u16_at(p, 0),
            0x001e => stack.push(dc.clone()),
            0x0127 => {
                if let Some(s) = stack.pop() {
                    dc = s;
                }
            }
            // Obiekty GDI trafiają na pierwsze wolne miejsce tabeli.
            0x02fa | 0x02fc | 0x02fb | 0x00f7 | 0x01f9 | 0x0142 | 0x06ff => {
                let o = match func {
                    0x02fa => Obj::Pen { style: u16_at(p, 0), width: i16_at(p, 2).abs(), color: u32_at(p, 6) },
                    0x02fc => Obj::Brush { style: u16_at(p, 0), color: u32_at(p, 2) },
                    0x02fb => {
                        let face: String = p.get(18..50).unwrap_or(&[]).iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
                        Obj::Font { height: i16_at(p, 0), weight: u16_at(p, 8) as i16, italic: p.get(10).is_some_and(|&b| b != 0), face, charset: p.get(13).copied().unwrap_or(0), escapement: i16_at(p, 4) / 10.0 }
                    }
                    _ => Obj::Other,
                };
                match objs.iter().position(|o| o.is_none()) {
                    Some(k) => objs[k] = Some(o),
                    None => objs.push(Some(o)),
                }
            }
            0x01f0 => {
                if let Some(o) = objs.get_mut(u16_at(p, 0) as usize) {
                    *o = None;
                }
            }
            0x012d => {
                if let Some(Some(o)) = objs.get(u16_at(p, 0) as usize) {
                    match o {
                        Obj::Pen { .. } => dc.pen = o.clone(),
                        Obj::Brush { .. } => dc.brush = o.clone(),
                        Obj::Font { .. } => dc.font = o.clone(),
                        Obj::Other => {}
                    }
                }
            }
            0x0214 => dc.cur = (i16_at(p, 2), i16_at(p, 0)),
            0x0213 => {
                let to = (i16_at(p, 2), i16_at(p, 0));
                let d = format!("M{:.2} {:.2}L{:.2} {:.2}", tx(&dc, dc.cur.0), ty(&dc, dc.cur.1), tx(&dc, to.0), ty(&dc, to.1));
                shape(&mut out, &dc, &d, false, kx);
                dc.cur = to;
                drawn += 1;
            }
            0x0324 | 0x0325 => {
                let n = u16_at(p, 0) as usize;
                let mut d = String::new();
                for k in 0..n {
                    let (x, y) = (i16_at(p, 2 + k * 4), i16_at(p, 4 + k * 4));
                    let _ = write!(d, "{}{:.2} {:.2}", if k == 0 { 'M' } else { 'L' }, tx(&dc, x), ty(&dc, y));
                }
                if func == 0x0324 {
                    d.push('Z');
                }
                shape(&mut out, &dc, &d, func == 0x0324, kx);
                drawn += 1;
            }
            0x0538 => {
                let polys = u16_at(p, 0) as usize;
                let mut at = 2 + polys * 2;
                let mut d = String::new();
                for q in 0..polys {
                    let n = u16_at(p, 2 + q * 2) as usize;
                    for k in 0..n {
                        let (x, y) = (i16_at(p, at + k * 4), i16_at(p, at + 2 + k * 4));
                        let _ = write!(d, "{}{:.2} {:.2}", if k == 0 { 'M' } else { 'L' }, tx(&dc, x), ty(&dc, y));
                    }
                    d.push('Z');
                    at += n * 4;
                }
                shape(&mut out, &dc, &d, true, kx);
                drawn += 1;
            }
            0x041b | 0x0418 | 0x061c => {
                let o = if func == 0x061c { 4 } else { 0 };
                let (b, r, t, l) = (i16_at(p, o), i16_at(p, o + 2), i16_at(p, o + 4), i16_at(p, o + 6));
                let (x0, x1) = (tx(&dc, l).min(tx(&dc, r)), tx(&dc, l).max(tx(&dc, r)));
                let (y0, y1) = (ty(&dc, t).min(ty(&dc, b)), ty(&dc, t).max(ty(&dc, b)));
                let d = if func == 0x0418 {
                    let (cx, cy, rx, ry) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0);
                    format!("M{:.2} {:.2}A{rx:.2} {ry:.2} 0 1 0 {:.2} {:.2}A{rx:.2} {ry:.2} 0 1 0 {:.2} {:.2}Z", cx - rx, cy, cx + rx, cy, cx - rx, cy)
                } else if func == 0x061c {
                    let (rw, rh) = ((i16_at(p, 2) / dc.ext.0 * w).abs() / 2.0, (i16_at(p, 0) / dc.ext.1 * h).abs() / 2.0);
                    format!(
                        "M{:.2} {y0:.2}H{:.2}A{rw:.2} {rh:.2} 0 0 1 {x1:.2} {:.2}V{:.2}A{rw:.2} {rh:.2} 0 0 1 {:.2} {y1:.2}H{:.2}A{rw:.2} {rh:.2} 0 0 1 {x0:.2} {:.2}V{:.2}A{rw:.2} {rh:.2} 0 0 1 {:.2} {y0:.2}Z",
                        x0 + rw, x1 - rw, y0 + rh, y1 - rh, x1 - rw, x0 + rw, y1 - rh, y0 + rh, x0 + rw
                    )
                } else {
                    format!("M{x0:.2} {y0:.2}H{x1:.2}V{y1:.2}H{x0:.2}Z")
                };
                shape(&mut out, &dc, &d, true, kx);
                drawn += 1;
            }
            0x0521 => {
                let n = u16_at(p, 0) as usize;
                let s = p.get(2..2 + n).unwrap_or(&[]).to_vec();
                let at = 2 + n.div_ceil(2) * 2;
                let (y, x) = (i16_at(p, at), i16_at(p, at + 2));
                text_out(&mut out, &dc, &s, x, y, &[], &tx, &ty, w, h);
                drawn += 1;
            }
            0x0a32 => {
                let (y, x) = (i16_at(p, 0), i16_at(p, 2));
                let n = u16_at(p, 4) as usize;
                let opts = u16_at(p, 6);
                let mut at = 8;
                if opts & 0x0006 != 0 {
                    at += 8;
                }
                let s = p.get(at..at + n).unwrap_or(&[]).to_vec();
                at += n.div_ceil(2) * 2;
                let dx: Vec<f32> = (0..n).map(|k| i16_at(p, at + k * 2)).take_while(|_| at + n * 2 <= p.len()).collect();
                text_out(&mut out, &dc, &s, x, y, &dx, &tx, &ty, w, h);
                drawn += 1;
            }
            // Bitmapa w metapliku (STRETCHDIB, DIBSTRETCHBLT) – jako obraz PNG w SVG.
            0x0f43 | 0x0b41 => {
                let (dib_at, dh, dw, dy, dx) = if func == 0x0f43 { (22, i16_at(p, 14), i16_at(p, 16), i16_at(p, 18), i16_at(p, 20)) } else { (20, i16_at(p, 12), i16_at(p, 14), i16_at(p, 16), i16_at(p, 18)) };
                if let Some(png) = p.get(dib_at..).and_then(dib_to_png) {
                    use base64::Engine as _;
                    let (x0, y0) = (tx(&dc, dx), ty(&dc, dy));
                    let (x1, y1) = (tx(&dc, dx + dw), ty(&dc, dy + dh));
                    let _ = write!(
                        out,
                        r#"<image x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" preserveAspectRatio="none" href="data:image/png;base64,{}"/>"#,
                        x0.min(x1),
                        y0.min(y1),
                        (x1 - x0).abs(),
                        (y1 - y0).abs(),
                        base64::engine::general_purpose::STANDARD.encode(png)
                    );
                    drawn += 1;
                }
            }
            _ => {}
        }
    }
    let (w, h) = size?;
    if drawn == 0 || w < 0.5 || h < 0.5 {
        return None;
    }
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.2}" height="{h:.2}" viewBox="0 0 {w:.2} {h:.2}">{out}</svg>"#);
    Some((svg, w, h))
}

/// Kształt z bieżącym piórem i pędzlem (pióro PS_NULL i pędzel BS_NULL – bez obrysu/wypełnienia).
fn shape(out: &mut String, dc: &Dc, d: &str, fill: bool, kx: f32) {
    let fill_s = match (&dc.brush, fill) {
        (Obj::Brush { style, color }, true) if *style != 1 => css(*color),
        _ => "none".into(),
    };
    let (stroke_s, sw) = match &dc.pen {
        Obj::Pen { style, width, color } if style & 0xf != 5 => (css(*color), (width * kx).max(0.25)),
        _ => ("none".into(), 0.0),
    };
    if fill_s == "none" && stroke_s == "none" {
        return;
    }
    let rule = if dc.poly_fill == 2 { "nonzero" } else { "evenodd" };
    let _ = write!(out, r#"<path d="{d}" fill="{fill_s}" fill-rule="{rule}" stroke="{stroke_s}" stroke-width="{sw:.2}" stroke-linejoin="round" stroke-linecap="round"/>"#);
}

/// Bajt tekstu WMF → znak: czcionka Symbol (litery greckie i znaki matematyczne), polska strona
/// kodowa (charset 238) albo Windows-1252.
fn decode(b: u8, face: &str, charset: u8) -> char {
    let f = face.to_ascii_lowercase();
    if charset == 2 || f.contains("symbol") {
        let greek = "ΑΒΧΔΕΦΓΗΙϑΚΛΜΝΟΠΘΡΣΤΥςΩΞΨΖ";
        let lower = "αβχδεφγηιϕκλμνοπθρστυϖωξψζ";
        return match b {
            b'A'..=b'Z' => greek.chars().nth((b - b'A') as usize).unwrap_or(b as char),
            b'a'..=b'z' => lower.chars().nth((b - b'a') as usize).unwrap_or(b as char),
            0x22 => '∀',
            0x24 => '∃',
            0x27 => '∋',
            0x2a => '∗',
            0x2d => '−',
            0x40 => '≅',
            0x5c => '∴',
            0x5e => '⊥',
            0x60 => '‾',
            0xa2 => '′',
            0xa3 => '≤',
            0xa5 => '∞',
            0xa6 => 'ƒ',
            0xab => '↔',
            0xac => '←',
            0xad => '↑',
            0xae => '→',
            0xaf => '↓',
            0xb0 => '°',
            0xb1 => '±',
            0xb2 => '″',
            0xb3 => '≥',
            0xb4 => '×',
            0xb5 => '∝',
            0xb6 => '∂',
            0xb7 => '•',
            0xb8 => '÷',
            0xb9 => '≠',
            0xba => '≡',
            0xbb => '≈',
            0xbc => '…',
            0xc4 => '⊗',
            0xc5 => '⊕',
            0xc6 => '∅',
            0xc7 => '∩',
            0xc8 => '∪',
            0xc9 => '⊃',
            0xca => '⊇',
            0xcb => '⊄',
            0xcc => '⊂',
            0xcd => '⊆',
            0xce => '∈',
            0xcf => '∉',
            0xd0 => '∠',
            0xd1 => '∇',
            0xd5 => '∏',
            0xd6 => '√',
            0xd7 => '⋅',
            0xd8 => '¬',
            0xd9 => '∧',
            0xda => '∨',
            0xdb => '⇔',
            0xdc => '⇐',
            0xdd => '⇑',
            0xde => '⇒',
            0xdf => '⇓',
            0xe1 => '〈',
            0xe5 => '∑',
            0xe6 => '⎛',
            0xe7 => '⎜',
            0xe8 => '⎝',
            0xe9 => '⎡',
            0xea => '⎢',
            0xeb => '⎣',
            0xec => '⎧',
            0xed => '⎨',
            0xee => '⎩',
            0xef => '⎪',
            0xf1 => '〉',
            0xf2 => '∫',
            0xf3 => '⌠',
            0xf4 => '⎮',
            0xf5 => '⌡',
            0xf6 => '⎞',
            0xf7 => '⎟',
            0xf8 => '⎠',
            0xf9 => '⎤',
            0xfa => '⎥',
            0xfb => '⎦',
            0xfc => '⎫',
            0xfd => '⎬',
            0xfe => '⎭',
            _ => b as char,
        };
    }
    if b < 0x80 {
        return b as char;
    }
    if charset == 238 {
        let pl = match b {
            0xb9 => Some('ą'),
            0xe6 => Some('ć'),
            0xea => Some('ę'),
            0xb3 => Some('ł'),
            0xf1 => Some('ń'),
            0xf3 => Some('ó'),
            0x9c => Some('ś'),
            0x9f => Some('ź'),
            0xbf => Some('ż'),
            0xa5 => Some('Ą'),
            0xc6 => Some('Ć'),
            0xca => Some('Ę'),
            0xa3 => Some('Ł'),
            0xd1 => Some('Ń'),
            0xd3 => Some('Ó'),
            0x8c => Some('Ś'),
            0x8f => Some('Ź'),
            0xaf => Some('Ż'),
            _ => None,
        };
        if let Some(c) = pl {
            return c;
        }
    }
    match b {
        0x80 => '€',
        0x84 => '„',
        0x85 => '…',
        0x91 => '‘',
        0x92 => '’',
        0x93 => '“',
        0x94 => '”',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        0x99 => '™',
        _ => char::from_u32(b as u32).unwrap_or('?'),
    }
}

/// Tekst jako krzywe glifów. `dx` – odstępy znaków w jednostkach logicznych (EXTTEXTOUT).
#[allow(clippy::too_many_arguments)]
fn text_out(out: &mut String, dc: &Dc, s: &[u8], x: f32, y: f32, dx: &[f32], tx: &dyn Fn(&Dc, f32) -> f32, ty: &dyn Fn(&Dc, f32) -> f32, w: f32, h: f32) {
    let Obj::Font { height, weight, italic, face, charset, .. } = &dc.font else { return };
    // Wysokość czcionki: ujemna – sam znak, dodatnia – z odstępem wiersza.
    let em = (height.abs() / dc.ext.1 * h).abs() * if *height > 0.0 { 0.82 } else { 1.0 };
    if em < 0.2 || s.is_empty() {
        return;
    }
    let family = match face.to_ascii_lowercase().as_str() {
        f if f.contains("symbol") || f.contains("mt extra") => "Cambria Math".to_string(),
        _ if face.is_empty() => "Times New Roman".to_string(),
        _ => face.clone(),
    };
    let props = CharProps { font: family, size: em, bold: *weight >= 600, italic: *italic, ..CharProps::default() };
    let chars: Vec<char> = s.iter().map(|&b| decode(b, face, *charset)).collect();
    // Szerokość całości (do wyrównania do środka/prawej).
    let sx = (w / dc.ext.0).abs();
    let total: f32 = if dx.len() == chars.len() { dx.iter().map(|d| d * sx).sum() } else { text::measure(&chars.iter().collect::<String>(), &props).1 };
    let mut pen = tx(dc, x);
    match dc.text_align & 0x6 {
        6 => pen -= total / 2.0,
        2 => pen -= total,
        _ => {}
    }
    let base = ty(dc, y);
    let (asc, desc, _) = text::vmetrics(fonts::pick(&props.font, props.bold, props.italic).id, em);
    let baseline = match dc.text_align & 0x18 {
        24 => base,
        8 => base - desc,
        _ => base + asc,
    };
    let mut d = String::new();
    for (k, c) in chars.iter().enumerate() {
        let (gs, adv) = text::measure(&c.to_string(), &props);
        let mut gx = pen;
        for g in &gs {
            glyph_path(&mut d, g.font, g.gid, g.size, gx + g.dx, baseline - g.dy, *italic && fonts::get(g.font).is_some_and(|f| !f.italic));
            gx += g.adv;
        }
        pen += dx.get(k).map(|v| v * sx).unwrap_or(adv);
    }
    if !d.is_empty() {
        let _ = write!(out, r#"<path d="{d}" fill="{}"/>"#, css(dc.text_color));
    }
}

struct PathOut<'a> {
    d: &'a mut String,
    k: f32,
    ox: f32,
    oy: f32,
    skew: f32,
}

impl PathOut<'_> {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        (self.ox + (x + y * self.skew) * self.k, self.oy - y * self.k)
    }
}

impl ttf_parser::OutlineBuilder for PathOut<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (a, b) = self.p(x, y);
        let _ = write!(self.d, "M{a:.2} {b:.2}");
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (a, b) = self.p(x, y);
        let _ = write!(self.d, "L{a:.2} {b:.2}");
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (a, b) = self.p(x1, y1);
        let (c, e) = self.p(x, y);
        let _ = write!(self.d, "Q{a:.2} {b:.2} {c:.2} {e:.2}");
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b) = self.p(x1, y1);
        let (c, e) = self.p(x2, y2);
        let (f, g) = self.p(x, y);
        let _ = write!(self.d, "C{a:.2} {b:.2} {c:.2} {e:.2} {f:.2} {g:.2}");
    }
    fn close(&mut self) {
        self.d.push('Z');
    }
}

fn glyph_path(d: &mut String, font: u32, gid: u16, size: f32, x: f32, y: f32, fake_italic: bool) {
    let Some(f) = fonts::get(font) else { return };
    let mut o = PathOut { d, k: size / f.upem, ox: x, oy: y, skew: if fake_italic { 0.2 } else { 0.0 } };
    let _ = f.face.outline_glyph(ttf_parser::GlyphId(gid), &mut o);
}

/// DIB (BITMAPINFOHEADER + piksele) z metapliku → PNG.
fn dib_to_png(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.len() < 40 {
        return None;
    }
    let hdr = u32_at(dib, 0) as usize;
    let bits = u16_at(dib, 14) as u32;
    let used = u32_at(dib, 32);
    let colors = if bits <= 8 { if used > 0 { used } else { 1 << bits } } else { 0 };
    let off = 14 + hdr as u32 + colors * 4;
    // Dokładamy nagłówek pliku BMP i dekodujemy jak zwykły obraz.
    let mut bmp = Vec::with_capacity(dib.len() + 14);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((dib.len() + 14) as u32).to_le_bytes());
    bmp.extend_from_slice(&[0, 0, 0, 0]);
    bmp.extend_from_slice(&off.to_le_bytes());
    bmp.extend_from_slice(dib);
    let img = image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp).ok()?;
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    Some(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(func: u16, params: &[i16]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&((3 + params.len()) as u32).to_le_bytes());
        v.extend_from_slice(&func.to_le_bytes());
        for p in params {
            v.extend_from_slice(&p.to_le_bytes());
        }
        v
    }

    #[test]
    fn rectangle_becomes_svg_path() {
        let mut d = vec![0xd7, 0xcd, 0xc6, 0x9a, 0, 0];
        for v in [0i16, 0, 1440, 720] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&1440u16.to_le_bytes());
        d.extend_from_slice(&[0; 6]);
        d.extend_from_slice(&[1, 0, 9, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        d.extend(rec(0x020c, &[720, 1440]));
        d.extend(rec(0x041b, &[600, 1200, 100, 100]));
        d.extend(rec(0x0000, &[]));
        let (svg, w, h) = to_svg(&d).unwrap();
        assert!((w - 72.0).abs() < 0.01 && (h - 36.0).abs() < 0.01);
        assert!(svg.contains("<path"));
    }
}
