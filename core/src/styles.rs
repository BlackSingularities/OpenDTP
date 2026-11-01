//! Style wbudowane i motywy dokumentu (zestawy krojów i kolorów nakładane na style).

use crate::model::*;

fn st(id: &str, name: &str, based: Option<&str>, next: Option<&str>, para: ParaFmt, chars: CharFmt, outline: Option<u8>) -> Style {
    Style { id: id.into(), name: name.into(), based_on: based.map(Into::into), next: next.map(Into::into), para, chars, outline, gallery: true }
}

/// Style nowego dokumentu. Identyfikatory są stałe (eksport do DOCX mapuje je na style Worda).
pub fn defaults() -> Vec<Style> {
    let heading = |size: f32, before: f32, after: f32, color: Color, bold: bool| {
        (
            ParaFmt { space_before: Some(before), space_after: Some(after), keep_next: Some(true), line: Some(LineSpacing::Multiple(1.08)), ..Default::default() },
            CharFmt { font: Some("Calibri Light".into()), size: Some(size), color: Some(color), bold: Some(bold), ..Default::default() },
        )
    };
    let accent = Color::rgb(0x1f, 0x4e, 0x79);
    let (h1p, h1c) = heading(20.0, 18.0, 6.0, accent, false);
    let (h2p, h2c) = heading(15.0, 12.0, 4.0, accent, false);
    let (h3p, h3c) = heading(12.5, 10.0, 3.0, Color::rgb(0x2e, 0x2e, 0x2e), true);
    vec![
        st(
            "normal",
            "Normalny",
            None,
            None,
            ParaFmt { space_after: Some(8.0), line: Some(LineSpacing::Multiple(1.15)), ..Default::default() },
            CharFmt { font: Some("Calibri".into()), size: Some(11.0), color: Some(Color::rgb(0x1a, 0x1a, 0x1a)), ..Default::default() },
            None,
        ),
        st(
            "title",
            "Tytuł",
            Some("normal"),
            Some("subtitle"),
            ParaFmt { space_after: Some(4.0), line: Some(LineSpacing::Multiple(1.0)), ..Default::default() },
            CharFmt { font: Some("Calibri Light".into()), size: Some(30.0), tracking: Some(-0.5), color: Some(Color::rgb(0x11, 0x11, 0x11)), ..Default::default() },
            None,
        ),
        st(
            "subtitle",
            "Podtytuł",
            Some("normal"),
            Some("normal"),
            ParaFmt { space_after: Some(14.0), ..Default::default() },
            CharFmt { size: Some(14.0), color: Some(Color::rgb(0x5a, 0x5a, 0x5a)), tracking: Some(0.3), ..Default::default() },
            None,
        ),
        st("heading1", "Nagłówek 1", Some("normal"), Some("normal"), h1p, h1c, Some(1)),
        st("heading2", "Nagłówek 2", Some("normal"), Some("normal"), h2p, h2c, Some(2)),
        st("heading3", "Nagłówek 3", Some("normal"), Some("normal"), h3p, h3c, Some(3)),
        st(
            "quote",
            "Cytat",
            Some("normal"),
            Some("normal"),
            ParaFmt { indent_left: Some(28.0), indent_right: Some(28.0), space_before: Some(6.0), space_after: Some(10.0), align: Some(Align::Center), ..Default::default() },
            CharFmt { italic: Some(true), size: Some(12.5), color: Some(Color::rgb(0x40, 0x40, 0x40)), ..Default::default() },
            None,
        ),
        st(
            "callout",
            "Wyróżnienie",
            Some("normal"),
            Some("normal"),
            ParaFmt {
                shading: Some(Color::rgb(0xee, 0xf3, 0xf9)),
                border: Some(Some(BoxBorder { width: 0.0, color: Color::NONE, radius: 6.0, padding: 8.0 })),
                space_before: Some(6.0),
                space_after: Some(12.0),
                ..Default::default()
            },
            CharFmt { color: Some(Color::rgb(0x1f, 0x3a, 0x5a)), ..Default::default() },
            None,
        ),
        st(
            "caption",
            "Podpis",
            Some("normal"),
            Some("normal"),
            ParaFmt { space_after: Some(10.0), ..Default::default() },
            CharFmt { size: Some(9.0), italic: Some(true), color: Some(Color::rgb(0x5a, 0x5a, 0x5a)), ..Default::default() },
            None,
        ),
        st(
            "code",
            "Kod",
            Some("normal"),
            Some("normal"),
            ParaFmt { space_after: Some(0.0), line: Some(LineSpacing::Multiple(1.0)), shading: Some(Color::rgb(0xf4, 0xf4, 0xf4)), ..Default::default() },
            CharFmt { font: Some("Consolas".into()), size: Some(10.0), ..Default::default() },
            None,
        ),
        st(
            "list",
            "Akapit listy",
            Some("normal"),
            Some("list"),
            ParaFmt { space_after: Some(3.0), ..Default::default() },
            CharFmt::default(),
            None,
        ),
        st(
            "toc1",
            "Spis treści 1",
            Some("normal"),
            Some("normal"),
            ParaFmt { space_after: Some(4.0), tabs: Some(vec![-1.0]), ..Default::default() },
            CharFmt { bold: Some(true), ..Default::default() },
            None,
        ),
        st(
            "toc2",
            "Spis treści 2",
            Some("normal"),
            Some("normal"),
            ParaFmt { space_after: Some(2.0), indent_left: Some(14.0), tabs: Some(vec![-1.0]), ..Default::default() },
            CharFmt::default(),
            None,
        ),
        st(
            "toc3",
            "Spis treści 3",
            Some("normal"),
            Some("normal"),
            ParaFmt { space_after: Some(2.0), indent_left: Some(28.0), tabs: Some(vec![-1.0]), ..Default::default() },
            CharFmt { size: Some(10.0), ..Default::default() },
            None,
        ),
        st(
            "header",
            "Nagłówek strony",
            Some("normal"),
            None,
            ParaFmt { space_after: Some(0.0), ..Default::default() },
            CharFmt { size: Some(9.0), color: Some(Color::rgb(0x6a, 0x6a, 0x6a)), ..Default::default() },
            None,
        ),
        st(
            "footer",
            "Stopka strony",
            Some("normal"),
            None,
            ParaFmt { space_after: Some(0.0), align: Some(Align::Center), ..Default::default() },
            CharFmt { size: Some(9.0), color: Some(Color::rgb(0x6a, 0x6a, 0x6a)), ..Default::default() },
            None,
        ),
    ]
    .into_iter()
    .map(|mut s| {
        if s.id.starts_with("toc") || s.id == "header" || s.id == "footer" || s.id == "list" {
            s.gallery = false;
        }
        s
    })
    .collect()
}

