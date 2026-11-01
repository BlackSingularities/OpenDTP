//! Import ODT (LibreOffice, OpenOffice, Dokumenty Google): akapity i nagłówki z poziomami,
//! style automatyczne (pogrubienie, kursywa, podkreślenie, przekreślenie, krój, rozmiar, kolor,
//! wyrównanie, wcięcia, odstępy), listy, łącza, tabulatory i złamania wiersza, tytuł i autor
//! z `meta.xml`. Tabele – wiersze z tabulatorami; obrazy i ramki pomijamy.

use std::collections::HashMap;
use std::io::Read;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::model::*;

fn attr(e: &BytesStart, name: &[u8]) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == name).and_then(|a| a.unescape_value().ok().map(|v| v.to_string()))
}

/// Długość ODF („12pt”, „0.5cm”, „4mm”, „0.25in”) w punktach.
fn length(s: &str) -> Option<f32> {
    let s = s.trim();
    let (num, unit) = s.split_at(s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len()));
    let v: f32 = num.parse().ok()?;
    Some(match unit {
        "cm" => v * 72.0 / 2.54,
        "mm" => v * 72.0 / 25.4,
        "in" => v * 72.0,
        "pc" => v * 12.0,
        _ => v,
    })
}

#[derive(Clone, Default)]
struct OdStyle {
    parent: Option<String>,
    chars: CharFmt,
    para: ParaFmt,
}

