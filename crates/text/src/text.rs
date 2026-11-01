//! Kształtowanie akapitu: rozstrzygnięte style przebiegów, zastępstwa czcionek dla brakujących
//! znaków, pola (numer strony…), wersaliki i kapitaliki, a na końcu glify z rustybuzz wraz
//! z miejscami, w których wolno złamać wiersz.

use std::str::FromStr;

use crate::fonts::{self, FontId};
use crate::model::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GKind {
    Normal,
    Space,
    Tab,
    /// Złamanie wiersza wewnątrz akapitu (Shift+Enter).
    Newline,
}

#[derive(Clone, Copy, Debug)]
pub struct SGlyph {
    pub font: FontId,
    pub gid: u16,
    /// Przesunięcie pióra w punktach (z rozstrzelaniem i skalą).
    pub adv: f32,
    pub dx: f32,
    pub dy: f32,
    /// Przesunięcie w tekście akapitu (bajty) – początek grona znaków.
    pub cluster: usize,
    pub style: u16,
    /// Rozmiar rysowania (indeksy i kapitaliki – mniejszy).
    pub size: f32,
    pub kind: GKind,
    pub fake_bold: bool,
    pub fake_italic: bool,
}

pub struct ShapedPara {
    pub text: String,
    pub glyphs: Vec<SGlyph>,
    pub styles: Vec<CharProps>,
    /// Indeksy glifów, od których wolno zacząć nowy wiersz (rosnąco, bez 0).
    pub breaks: Vec<usize>,
    /// Styl końca akapitu (pusty akapit, znacznik listy).
    pub end_style: u16,
    /// Miejsca podziału wyrazu (indeksy glifów, także w `breaks`) – na końcu wiersza dostają dywiz.
    pub hyph: Vec<usize>,
}

impl ShapedPara {
    pub fn style(&self, g: &SGlyph) -> &CharProps {
        &self.styles[g.style as usize]
    }
}

/// Wartości pól w danym miejscu.
pub struct FieldValues<'a> {
    pub page: u32,
    pub pages: u32,
    pub title: &'a str,
    pub author: &'a str,
    /// Numery przypisów: (id, numer, końcowy).
    pub notes: &'a [(u64, u32, bool)],
    /// Numery podpisów i strony celów odsyłaczy (brak – pola pokazują „?”).
    pub ext: Option<&'a FieldExtra>,
    /// Tekst żywej paginy na tej stronie.
    pub running: &'a str,
}

/// Wartości zależne od całego składu: numery podpisów i położenie celów odsyłaczy.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldExtra {
    /// (id podpisu, numer).
    pub seqs: Vec<(u64, u32)>,
    /// Cel odsyłacza: id → (numer strony, tekst akapitu celu).
    pub targets: std::collections::HashMap<u64, (u32, String)>,
    /// Metadane dla pól `Field::Meta`, w kolejności `MetaKey`.
    pub meta: [String; 4],
}