/// Motyw dokumentu: kroje i barwy nakładane na style (karta „Projekt”).
pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub body: &'static str,
    pub heading: &'static str,
    pub accent: Color,
    pub text: Color,
}

pub const THEMES: &[Theme] = &[
    Theme { id: "classic", name: "Klasyczny", body: "Calibri", heading: "Calibri Light", accent: Color::rgb(0x1f, 0x4e, 0x79), text: Color::rgb(0x1a, 0x1a, 0x1a) },
    Theme { id: "editorial", name: "Redakcyjny", body: "Georgia", heading: "Georgia", accent: Color::rgb(0x8a, 0x1c, 0x1c), text: Color::rgb(0x22, 0x1e, 0x1a) },
    Theme { id: "modern", name: "Nowoczesny", body: "Segoe UI", heading: "Segoe UI Light", accent: Color::rgb(0x00, 0x6d, 0x77), text: Color::rgb(0x15, 0x15, 0x15) },
    Theme { id: "elegant", name: "Elegancki", body: "Garamond", heading: "Constantia", accent: Color::rgb(0x5b, 0x3a, 0x7a), text: Color::rgb(0x20, 0x1c, 0x24) },
    Theme { id: "technical", name: "Techniczny", body: "Bahnschrift Light", heading: "Bahnschrift", accent: Color::rgb(0xd0, 0x5a, 0x00), text: Color::rgb(0x18, 0x18, 0x18) },
    Theme { id: "corporate", name: "Firmowy", body: "Corbel", heading: "Candara", accent: Color::rgb(0x2b, 0x57, 0x9a), text: Color::rgb(0x1c, 0x1c, 0x1c) },
    Theme { id: "noir", name: "Czerń i biel", body: "Cambria", heading: "Segoe UI Black", accent: Color::rgb(0x00, 0x00, 0x00), text: Color::rgb(0x00, 0x00, 0x00) },
];

/// Nakłada motyw na style dokumentu.
pub fn apply_theme(doc: &mut Document, theme: &Theme) {
    for s in doc.styles.iter_mut() {
        let is_heading = s.outline.is_some() || s.id == "title";
        match s.id.as_str() {
            "normal" => {
                s.chars.font = Some(theme.body.into());
                s.chars.color = Some(theme.text);
            }
            "code" => {}
            "subtitle" => {
                s.chars.font = Some(theme.heading.into());
            }
            "callout" => {
                let c = theme.accent;
                s.para.shading = Some(Color::rgb(mix(c.r), mix(c.g), mix(c.b)));
                s.chars.color = Some(c);
            }
            _ if is_heading => {
                s.chars.font = Some(theme.heading.into());
                if s.id != "title" && s.id != "heading3" {
                    s.chars.color = Some(theme.accent);
                }
            }
            _ => {}
        }
    }
}

fn mix(v: u8) -> u8 {
    (v as f32 * 0.08 + 255.0 * 0.92) as u8
}
