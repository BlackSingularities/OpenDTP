//! Eksport ODT (OpenDocument) – dla LibreOffice i urzędowych wymagań formatu otwartego: treść
//! główna z formatowaniem znaków i akapitów (style automatyczne), nagłówki z poziomami, listy,
//! łącza, rozmiar strony i marginesy, tytuł i autor. Śledzone zmiany są przyjęte, obiekty pominięte.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;

use crate::model::*;

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut spaces = 0;
    for c in s.chars() {
        if c == ' ' {
            spaces += 1;
            continue;
        }
        flush_spaces(&mut out, &mut spaces);
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("<text:tab/>"),
            '\n' | '\u{2028}' => out.push_str("<text:line-break/>"),
            FIELD_CHAR => {}
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    flush_spaces(&mut out, &mut spaces);
    out
}

/// Kolejne spacje: pierwsza zwykła, reszta jako `text:s` (XML zwija powtórzone odstępy).
fn flush_spaces(out: &mut String, n: &mut usize) {
    if *n == 0 {
        return;
    }
    out.push(' ');
    if *n > 1 {
        let _ = write!(out, "<text:s text:c=\"{}\"/>", *n - 1);
    }
    *n = 0;
}

fn attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}

fn pt(v: f32) -> String {
    format!("{:.2}pt", v)
}