/// Numeracja rzymska (małe litery) – przypisy końcowe.
pub fn roman(mut n: u32) -> String {
    const T: [(u32, &str); 13] = [(1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"), (50, "l"), (40, "xl"), (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i")];
    let mut s = String::new();
    for (v, r) in T {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

impl FieldValues<'_> {
    fn seq(&self, id: u64) -> Option<u32> {
        self.ext?.seqs.iter().find(|s| s.0 == id).map(|s| s.1)
    }

    pub fn text(&self, f: Field) -> String {
        match f {
            Field::PageNumber => self.page.to_string(),
            Field::PageCount => self.pages.to_string(),
            Field::Date => crate::util::today(),
            Field::Time => crate::util::now_time(),
            Field::Title => self.title.to_string(),
            Field::Author => self.author.to_string(),
            Field::Anchor(_) | Field::Bookmark(_) | Field::IndexMark(_) | Field::Comment(_) => String::new(),
            Field::Running(_) => self.running.to_string(),
            Field::Meta(k) => self.ext.map(|e| e.meta[k.index()].clone()).unwrap_or_default(),
            Field::Seq(id, _) => self.seq(id).map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
            Field::Ref(id, kind) => {
                let t = self.ext.and_then(|e| e.targets.get(&id));
                match kind {
                    RefKind::Page => t.map(|t| t.0.to_string()),
                    RefKind::Number => self.seq(id).map(|n| n.to_string()).or_else(|| t.map(|t| t.0.to_string())),
                    RefKind::Text => t.map(|t| t.1.clone()),
                }
                .unwrap_or_else(|| "?".into())
            }
            Field::Footnote(id) => match self.notes.iter().find(|n| n.0 == id) {
                Some((_, k, true)) => roman(*k),
                Some((_, k, false)) => k.to_string(),
                None => "?".into(),
            },
        }
    }
}

struct DChar {
    c: char,
    cluster: usize,
    style: u16,
    small: bool,
}

/// Krótkie polskie wyrazy, po których nie łamiemy wiersza (sierotki).
fn orphan_word(w: &str) -> bool {
    let w = w.trim_matches(|c: char| c.is_whitespace() || c == '(' || c == '„' || c == '"');
    let mut it = w.chars();
    match (it.next(), it.next()) {
        (Some(c), None) => "aiouwzAIOUWZ".contains(c) || c.is_ascii_digit(),
        _ => false,
    }
}

pub fn shape(p: &Paragraph, base: &CharProps, fields: &FieldValues, orphans: bool) -> ShapedPara {
    shape_ex(p, base, fields, orphans, false)
}

fn dictionary(lang: &str) -> Option<&'static hyphenation::Standard> {
    use hyphenation::{Language, Load, Standard};
    use std::sync::OnceLock;
    static PL: OnceLock<Option<Standard>> = OnceLock::new();
    static EN: OnceLock<Option<Standard>> = OnceLock::new();
    if lang.starts_with("en") {
        EN.get_or_init(|| Standard::from_embedded(Language::EnglishUS).ok()).as_ref()
    } else {
        PL.get_or_init(|| Standard::from_embedded(Language::Polish).ok()).as_ref()
    }
}

