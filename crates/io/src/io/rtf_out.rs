//! Eksport RTF – dla WordPada, starszych edytorów i programów pocztowych: treść główna z pełnym
//! formatowaniem znaków i akapitów, nagłówki z poziomem konspektu, łącza jako pola HYPERLINK,
//! listy jako tekst ze znacznikiem. Śledzone zmiany są przyjęte, obiekty pominięte.

use std::fmt::Write as _;

use crate::model::*;

fn esc(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\t' => out.push_str("\\tab "),
            '\n' | '\u{2028}' => out.push_str("\\line "),
            '\u{a0}' => out.push_str("\\~"),
            FIELD_CHAR => {}
            c if (c as u32) < 0x80 => out.push(c),
            c => {
                // \uN z liczbą ze znakiem (16 bitów), znak zastępczy „?”; spoza BMP – para zastępcza.
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf).iter() {
                    let _ = write!(out, "\\u{}?", *u as i16);
                }
            }
        }
    }
}

struct Tables {
    fonts: Vec<String>,
    colors: Vec<Color>,
}

impl Tables {
    fn font(&mut self, f: &str) -> usize {
        match self.fonts.iter().position(|x| x == f) {
            Some(i) => i,
            None => {
                self.fonts.push(f.to_string());
                self.fonts.len() - 1
            }
        }
    }

    /// Indeks w tabeli kolorów (0 – kolor domyślny, więc od 1).
    fn color(&mut self, c: Color) -> usize {
        let c = Color { a: 255, ..c };
        match self.colors.iter().position(|x| *x == c) {
            Some(i) => i + 1,
            None => {
                self.colors.push(c);
                self.colors.len()
            }
        }
    }
}

fn twips(pt: f32) -> i32 {
    (pt * 20.0).round() as i32
}