/// Style automatyczne i nazwane (`styles.xml` i `content.xml`) – formatowanie i dziedziczenie.
fn read_styles(xml: &str, out: &mut HashMap<String, OdStyle>) {
    let mut r = Reader::from_str(xml);
    let mut cur: Option<String> = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"style:style" => {
                    let name = attr(&e, b"style:name").unwrap_or_default();
                    let parent = attr(&e, b"style:parent-style-name");
                    out.insert(name.clone(), OdStyle { parent, ..Default::default() });
                    cur = Some(name);
                }
                b"style:text-properties" => {
                    let Some(s) = cur.as_ref().and_then(|n| out.get_mut(n)) else { continue };
                    let c = &mut s.chars;
                    if let Some(w) = attr(&e, b"fo:font-weight") {
                        c.bold = Some(w == "bold" || w.parse::<u32>().is_ok_and(|n| n >= 600));
                    }
                    if let Some(v) = attr(&e, b"fo:font-style") {
                        c.italic = Some(v == "italic" || v == "oblique");
                    }
                    if let Some(v) = attr(&e, b"style:text-underline-style") {
                        c.underline = Some(if v == "none" { Underline::None } else { Underline::Single });
                    }
                    if let Some(v) = attr(&e, b"style:text-line-through-style") {
                        c.strike = Some(v != "none");
                    }
                    if let Some(v) = attr(&e, b"fo:font-size").and_then(|v| length(&v)) {
                        c.size = Some(v);
                    }
                    if let Some(v) = attr(&e, b"style:font-name").or_else(|| attr(&e, b"fo:font-family")) {
                        c.font = Some(v.trim_matches('\'').to_string());
                    }
                    if let Some(v) = attr(&e, b"fo:color").and_then(|v| Color::parse(&v)) {
                        c.color = Some(v);
                    }
                    if let Some(v) = attr(&e, b"fo:background-color").filter(|v| v != "transparent").and_then(|v| Color::parse(&v)) {
                        c.highlight = Some(v);
                    }
                    if let Some(v) = attr(&e, b"style:text-position") {
                        if v.starts_with("super") || v.starts_with("33%") {
                            c.script = Some(Script::Super);
                        } else if v.starts_with("sub") || v.starts_with('-') {
                            c.script = Some(Script::Sub);
                        }
                    }
                    if let Some(v) = attr(&e, b"fo:font-variant") {
                        c.caps = Some(if v == "small-caps" { Caps::Small } else { Caps::None });
                    }
                    if attr(&e, b"fo:text-transform").as_deref() == Some("uppercase") {
                        c.caps = Some(Caps::All);
                    }
                }
                b"style:paragraph-properties" => {
                    let Some(s) = cur.as_ref().and_then(|n| out.get_mut(n)) else { continue };
                    let p = &mut s.para;
                    if let Some(v) = attr(&e, b"fo:text-align") {
                        p.align = Some(match v.as_str() {
                            "center" => Align::Center,
                            "end" | "right" => Align::Right,
                            "justify" => Align::Justify,
                            _ => Align::Left,
                        });
                    }
                    p.indent_left = attr(&e, b"fo:margin-left").and_then(|v| length(&v)).or(p.indent_left);
                    p.indent_right = attr(&e, b"fo:margin-right").and_then(|v| length(&v)).or(p.indent_right);
                    p.first_line = attr(&e, b"fo:text-indent").and_then(|v| length(&v)).or(p.first_line);
                    p.space_before = attr(&e, b"fo:margin-top").and_then(|v| length(&v)).or(p.space_before);
                    p.space_after = attr(&e, b"fo:margin-bottom").and_then(|v| length(&v)).or(p.space_after);
                    if attr(&e, b"fo:break-before").as_deref() == Some("page") {
                        p.break_before = Some(Break::Page);
                    }
                    if let Some(v) = attr(&e, b"fo:background-color").filter(|v| v != "transparent").and_then(|v| Color::parse(&v)) {
                        p.shading = Some(v);
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) if e.name().as_ref() == b"style:style" => cur = None,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

/// Formatowanie stylu razem z rodzicami (rodzic najpierw, potem nadpisania).
fn resolved(styles: &HashMap<String, OdStyle>, name: &str) -> (CharFmt, ParaFmt, Option<u8>) {
    let mut chain = Vec::new();
    let mut cur = Some(name.to_string());
    let mut heading = None;
    while let Some(n) = cur {
        if chain.len() > 16 {
            break;
        }
        if heading.is_none() {
            heading = heading_level(&n);
        }
        let s = styles.get(&n);
        chain.push(n);
        cur = s.and_then(|s| s.parent.clone());
    }
    let (mut c, mut p) = (CharFmt::default(), ParaFmt::default());
    for n in chain.iter().rev() {
        if let Some(s) = styles.get(n) {
            c.merge(&s.chars);
            p.merge(&s.para);
        }
    }
    (c, p, heading)
}

/// „Heading_20_1”, „Heading 2”, „Nagłówek_20_3” → poziom.
fn heading_level(name: &str) -> Option<u8> {
    let n = name.replace("_20_", " ").to_lowercase();
    let rest = n.strip_prefix("heading ").or_else(|| n.strip_prefix("nagłówek "))?;
    rest.trim().parse::<u8>().ok().filter(|l| (1..=9).contains(l))
}

#[derive(Default)]
struct Body {
    paras: Vec<Paragraph>,
    cur: Option<Paragraph>,
    /// Formatowanie zagnieżdżonych `text:span` / `text:a`.
    spans: Vec<CharFmt>,
    lists: Vec<ListKind>,
    /// Wewnątrz komórki tabeli: akapity komórki łączymy tabulatorami w jeden wiersz.
    cell: usize,
    row: Option<Paragraph>,
    skip: usize,
}

impl Body {
    fn push_text(&mut self, s: &str) {
        let fmt = self.spans.last().cloned().unwrap_or_default();
        let Some(p) = self.cur.as_mut() else { return };
        match p.runs.last_mut() {
            Some(r) if r.fmt == fmt && r.field.is_none() => r.text.push_str(s),
            _ => p.runs.push(Run { text: s.to_string(), fmt, field: None }),
        }
    }

    fn finish(&mut self) {
        let Some(mut p) = self.cur.take() else { return };
        if p.runs.is_empty() {
            p.runs.push(Run::default());
        }
        p.normalize();
        if self.cell > 0 {
            let row = self.row.get_or_insert_with(|| Paragraph { style: "normal".into(), fmt: ParaFmt::default(), runs: Vec::new() });
            if !row.runs.is_empty() && !row.text().ends_with('\t') {
                row.runs.push(Run { text: " ".into(), ..Default::default() });
            }
            row.runs.extend(p.runs.into_iter().filter(|r| !r.text.is_empty()));
        } else {
            self.paras.push(p);
        }
    }
}

pub fn import(bytes: &[u8]) -> Result<Document, String> {
    let mut za = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| crate::lang::tr("To nie jest plik ODT").to_string())?;
    let mut read = |name: &str| -> Option<String> {
        let mut f = za.by_name(name).ok()?;
        let mut s = String::new();
        f.read_to_string(&mut s).ok()?;
        Some(s)
    };
    let content = read("content.xml").ok_or_else(|| crate::lang::tr("Plik nie zawiera dokumentu").to_string())?;
    let styles_xml = read("styles.xml").unwrap_or_default();
    let meta_xml = read("meta.xml").unwrap_or_default();

    let mut styles = HashMap::new();
    read_styles(&styles_xml, &mut styles);
    read_styles(&content, &mut styles);

    let mut b = Body::default();
    let mut r = Reader::from_str(&content);
    let mut in_body = false;
    loop {
        let ev = r.read_event();
        match ev {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let empty = matches!(ev, Ok(Event::Empty(_)));
                let name = e.name();
                match name.as_ref() {
                    b"office:text" => in_body = true,
                    _ if !in_body => {}
                    b"text:p" | b"text:h" if b.skip == 0 => {
                        b.finish();
                        let sname = attr(e, b"text:style-name").unwrap_or_default();
                        let (chars, mut para, heading) = resolved(&styles, &sname);
                        let level = if name.as_ref() == b"text:h" { attr(e, b"text:outline-level").and_then(|v| v.parse::<u8>().ok()).or(heading).or(Some(1)) } else { heading };
                        let style = match level {
                            Some(l) => format!("heading{}", l.clamp(1, 3)),
                            None => "normal".into(),
                        };
                        if let Some(&kind) = b.lists.last() {
                            para.list = Some(Some(ListFmt { kind, level: (b.lists.len() as u8).saturating_sub(1), start: None }));
                        }
                        b.cur = Some(Paragraph { style, fmt: para, runs: Vec::new() });
                        b.spans = vec![chars];
                        if empty {
                            b.finish();
                        }
                    }
                    b"text:span" | b"text:a" if b.cur.is_some() => {
                        let mut f = b.spans.last().cloned().unwrap_or_default();
                        if let Some(s) = attr(e, b"text:style-name") {
                            f.merge(&resolved(&styles, &s).0);
                        }
                        if name.as_ref() == b"text:a" {
                            f.link = attr(e, b"xlink:href");
                        }
                        if !empty {
                            b.spans.push(f);
                        }
                    }
                    b"text:s" => {
                        let n = attr(e, b"text:c").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).min(200);
                        b.push_text(&" ".repeat(n));
                    }
                    b"text:tab" => b.push_text("\t"),
                    b"text:line-break" => b.push_text("\u{2028}"),
                    b"text:list" if !empty => {
                        // Numerowana, gdy styl listy ma `text:list-level-style-number` – w praktyce
                        // rozpoznajemy po nazwie stylu, a domyślnie wypunktowanie.
                        let numbered = attr(e, b"text:style-name").is_some_and(|s| s.to_lowercase().contains("num"));
                        b.lists.push(if numbered { ListKind::Number } else { ListKind::Bullet });
                    }
                    b"table:table-cell" if !empty => {
                        if b.cell > 0 {
                            if let Some(row) = b.row.as_mut() {
                                row.runs.push(Run { text: "\t".into(), ..Default::default() });
                            }
                        }
                        b.cell += 1;
                    }
                    b"text:note" | b"office:annotation" | b"draw:frame" | b"text:tracked-changes" | b"text:sequence-decls" if !empty => b.skip += 1,
                    _ => {}
                }
            }
            Ok(Event::Text(t)) if in_body && b.skip == 0 => {
                let s = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                if !s.is_empty() {
                    b.push_text(&s.replace(['\n', '\r'], " "));
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"text:p" | b"text:h" if b.skip == 0 => b.finish(),
                b"text:span" | b"text:a" => {
                    if b.spans.len() > 1 {
                        b.spans.pop();
                    }
                }
                b"text:list" => {
                    b.lists.pop();
                }
                b"table:table-row" => {
                    b.finish();
                    b.cell = 0;
                    if let Some(mut row) = b.row.take() {
                        if row.runs.is_empty() {
                            row.runs.push(Run::default());
                        }
                        b.paras.push(row);
                    }
                }
                b"text:note" | b"office:annotation" | b"draw:frame" | b"text:tracked-changes" | b"text:sequence-decls" => b.skip = b.skip.saturating_sub(1),
                b"office:text" => break,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    b.finish();

    let mut doc = Document::new();
    if !b.paras.is_empty() {
        doc.body = Story { paras: b.paras };
    }
    let tag = |name: &str| -> String {
        let open = format!("<{name}>");
        let close = format!("</{name}>");
        meta_xml.find(&open).and_then(|a| meta_xml[a + open.len()..].find(&close).map(|b| meta_xml[a + open.len()..a + open.len() + b].to_string())).map(|s| unescape_xml(&s)).unwrap_or_default()
    };
    doc.meta.title = tag("dc:title");
    doc.meta.author = tag("dc:creator");
    if doc.meta.author.is_empty() {
        doc.meta.author = tag("meta:initial-creator");
    }
    doc.meta.subject = tag("dc:subject");
    Ok(doc)
}

fn unescape_xml(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn odt(content: &str) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("content.xml", o).unwrap();
            z.write_all(content.as_bytes()).unwrap();
            z.start_file("meta.xml", o).unwrap();
            z.write_all(b"<office:meta><dc:title>Raport &amp; plan</dc:title></office:meta>").unwrap();
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn paragraphs_headings_and_styles() {
        let xml = r#"<office:document-content><office:automatic-styles>
<style:style style:name="T1" style:family="text"><style:text-properties fo:font-weight="bold"/></style:style>
<style:style style:name="P1" style:family="paragraph"><style:paragraph-properties fo:text-align="center"/></style:style>
</office:automatic-styles><office:body><office:text>
<text:h text:outline-level="2">Wstęp</text:h>
<text:p text:style-name="P1">Ala <text:span text:style-name="T1">ma</text:span><text:s text:c="2"/>kota</text:p>
<text:list><text:list-item><text:p>punkt</text:p></text:list-item></text:list>
</office:text></office:body></office:document-content>"#;
        let d = import(&odt(xml)).unwrap();
        assert_eq!(d.meta.title, "Raport & plan");
        assert_eq!(d.body.paras[0].style, "heading2");
        assert_eq!(d.body.paras[0].text(), "Wstęp");
        let p = &d.body.paras[1];
        assert_eq!(p.text(), "Ala ma  kota");
        assert_eq!(p.fmt.align, Some(Align::Center));
        assert!(p.runs.iter().any(|r| r.text == "ma" && r.fmt.bold == Some(true)));
        assert!(d.body.paras[2].fmt.list.is_some());
    }
}
