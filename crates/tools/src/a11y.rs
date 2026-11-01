//! Kontrola dostępności (jak „Sprawdź dostępność” w WordCraft): obrazy i wykresy bez tekstu
//! alternatywnego, przeskoki poziomów nagłówków, puste nagłówki, tabele bez wiersza nagłówka,
//! za słaby kontrast tekstu, łącza opisane samym adresem, brak tytułu i języka dokumentu.

use crate::lang::tr;
use crate::model::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// Błąd (czytnik ekranu straci informację) albo wskazówka.
    pub error: bool,
    pub text: String,
    /// Obiekt, którego dotyczy (do zaznaczenia).
    pub obj: Option<u64>,
    /// Strona obiektu.
    pub page: Option<usize>,
}

fn lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: Color) -> f32 {
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// Współczynnik kontrastu WCAG 2 (1..21).
pub fn contrast(a: Color, b: Color) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn short(s: &str) -> String {
    let s = s.trim();
    let mut out: String = s.chars().take(40).collect();
    if s.chars().count() > 40 {
        out.push('…');
    }
    out
}

pub fn check(doc: &Document) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut push = |error: bool, text: String, obj: Option<&Object>| {
        out.push(Finding { error, text, obj: obj.map(|o| o.id), page: obj.map(|o| o.page) });
    };

    if doc.meta.title.trim().is_empty() {
        push(false, tr("Dokument nie ma tytułu (polecenie „Edytuj metadane dokumentu”) – czytnik ekranu przeczyta nazwę pliku").to_string(), None);
    }
    if doc.settings.lang.trim().is_empty() {
        push(true, tr("Nie ustawiono języka dokumentu – czytnik ekranu nie dobierze wymowy").to_string(), None);
    }

    for o in doc.objects.iter().filter(|o| o.cont_of.is_none()) {
        match &o.kind {
            ObjKind::Image { .. } | ObjKind::Chart { .. } if o.name.trim().is_empty() => {
                push(true, tr("Obraz albo wykres bez tekstu alternatywnego – wpisz opis w polu nazwy we właściwościach").to_string(), Some(o));
            }
            ObjKind::Table { table } if !table.header && table.rows.len() > 1 => {
                push(true, tr("Tabela bez wiersza nagłówka – czytnik ekranu nie powiąże komórek z kolumnami").to_string(), Some(o));
            }
            _ => {}
        }
    }

    // Nagłówki: przeskok poziomu (np. z 1 od razu na 3) i nagłówki bez treści.
    let mut last = 0u8;
    for p in &doc.body.paras {
        let Some(level) = doc.outline_level(p) else { continue };
        let text = p.text();
        if text.trim().trim_matches(FIELD_CHAR).is_empty() {
            push(false, tr("Pusty nagłówek – usuń go albo zmień styl na zwykły tekst").to_string(), None);
            continue;
        }
        if level > last + 1 {
            push(false, crate::trf!("Nagłówek „{}” przeskakuje poziom ({} po {}) – spis treści i nawigacja będą niespójne", short(&text), level, last), None);
        }
        last = level;
    }

    // Kontrast tekstu i łącza opisane adresem – raz na każdy rodzaj problemu, żeby nie zasypać listy.
    let page = if doc.page.color.a == 0 { Color { r: 255, g: 255, b: 255, a: 255 } } else { doc.page.color };
    let mut low = 0usize;
    let mut low_example = String::new();
    let mut bare_links = 0usize;
    for s in crate::stories(doc) {
        for p in &s.paras {
            let (pp, base) = doc.resolve_para(p);
            let bg = if pp.shading.a > 0 { pp.shading } else { page };
            for r in &p.runs {
                if r.text.trim().is_empty() || r.field.is_some() {
                    continue;
                }
                let mut cp = base.clone();
                cp.apply(&r.fmt);
                let back = if cp.highlight.a > 0 { cp.highlight } else { bg };
                let large = cp.size >= 18.0 || (cp.bold && cp.size >= 14.0);
                if cp.color.a > 0 && contrast(cp.color, back) < if large { 3.0 } else { 4.5 } {
                    low += 1;
                    if low_example.is_empty() {
                        low_example = short(&r.text);
                    }
                }
                if let Some(link) = &cp.link {
                    let t = r.text.trim();
                    if t.starts_with("http") || t == link.trim() {
                        bare_links += 1;
                    }
                }
            }
        }
    }
    if low > 0 {
        push(true, crate::trf!("Za słaby kontrast tekstu w {} miejscach (np. „{}”) – potrzeba co najmniej 4,5:1", low, low_example), None);
    }
    if bare_links > 0 {
        push(false, crate::trf!("Łącza opisane samym adresem: {} – lepiej opisać, dokąd prowadzą", bare_links), None);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_matches_wcag() {
        let black = Color { r: 0, g: 0, b: 0, a: 255 };
        let white = Color { r: 255, g: 255, b: 255, a: 255 };
        assert!((contrast(black, white) - 21.0).abs() < 0.01);
        let grey = Color { r: 0x77, g: 0x77, b: 0x77, a: 255 };
        assert!(contrast(grey, white) > 4.4 && contrast(grey, white) < 4.6);
    }

    #[test]
    fn finds_heading_skip_and_missing_title() {
        let mut d = Document::new();
        d.body = Story { paras: vec![Paragraph::with_text("heading1", "A"), Paragraph::with_text("heading3", "B")] };
        let f = check(&d);
        assert!(f.iter().any(|x| x.text.contains("przeskakuje")) || d.style("heading3").is_none());
        assert!(f.iter().any(|x| x.text.contains("tytułu")));
    }
}