/// Kształtowanie z opcjonalnym dzieleniem wyrazów (wzorce Knutha–Lianga dla języka tekstu).
pub fn shape_ex(p: &Paragraph, base: &CharProps, fields: &FieldValues, orphans: bool, hyphenate: bool) -> ShapedPara {
    let mut styles: Vec<CharProps> = Vec::with_capacity(p.runs.len());
    let mut chars: Vec<DChar> = Vec::new();
    let mut text = String::new();
    for r in &p.runs {
        let mut cp = base.clone();
        cp.apply(&r.fmt);
        let si = match styles.iter().position(|s| *s == cp) {
            Some(i) => i,
            None => {
                styles.push(cp.clone());
                styles.len() - 1
            }
        } as u16;
        let start = text.len();
        text.push_str(&r.text);
        if let Some(f) = r.field {
            for c in fields.text(f).chars() {
                chars.push(DChar { c, cluster: start, style: si, small: false });
            }
            continue;
        }
        for (i, c) in r.text.char_indices() {
            let cl = start + i;
            match cp.caps {
                Caps::None => chars.push(DChar { c, cluster: cl, style: si, small: false }),
                Caps::All => {
                    for u in c.to_uppercase() {
                        chars.push(DChar { c: u, cluster: cl, style: si, small: false });
                    }
                }
                Caps::Small => {
                    let small = c.is_lowercase();
                    for u in c.to_uppercase() {
                        chars.push(DChar { c: u, cluster: cl, style: si, small });
                    }
                }
            }
        }
    }
    if styles.is_empty() {
        styles.push(base.clone());
    }
    let end_style = p.runs.last().and_then(|r| {
        let mut cp = base.clone();
        cp.apply(&r.fmt);
        styles.iter().position(|s| *s == cp)
    }).unwrap_or(0) as u16;

    // Krój dla każdego znaku (z zastępstwem) i podział na kawałki do kształtowania.
    let picks: Vec<fonts::Pick> = styles.iter().map(|s| fonts::pick(&s.font, s.bold, s.italic)).collect();
    let mut glyphs: Vec<SGlyph> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let d = &chars[i];
        if d.c == '\n' || d.c == '\u{2028}' || d.c == '\t' {
            let st = &styles[d.style as usize];
            let pk = picks[d.style as usize];
            glyphs.push(SGlyph {
                font: pk.id,
                gid: 0,
                adv: 0.0,
                dx: 0.0,
                dy: 0.0,
                cluster: d.cluster,
                style: d.style,
                size: st.draw_size(),
                kind: if d.c == '\t' { GKind::Tab } else { GKind::Newline },
                fake_bold: pk.fake_bold,
                fake_italic: pk.fake_italic,
            });
            i += 1;
            continue;
        }
        let font_for = |d: &DChar| -> (FontId, bool, bool) {
            let pk = picks[d.style as usize];
            let ok = fonts::get(pk.id).is_some_and(|l| l.has(d.c)) || d.c.is_whitespace() || d.c.is_control();
            if ok {
                (pk.id, pk.fake_bold, pk.fake_italic)
            } else {
                match fonts::fallback_for(d.c) {
                    Some(f) => (f, styles[d.style as usize].bold, styles[d.style as usize].italic),
                    None => (pk.id, pk.fake_bold, pk.fake_italic),
                }
            }
        };
        let (font, fb, fi) = font_for(d);
        let (style, small) = (d.style, d.small);
        let mut j = i + 1;
        while j < chars.len() {
            let e = &chars[j];
            if e.style != style || e.small != small || matches!(e.c, '\n' | '\u{2028}' | '\t') || font_for(e).0 != font {
                break;
            }
            j += 1;
        }
        shape_chunk(&chars[i..j], font, fb, fi, &styles[style as usize], style, small, &mut glyphs);
        i = j;
    }

    // Miejsca złamania wiersza.
    let mut breaks = Vec::new();
    let mut prev = 0usize;
    for (pos, op) in unicode_linebreak::linebreaks(&text) {
        if pos >= text.len() {
            break;
        }
        if op == unicode_linebreak::BreakOpportunity::Allowed && orphans && orphan_word(&text[prev..pos]) {
            continue;
        }
        prev = pos;
        let gi = glyphs.partition_point(|g| g.cluster < pos);
        if gi > 0 && gi < glyphs.len() && breaks.last() != Some(&gi) {
            breaks.push(gi);
        }
    }
    // Po złamaniu wiersza (Shift+Enter) i tabulatorze zawsze można zacząć nowy wiersz.
    for (gi, g) in glyphs.iter().enumerate() {
        if g.kind == GKind::Newline && gi + 1 < glyphs.len() {
            breaks.push(gi + 1);
        }
    }
    // Podział wyrazów: dłuższe słowa (co najmniej 6 liter, po 2 litery z każdej strony).
    let mut hyph = Vec::new();
    if hyphenate {
        use hyphenation::Hyphenator;
        use unicode_segmentation::UnicodeSegmentation;
        for (i, w) in text.split_word_bound_indices() {
            if w.chars().count() < 6 || !w.chars().all(|c| c.is_alphabetic()) {
                continue;
            }
            let gi0 = glyphs.partition_point(|g| g.cluster < i);
            let lang = glyphs.get(gi0).map(|g| styles[g.style as usize].lang.clone()).unwrap_or_else(|| base.lang.clone());
            let Some(dict) = dictionary(&lang) else { continue };
            let lower = w.to_lowercase();
            if lower.len() != w.len() {
                continue;
            }
            for b in dict.hyphenate(&lower).breaks {
                let left = w[..b].chars().count();
                let right = w[b..].chars().count();
                if left < 2 || right < 2 {
                    continue;
                }
                let gi = glyphs.partition_point(|g| g.cluster < i + b);
                if gi > 0 && gi < glyphs.len() {
                    hyph.push(gi);
                    breaks.push(gi);
                }
            }
        }
    }
    breaks.sort_unstable();
    breaks.dedup();
    hyph.sort_unstable();
    hyph.dedup();
    ShapedPara { text, glyphs, styles, breaks, end_style, hyph }
}