fn hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn text_props(cp: &CharProps) -> String {
    let mut s = format!(
        r#"<style:text-properties style:font-name="{f}" fo:font-family="'{f}'" fo:font-size="{size}" fo:font-weight="{w}" style:font-weight-asian="{w}" fo:font-style="{i}""#,
        f = attr(&cp.font),
        size = pt(cp.size),
        w = if cp.bold { "bold" } else { "normal" },
        i = if cp.italic { "italic" } else { "normal" }
    );
    if cp.underline != Underline::None {
        s.push_str(r#" style:text-underline-style="solid" style:text-underline-width="auto" style:text-underline-color="font-color""#);
        if cp.underline == Underline::Double {
            s.push_str(r#" style:text-underline-type="double""#);
        }
    }
    if cp.strike {
        s.push_str(r#" style:text-line-through-style="solid""#);
    }
    match cp.script {
        Script::Super => s.push_str(r#" style:text-position="super 58%""#),
        Script::Sub => s.push_str(r#" style:text-position="sub 58%""#),
        Script::Normal => {}
    }
    match cp.caps {
        Caps::All => s.push_str(r#" fo:text-transform="uppercase""#),
        Caps::Small => s.push_str(r#" fo:font-variant="small-caps""#),
        Caps::None => {}
    }
    if cp.color.a > 0 {
        let _ = write!(s, r#" fo:color="{}""#, hex(cp.color));
    }
    if cp.highlight.a > 0 {
        let _ = write!(s, r#" fo:background-color="{}""#, hex(cp.highlight));
    }
    s.push_str("/>");
    s
}

fn para_props(pp: &ParaProps) -> String {
    let align = match pp.align {
        Align::Left => "start",
        Align::Center => "center",
        Align::Right => "end",
        Align::Justify => "justify",
    };
    let mut s = format!(
        r#"<style:paragraph-properties fo:text-align="{align}" fo:margin-left="{}" fo:margin-right="{}" fo:text-indent="{}" fo:margin-top="{}" fo:margin-bottom="{}""#,
        pt(pp.indent_left),
        pt(pp.indent_right),
        pt(pp.first_line),
        pt(pp.space_before),
        pt(pp.space_after)
    );
    if pp.keep_next {
        s.push_str(r#" fo:keep-with-next="always""#);
    }
    if pp.break_before == Break::Page {
        s.push_str(r#" fo:break-before="page""#);
    }
    if pp.shading.a > 0 {
        let _ = write!(s, r#" fo:background-color="{}""#, hex(pp.shading));
    }
    s.push_str("/>");
    s
}

pub fn export(doc: &Document) -> Result<Vec<u8>, String> {
    let mut auto = String::new();
    let mut tstyles: HashMap<String, String> = HashMap::new();
    let mut pstyles: HashMap<String, String> = HashMap::new();
    let mut body = String::new();
    let mut fonts: Vec<String> = Vec::new();
    let mut in_list = false;
    for p in &doc.body.paras {
        let (pp, base) = doc.resolve_para(p);
        // Styl akapitu: własności akapitu + formatowanie znaków stylu.
        let pkey = format!("{}{}", para_props(&pp), text_props(&base));
        let n = pstyles.len() + 1;
        let pname = pstyles
            .entry(pkey.clone())
            .or_insert_with(|| {
                let _ = write!(auto, r#"<style:style style:name="P{n}" style:family="paragraph">{pkey}</style:style>"#);
                format!("P{n}")
            })
            .clone();
        if !fonts.contains(&base.font) {
            fonts.push(base.font.clone());
        }
        let mut inner = String::new();
        for r in &p.runs {
            if r.text.is_empty() || matches!(r.fmt.rev.as_ref().map(|x| x.kind), Some(RevKind::Delete)) {
                continue;
            }
            let mut f = r.fmt.clone();
            f.rev = None;
            let mut cp = base.clone();
            cp.apply(&f);
            if !fonts.contains(&cp.font) {
                fonts.push(cp.font.clone());
            }
            let mut piece = if cp == base {
                esc(&r.text)
            } else {
                let tkey = text_props(&cp);
                let n = tstyles.len() + 1;
                let tname = tstyles
                    .entry(tkey.clone())
                    .or_insert_with(|| {
                        let _ = write!(auto, r#"<style:style style:name="T{n}" style:family="text">{tkey}</style:style>"#);
                        format!("T{n}")
                    })
                    .clone();
                format!(r#"<text:span text:style-name="{tname}">{}</text:span>"#, esc(&r.text))
            };
            if let Some(url) = cp.link.as_deref().filter(|l| !internal_link(l)) {
                piece = format!(r#"<text:a xlink:type="simple" xlink:href="{}">{piece}</text:a>"#, attr(url));
            }
            inner.push_str(&piece);
        }
        // Listy: kolejne akapity listy w jednym `text:list`.
        let list = pp.list.as_ref();
        if list.is_some() && !in_list {
            let numbered = list.is_some_and(|l| l.kind.numbered());
            let _ = write!(body, r#"<text:list text:style-name="{}">"#, if numbered { "LNum" } else { "LBul" });
            in_list = true;
        } else if list.is_none() && in_list {
            body.push_str("</text:list>");
            in_list = false;
        }
        let tag = match doc.outline_level(p) {
            Some(l) => format!(r#"text:h text:outline-level="{}""#, l.max(1)),
            None => "text:p".to_string(),
        };
        let close = if tag.starts_with("text:h") { "text:h" } else { "text:p" };
        if in_list {
            body.push_str("<text:list-item>");
        }
        let _ = write!(body, r#"<{tag} text:style-name="{pname}">{inner}</{close}>"#);
        if in_list {
            body.push_str("</text:list-item>");
        }
    }
    if in_list {
        body.push_str("</text:list>");
    }

    let font_decls: String = fonts.iter().map(|f| format!(r#"<style:font-face style:name="{0}" svg:font-family="'{0}'"/>"#, attr(f))).collect();
    const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" office:version="1.3""#;
    let lists = r#"<text:list-style style:name="LBul"><text:list-level-style-bullet text:level="1" text:bullet-char="•"><style:list-level-properties text:list-level-position-and-space-mode="label-alignment"><style:list-level-label-alignment text:label-followed-by="listtab" fo:text-indent="-12pt" fo:margin-left="18pt"/></style:list-level-properties></text:list-level-style-bullet></text:list-style><text:list-style style:name="LNum"><text:list-level-style-number text:level="1" style:num-format="1" style:num-suffix="."><style:list-level-properties text:list-level-position-and-space-mode="label-alignment"><style:list-level-label-alignment text:label-followed-by="listtab" fo:text-indent="-14pt" fo:margin-left="20pt"/></style:list-level-properties></text:list-level-style-number></text:list-style>"#;
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {NS}><office:font-face-decls>{font_decls}</office:font-face-decls><office:automatic-styles>{auto}{lists}</office:automatic-styles><office:body><office:text>{body}</office:text></office:body></office:document-content>"#
    );
    let pg = &doc.page;
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {NS}><office:automatic-styles><style:page-layout style:name="PL"><style:page-layout-properties fo:page-width="{}" fo:page-height="{}" fo:margin-top="{}" fo:margin-bottom="{}" fo:margin-left="{}" fo:margin-right="{}" style:print-orientation="{}"/></style:page-layout></office:automatic-styles><office:master-styles><style:master-page style:name="Standard" style:page-layout-name="PL"/></office:master-styles></office:document-styles>"#,
        pt(pg.width),
        pt(pg.height),
        pt(pg.margin_top),
        pt(pg.margin_bottom),
        pt(pg.margin_left),
        pt(pg.margin_right),
        if pg.width > pg.height { "landscape" } else { "portrait" }
    );
    let m = &doc.meta;
    let meta = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-meta {NS}><office:meta><meta:generator>OpenDTP {}</meta:generator><dc:title>{}</dc:title><dc:creator>{}</dc:creator><dc:subject>{}</dc:subject><meta:keyword>{}</meta:keyword></office:meta></office:document-meta>"#,
        crate::version(),
        attr(&m.title),
        attr(&m.author),
        attr(&m.subject),
        attr(&m.keywords)
    );
    let manifest = r#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.text"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/></manifest:manifest>"#;

    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        // `mimetype` musi być pierwszy i nieskompresowany – po nim programy rozpoznają format.
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let files: [(&str, &[u8], zip::write::SimpleFileOptions); 5] = [
            ("mimetype", b"application/vnd.oasis.opendocument.text", stored),
            ("META-INF/manifest.xml", manifest.as_bytes(), deflate),
            ("content.xml", content.as_bytes(), deflate),
            ("styles.xml", styles.as_bytes(), deflate),
            ("meta.xml", meta.as_bytes(), deflate),
        ];
        for (name, data, opt) in files {
            z.start_file(name, opt).map_err(|e| e.to_string())?;
            z.write_all(data).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_through_import() {
        let mut d = Document::new();
        d.meta.title = "Raport & plan".into();
        d.body = Story::plain("Wstęp\nAla  ma kota", "normal");
        d.body.paras[0].style = "heading1".into();
        d.body.paras[1].runs[0].fmt.italic = Some(true);
        let bytes = export(&d).unwrap();
        let back = crate::io::odt_in::import(&bytes).unwrap();
        assert_eq!(back.meta.title, "Raport & plan");
        assert_eq!(back.body.paras[0].text(), "Wstęp");
        assert!(back.body.paras[0].style.starts_with("heading"));
        assert_eq!(back.body.paras[1].text(), "Ala  ma kota");
        assert_eq!(back.body.paras[1].runs[0].fmt.italic, Some(true));
    }
}
