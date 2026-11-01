//! Eksport do EPUB 3 (czytniki e-booków): rozdziały dzielone na nagłówkach pierwszego poziomu,
//! spis treści z nagłówków 1–2, obrazy zakotwiczone w tekście, style podstawowe w CSS.

use std::fmt::Write as _;
use std::io::Write;

use crate::model::*;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Granice rozdziałów: akapit tytułu albo nagłówka 1. poziomu zaczyna nowy plik.
fn chapters(doc: &Document) -> Vec<std::ops::Range<usize>> {
    let n = doc.body.paras.len();
    let mut starts = vec![0];
    for (i, p) in doc.body.paras.iter().enumerate().skip(1) {
        if p.style.starts_with("toc") {
            continue;
        }
        if doc.outline_level(p) == Some(1) {
            starts.push(i);
        }
    }
    starts.dedup();
    let mut out = Vec::new();
    for (k, &s) in starts.iter().enumerate() {
        let e = starts.get(k + 1).copied().unwrap_or(n);
        if e > s {
            out.push(s..e);
        }
    }
    if out.is_empty() {
        out.push(0..n);
    }
    out
}

fn css(doc: &Document) -> String {
    let base = doc.resolve_para(&Paragraph { style: "normal".into(), fmt: ParaFmt::default(), runs: vec![Run::default()] }).1;
    let mut s = String::new();
    let _ = write!(s, "body{{font-family:'{}',serif;line-height:1.45;margin:0 4%}}\n", base.font);
    s.push_str("p{margin:0 0 .6em;text-indent:0}h1,h2,h3{line-height:1.2;margin:1.2em 0 .5em;page-break-after:avoid}h1{font-size:1.7em}h2{font-size:1.35em}h3{font-size:1.15em}\n");
    s.push_str("blockquote{margin:1em 1.5em;font-style:italic}p.img{text-align:center;text-indent:0}img{max-width:100%;height:auto}nav ol{list-style:none;padding-left:1em}\n");
    s
}

pub fn export(doc: &Document) -> Result<Vec<u8>, String> {
    let title = if doc.meta.title.trim().is_empty() { crate::lang::tr("Dokument").to_string() } else { doc.meta.title.clone() };
    let lang = if doc.settings.lang.is_empty() { "pl".to_string() } else { doc.settings.lang.clone() };
    let now = crate::util::iso_now();
    let modified = format!("{}Z", now.get(..19).unwrap_or("2026-01-01T00:00:00"));
    // Identyfikator z czasu – stały dla jednego eksportu.
    let uid = format!("urn:opendtp:{}", now.replace([':', '-', '.', 'T', 'Z', '+'], ""));
    let parts = chapters(doc);

    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let put = |z: &mut zip::ZipWriter<&mut std::io::Cursor<Vec<u8>>>, name: &str, data: &[u8], opt: zip::write::SimpleFileOptions| -> Result<(), String> {
            z.start_file(name, opt).map_err(|e| e.to_string())?;
            z.write_all(data).map_err(|e| e.to_string())
        };
        // `mimetype` musi być pierwszy i nieskompresowany.
        put(&mut z, "mimetype", b"application/epub+zip", stored)?;
        put(
            &mut z,
            "META-INF/container.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
            deflate,
        )?;
        put(&mut z, "OEBPS/style.css", css(doc).as_bytes(), deflate)?;

        // Rozdziały.
        let mut manifest = String::new();
        let mut spine = String::new();
        let mut nav = String::new();
        for (k, r) in parts.iter().enumerate() {
            let file = format!("ch{:03}.xhtml", k + 1);
            let body = crate::io::plain::html_body(doc, r.clone(), Some("images"));
            let head = doc.body.paras.get(r.start).map(|p| p.text().replace(FIELD_CHAR, "")).unwrap_or_default();
            let ch_title = if head.trim().is_empty() { title.clone() } else { head.trim().to_string() };
            let x = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" lang=\"{lang}\" xml:lang=\"{lang}\"><head><meta charset=\"utf-8\"/><title>{}</title><link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\"/></head><body>\n{body}</body></html>\n",
                esc(&ch_title)
            );
            put(&mut z, &format!("OEBPS/{file}"), x.as_bytes(), deflate)?;
            let _ = write!(manifest, r#"<item id="c{k}" href="{file}" media-type="application/xhtml+xml"/>"#);
            let _ = write!(spine, r#"<itemref idref="c{k}"/>"#);
            // Spis treści: nagłówki 1–2 w tym rozdziale.
            for pi in r.clone() {
                let p = &doc.body.paras[pi];
                let lvl = doc.outline_level(p).or(if p.style == "title" { Some(1) } else { None });
                let Some(lvl) = lvl.filter(|l| *l <= 2) else { continue };
                let t = p.text().replace(FIELD_CHAR, "");
                if t.trim().is_empty() {
                    continue;
                }
                let indent = if lvl == 2 { " style=\"margin-left:1.2em\"" } else { "" };
                let _ = write!(nav, r##"<li{indent}><a href="{file}#h{pi}">{}</a></li>"##, esc(t.trim()));
            }
        }
        if nav.is_empty() {
            let _ = write!(nav, r#"<li><a href="ch001.xhtml">{}</a></li>"#, esc(&title));
        }
        let navx = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" lang=\"{lang}\"><head><meta charset=\"utf-8\"/><title>{}</title><link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\"/></head><body><nav epub:type=\"toc\" id=\"toc\"><h1>{}</h1><ol>{nav}</ol></nav></body></html>\n",
            esc(&title),
            esc(crate::lang::tr("Spis treści"))
        );
        put(&mut z, "OEBPS/nav.xhtml", navx.as_bytes(), deflate)?;

        // Obrazy zakotwiczone w treści.
        let mut done = std::collections::HashSet::new();
        for p in &doc.body.paras {
            for r in &p.runs {
                let Some(Field::Anchor(id)) = r.field else { continue };
                let Some(ObjKind::Image { asset, .. }) = doc.object(id).map(|o| &o.kind) else { continue };
                let Some(a) = doc.assets.get(asset) else { continue };
                if !done.insert(asset.clone()) {
                    continue;
                }
                let ext = crate::io::odtp::ext_of(&a.mime);
                put(&mut z, &format!("OEBPS/images/{asset}.{ext}"), &a.data, stored)?;
                let _ = write!(manifest, r#"<item id="i{}" href="images/{asset}.{ext}" media-type="{}"/>"#, done.len(), a.mime);
            }
        }

        let author = if doc.meta.author.is_empty() { String::new() } else { format!("<dc:creator>{}</dc:creator>", esc(&doc.meta.author)) };
        let opf = format!(
            concat!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
                "<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"uid\" xml:lang=\"{lang}\">",
                "<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"uid\">{uid}</dc:identifier><dc:title>{title}</dc:title>",
                "<dc:language>{lang}</dc:language>{author}<meta property=\"dcterms:modified\">{modified}</meta></metadata>",
                "<manifest><item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>",
                "<item id=\"css\" href=\"style.css\" media-type=\"text/css\"/>{manifest}</manifest>",
                "<spine>{spine}</spine></package>\n"
            ),
            lang = lang,
            uid = uid,
            title = esc(&title),
            author = author,
            modified = modified,
            manifest = manifest,
            spine = spine
        );
        put(&mut z, "OEBPS/content.opf", opf.as_bytes(), deflate)?;
        z.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}