#[allow(clippy::too_many_arguments)]
fn shape_chunk(chars: &[DChar], font: FontId, fake_bold: bool, fake_italic: bool, st: &CharProps, style: u16, small: bool, out: &mut Vec<SGlyph>) {
    let Some(l) = fonts::get(font) else { return };
    let mut s = String::new();
    let mut map: Vec<(usize, usize, char)> = Vec::new(); // (bajt w kawałku, klaster w akapicie, znak)
    for d in chars {
        map.push((s.len(), d.cluster, d.c));
        s.push(d.c);
    }
    let mut buf = rustybuzz::UnicodeBuffer::new();
    buf.push_str(&s);
    buf.guess_segment_properties();
    buf.set_direction(rustybuzz::Direction::LeftToRight);
    let mut feats = Vec::new();
    if !st.ligatures {
        for f in ["-liga", "-clig"] {
            if let Ok(f) = rustybuzz::Feature::from_str(f) {
                feats.push(f);
            }
        }
    }
    if !st.kerning {
        if let Ok(f) = rustybuzz::Feature::from_str("-kern") {
            feats.push(f);
        }
    }
    for f in st.features.split([',', ' ']).map(str::trim).filter(|f| !f.is_empty()) {
        if let Ok(f) = rustybuzz::Feature::from_str(f) {
            feats.push(f);
        }
    }
    let res = rustybuzz::shape(&l.hb, &feats, buf);
    let size = st.draw_size() * if small { 0.78 } else { 1.0 };
    let k = size / l.upem * st.scale / 100.0;
    let ky = size / l.upem;
    for (info, pos) in res.glyph_infos().iter().zip(res.glyph_positions()) {
        let bi = info.cluster as usize;
        let mi = map.partition_point(|m| m.0 <= bi).saturating_sub(1);
        let (_, cluster, c) = map[mi];
        let kind = if c == ' ' || c == '\u{00A0}' || c == '\u{3000}' { GKind::Space } else { GKind::Normal };
        out.push(SGlyph {
            font,
            gid: info.glyph_id as u16,
            adv: pos.x_advance as f32 * k + st.tracking,
            dx: pos.x_offset as f32 * k,
            dy: pos.y_offset as f32 * ky,
            cluster,
            style,
            size,
            kind,
            fake_bold,
            fake_italic,
        });
    }
}

/// Szerokość tekstu w jednym stylu (znaczniki list, wykresy, etykiety).
pub fn measure(text: &str, props: &CharProps) -> (Vec<SGlyph>, f32) {
    let p = Paragraph { style: String::new(), fmt: ParaFmt::default(), runs: vec![Run { text: text.into(), fmt: CharFmt::default(), field: None }] };
    let fv = FieldValues { page: 1, pages: 1, title: "", author: "", notes: &[], ext: None, running: "" };
    let sp = shape(&p, props, &fv, false);
    let w = sp.glyphs.iter().map(|g| g.adv).sum();
    (sp.glyphs, w)
}

/// Metryki pionowe glifu w punktach: (wznoszenie, obniżenie, odstęp) – dodatnie.
pub fn vmetrics(font: FontId, size: f32) -> (f32, f32, f32) {
    match fonts::get(font) {
        Some(l) => {
            let k = size / l.upem;
            (l.ascent * k, -l.descent * k, l.gap * k)
        }
        None => (size * 0.8, size * 0.2, 0.0),
    }
}