pub fn export(doc: &Document) -> Vec<u8> {
    let mut t = Tables { fonts: Vec::new(), colors: Vec::new() };
    let mut body = String::new();
    let mut counters = [0u32; 10];
    for p in &doc.body.paras {
        let (pp, base) = doc.resolve_para(p);
        body.push_str("\\pard\\plain");
        match pp.align {
            Align::Left => {}
            Align::Center => body.push_str("\\qc"),
            Align::Right => body.push_str("\\qr"),
            Align::Justify => body.push_str("\\qj"),
        }
        let _ = write!(body, "\\li{}\\ri{}\\fi{}\\sb{}\\sa{}", twips(pp.indent_left), twips(pp.indent_right), twips(pp.first_line), twips(pp.space_before), twips(pp.space_after));
        if let Some(l) = doc.outline_level(p) {
            let _ = write!(body, "\\outlinelevel{}", l.saturating_sub(1).min(8));
        }
        if pp.keep_next {
            body.push_str("\\keepn");
        }
        if pp.break_before == Break::Page {
            body.push_str("\\pagebb");
        }
        body.push(' ');
        // Znacznik listy jako tekst – działa w każdym czytniku RTF.
        if let Some(l) = &pp.list {
            let lvl = (l.level as usize).min(9);
            counters[lvl] += 1;
            for c in counters.iter_mut().skip(lvl + 1) {
                *c = 0;
            }
            let n = if counters[lvl] == 1 { l.start.unwrap_or(1) } else { counters[lvl] + l.start.unwrap_or(1) - 1 };
            esc(&format!("{}\t", l.kind.marker(n, l.level)), &mut body);
        } else {
            counters = [0; 10];
        }
        for r in &p.runs {
            if r.text.is_empty() || matches!(r.fmt.rev.as_ref().map(|x| x.kind), Some(RevKind::Delete)) {
                continue;
            }
            let mut f = r.fmt.clone();
            f.rev = None;
            let mut cp = base.clone();
            cp.apply(&f);
            let mut run = String::from("{");
            let _ = write!(run, "\\f{}\\fs{}", t.font(&cp.font), (cp.size * 2.0).round() as i32);
            if cp.bold {
                run.push_str("\\b");
            }
            if cp.italic {
                run.push_str("\\i");
            }
            if cp.underline != Underline::None {
                run.push_str(match cp.underline {
                    Underline::Double => "\\uldb",
                    Underline::Dotted => "\\uld",
                    Underline::Wave => "\\ulwave",
                    _ => "\\ul",
                });
            }
            if cp.strike {
                run.push_str("\\strike");
            }
            match cp.script {
                Script::Super => run.push_str("\\super"),
                Script::Sub => run.push_str("\\sub"),
                Script::Normal => {}
            }
            match cp.caps {
                Caps::All => run.push_str("\\caps"),
                Caps::Small => run.push_str("\\scaps"),
                Caps::None => {}
            }
            if cp.color.a > 0 && (cp.color.r, cp.color.g, cp.color.b) != (0, 0, 0) {
                let _ = write!(run, "\\cf{}", t.color(cp.color));
            }
            if cp.highlight.a > 0 {
                let _ = write!(run, "\\chcbpat{}", t.color(cp.highlight));
            }
            run.push(' ');
            esc(&r.text, &mut run);
            run.push('}');
            match cp.link.as_deref().filter(|l| !internal_link(l)) {
                Some(url) => {
                    let mut u = String::new();
                    esc(url, &mut u);
                    let _ = write!(body, "{{\\field{{\\*\\fldinst{{HYPERLINK \"{u}\"}}}}{{\\fldrslt{run}}}}}");
                }
                None => body.push_str(&run),
            }
        }
        body.push_str("\\par\n");
    }

    let mut out = String::from("{\\rtf1\\ansi\\ansicpg1250\\deff0\\uc1\n{\\fonttbl");
    if t.fonts.is_empty() {
        t.fonts.push("Calibri".into());
    }
    for (i, f) in t.fonts.iter().enumerate() {
        let mut name = String::new();
        esc(f, &mut name);
        let _ = write!(out, "{{\\f{i}\\fnil\\fcharset238 {name};}}");
    }
    out.push_str("}\n{\\colortbl;");
    for c in &t.colors {
        let _ = write!(out, "\\red{}\\green{}\\blue{};", c.r, c.g, c.b);
    }
    out.push_str("}\n{\\info");
    for (k, v) in [("title", &doc.meta.title), ("author", &doc.meta.author), ("subject", &doc.meta.subject), ("keywords", &doc.meta.keywords)] {
        if !v.is_empty() {
            let mut e = String::new();
            esc(v, &mut e);
            let _ = write!(out, "{{\\{k} {e}}}");
        }
    }
    let pg = &doc.page;
    let _ = write!(
        out,
        "}}\n\\paperw{}\\paperh{}\\margl{}\\margr{}\\margt{}\\margb{}{}\n",
        twips(pg.width),
        twips(pg.height),
        twips(pg.margin_left),
        twips(pg.margin_right),
        twips(pg.margin_top),
        twips(pg.margin_bottom),
        if pg.width > pg.height { "\\landscape" } else { "" }
    );
    out.push_str(&body);
    out.push('}');
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_through_import() {
        let mut d = Document::new();
        d.meta.title = "Zażółć".into();
        d.body = Story::plain("Pierwszy akapit {z klamrą}\nGęślą jaźń", "normal");
        d.body.paras[0].runs[0].fmt.bold = Some(true);
        let bytes = export(&d);
        let back = crate::io::rtf_in::import(&bytes).unwrap();
        assert_eq!(back.meta.title, "Zażółć");
        assert_eq!(back.body.paras[0].text(), "Pierwszy akapit {z klamrą}");
        assert_eq!(back.body.paras[0].runs[0].fmt.bold, Some(true));
        assert_eq!(back.body.paras[1].text(), "Gęślą jaźń");
    }
}
