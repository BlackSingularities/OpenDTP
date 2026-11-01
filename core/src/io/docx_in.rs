//! Import DOCX: akapity, style (nagłówki, tytuł, cytat…), formatowanie znaków i akapitów, listy,
//! tabulatory, złamania, odnośniki, nagłówek i stopka, ustawienia strony i kolumny. Obrazy
//! w tekście stają się obiektami „nad i pod” w miejscu akapitu. Tabele – wiersze z tabulatorami.

use std::collections::HashMap;
use std::io::Read;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::model::*;

fn attr(e: &BytesStart, name: &[u8]) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == name).and_then(|a| a.unescape_value().ok().map(|v| v.to_string()))
}

fn val(e: &BytesStart) -> Option<String> {
    attr(e, b"w:val")
}

fn on(e: &BytesStart) -> bool {
    !matches!(val(e).as_deref(), Some("0") | Some("false") | Some("off") | Some("none"))
}

fn twips(s: Option<String>) -> Option<f32> {
    s.and_then(|v| v.parse::<f32>().ok()).map(|v| v / 20.0)
}

fn our_style(id: &str) -> String {
    match id.to_lowercase().as_str() {
        "normal" => "normal",
        "title" | "tytu" => "title",
        "subtitle" | "podtytu" => "subtitle",
        "heading1" | "nagwek1" => "heading1",
        "heading2" | "nagwek2" => "heading2",
        "heading3" | "heading4" | "heading5" | "heading6" | "nagwek3" => "heading3",
        "quote" | "intensequote" | "cytat" => "quote",
        "caption" | "legenda" => "caption",
        "listparagraph" | "akapitzlist" => "list",
        "toc1" | "spistreci1" => "toc1",
        "toc2" | "spistreci2" => "toc2",
        "toc3" | "spistreci3" => "toc3",
        _ => "normal",
    }
    .to_string()
}

struct Zip {
    files: HashMap<String, Vec<u8>>,
}

impl Zip {
    fn text(&self, name: &str) -> Option<String> {
        self.files.get(name).map(|d| String::from_utf8_lossy(d).to_string())
    }
}

fn rels(z: &Zip, part: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(xml) = z.text(part) else { return out };
    let mut r = Reader::from_str(&xml);
    loop {
        match r.read_event() {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.local_name().as_ref() == b"Relationship" => {
                if let (Some(id), Some(t)) = (attr(&e, b"Id"), attr(&e, b"Target")) {
                    out.insert(id, t);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// numId → (rodzaj listy dla poziomów 0..8).
fn numbering(z: &Zip) -> HashMap<String, Vec<ListKind>> {
    let mut abs: HashMap<String, Vec<ListKind>> = HashMap::new();
    let mut nums: HashMap<String, String> = HashMap::new();
    let Some(xml) = z.text("word/numbering.xml") else { return HashMap::new() };
    let mut r = Reader::from_str(&xml);
    let mut cur_abs: Option<String> = None;
    let mut cur_num: Option<String> = None;
    let mut lvl_text = String::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"w:abstractNum" => cur_abs = attr(&e, b"w:abstractNumId"),
                b"w:num" => cur_num = attr(&e, b"w:numId"),
                b"w:abstractNumId" => {
                    if let (Some(n), Some(v)) = (&cur_num, val(&e)) {
                        nums.insert(n.clone(), v);
                    }
                }
                b"w:lvlText" => lvl_text = val(&e).unwrap_or_default(),
                b"w:numFmt" => {
                    if let Some(a) = &cur_abs {
                        let k = match val(&e).as_deref() {
                            Some("bullet") => {
                                if lvl_text.contains('–') || lvl_text.contains('-') {
                                    ListKind::Dash
                                } else {
                                    ListKind::Bullet
                                }
                            }
                            Some("lowerLetter") => ListKind::LowerLetter,
                            Some("upperLetter") => ListKind::UpperLetter,
                            Some("lowerRoman") => ListKind::LowerRoman,
                            Some("upperRoman") => ListKind::UpperRoman,
                            _ => ListKind::Number,
                        };
                        abs.entry(a.clone()).or_default().push(k);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    nums.into_iter().filter_map(|(n, a)| abs.get(&a).map(|v| (n, v.clone()))).collect()
}

#[derive(Default)]
struct State {
    paras: Vec<Paragraph>,
    para: Option<Paragraph>,
    run_fmt: CharFmt,
    in_ppr: bool,
    in_rpr: bool,
    in_run: bool,
    in_text: bool,
    link: Option<String>,
    num: Option<(String, u8)>,
    pending_page_break: bool,
    images: Vec<(usize, String, f32, f32)>,
    in_table: usize,
    row: Option<Paragraph>,
    cell_first: bool,
    field_instr: String,
}

fn parse_story(xml: &str, rels: &HashMap<String, String>, nums: &HashMap<String, Vec<ListKind>>, page: Option<&mut PageSetup>) -> (Vec<Paragraph>, Vec<(usize, String, f32, f32)>) {
    let mut r = Reader::from_str(xml);
    let mut st = State::default();
    let mut page = page;
    let mut cx = 0.0f32;
    let mut cy = 0.0f32;
    loop {
        let ev = r.read_event();
        match ev {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let empty = matches!(ev, Ok(Event::Empty(_)));
                let name = e.name();
                match name.as_ref() {
                    b"w:tbl" if !empty => st.in_table += 1,
                    b"w:tr" if !empty => {
                        st.row = Some(Paragraph::default());
                        st.cell_first = true;
                    }
                    b"w:tc" if !empty => {
                        if let Some(row) = st.row.as_mut() {
                            if !st.cell_first {
                                row.runs.push(Run { text: "\t".into(), ..Default::default() });
                            }
                        }
                        st.cell_first = false;
                    }
                    b"w:p" if !empty => {
                        let mut p = Paragraph { runs: Vec::new(), ..Default::default() };
                        if st.pending_page_break {
                            p.fmt.break_before = Some(Break::Page);
                            st.pending_page_break = false;
                        }
                        st.para = Some(p);
                        st.num = None;
                    }
                    b"w:pPr" if !empty => st.in_ppr = true,
                    b"w:rPr" if !empty => st.in_rpr = true,
                    b"w:r" if !empty => {
                        st.in_run = true;
                        st.run_fmt = CharFmt::default();
                        if let Some(l) = &st.link {
                            st.run_fmt.link = Some(l.clone());
                        }
                    }
                    b"w:hyperlink" if !empty => st.link = attr(e, b"r:id").and_then(|id| rels.get(&id).cloned()),
                    b"w:t" if !empty => st.in_text = true,
                    b"w:instrText" if !empty => st.in_text = false,
                    b"w:fldSimple" => st.field_instr = attr(e, b"w:instr").unwrap_or_default(),
                    b"w:tab" if st.in_run => push_text(&mut st, "\t"),
                    b"w:br" if st.in_run => match attr(e, b"w:type").as_deref() {
                        Some("page") => st.pending_page_break = true,
                        Some("column") => {}
                        _ => push_text(&mut st, "\n"),
                    },
                    b"w:pStyle" if st.in_ppr => {
                        if let (Some(p), Some(v)) = (st.para.as_mut(), val(e)) {
                            p.style = our_style(&v);
                        }
                    }
                    b"w:jc" if st.in_ppr => {
                        if let Some(p) = st.para.as_mut() {
                            p.fmt.align = Some(match val(e).as_deref() {
                                Some("center") => Align::Center,
                                Some("right") | Some("end") => Align::Right,
                                Some("both") | Some("distribute") => Align::Justify,
                                _ => Align::Left,
                            });
                        }
                    }
                    b"w:ind" if st.in_ppr => {
                        if let Some(p) = st.para.as_mut() {
                            p.fmt.indent_left = twips(attr(e, b"w:left").or_else(|| attr(e, b"w:start")));
                            p.fmt.indent_right = twips(attr(e, b"w:right").or_else(|| attr(e, b"w:end")));
                            if let Some(f) = twips(attr(e, b"w:firstLine")) {
                                p.fmt.first_line = Some(f);
                            }
                            if let Some(h) = twips(attr(e, b"w:hanging")) {
                                p.fmt.first_line = Some(-h);
                            }
                        }
                    }
                    b"w:spacing" if st.in_ppr => {
                        if let Some(p) = st.para.as_mut() {
                            p.fmt.space_before = twips(attr(e, b"w:before"));
                            p.fmt.space_after = twips(attr(e, b"w:after"));
                            if let Some(l) = attr(e, b"w:line").and_then(|v| v.parse::<f32>().ok()) {
                                p.fmt.line = Some(match attr(e, b"w:lineRule").as_deref() {
                                    Some("exact") => LineSpacing::Exact(l / 20.0),
                                    Some("atLeast") => LineSpacing::AtLeast(l / 20.0),
                                    _ => LineSpacing::Multiple(l / 240.0),
                                });
                            }
                        }
                    }
                    b"w:keepNext" if st.in_ppr => {
                        if let Some(p) = st.para.as_mut() {
                            p.fmt.keep_next = Some(on(e));
                        }
                    }
                    b"w:pageBreakBefore" if st.in_ppr => {
                        if let Some(p) = st.para.as_mut() {
                            if on(e) {
                                p.fmt.break_before = Some(Break::Page);
                            }
                        }
                    }
                    b"w:shd" if st.in_ppr && !st.in_rpr => {
                        if let (Some(p), Some(c)) = (st.para.as_mut(), attr(e, b"w:fill").and_then(|f| Color::parse(&f))) {
                            p.fmt.shading = Some(c);
                        }
                    }
                    b"w:ilvl" if st.in_ppr => {
                        let lvl = val(e).and_then(|v| v.parse().ok()).unwrap_or(0);
                        st.num = Some((st.num.take().map(|n| n.0).unwrap_or_default(), lvl));
                    }
                    b"w:numId" if st.in_ppr => {
                        let lvl = st.num.as_ref().map(|n| n.1).unwrap_or(0);
                        st.num = Some((val(e).unwrap_or_default(), lvl));
                    }
                    b"w:tab" if st.in_ppr => {
                        if let Some(p) = st.para.as_mut() {
                            let pos = twips(attr(e, b"w:pos")).unwrap_or(0.0);
                            let right = attr(e, b"w:val").as_deref() == Some("right") && attr(e, b"w:leader").is_some();
                            p.fmt.tabs.get_or_insert_with(Vec::new).push(if right { -1.0 } else { pos });
                        }
                    }
                    b"w:rFonts" if st.in_rpr => st.run_fmt.font = attr(e, b"w:ascii").or_else(|| attr(e, b"w:hAnsi")),
                    b"w:b" if st.in_rpr => st.run_fmt.bold = Some(on(e)),
                    b"w:i" if st.in_rpr => st.run_fmt.italic = Some(on(e)),
                    b"w:strike" | b"w:dstrike" if st.in_rpr => st.run_fmt.strike = Some(on(e)),
                    b"w:caps" if st.in_rpr && on(e) => st.run_fmt.caps = Some(Caps::All),
                    b"w:smallCaps" if st.in_rpr && on(e) => st.run_fmt.caps = Some(Caps::Small),
                    b"w:u" if st.in_rpr => {
                        st.run_fmt.underline = Some(match val(e).as_deref() {
                            Some("none") => Underline::None,
                            Some("double") => Underline::Double,
                            Some("dotted") | Some("dottedHeavy") => Underline::Dotted,
                            Some("wave") | Some("wavyHeavy") | Some("wavyDouble") => Underline::Wave,
                            _ => Underline::Single,
                        })
                    }
                    b"w:sz" if st.in_rpr => st.run_fmt.size = val(e).and_then(|v| v.parse::<f32>().ok()).map(|v| v / 2.0),
                    b"w:color" if st.in_rpr => st.run_fmt.color = val(e).filter(|v| v != "auto").and_then(|v| Color::parse(&v)),
                    b"w:spacing" if st.in_rpr => st.run_fmt.tracking = twips(val(e)),
                    b"w:vertAlign" if st.in_rpr => {
                        st.run_fmt.script = match val(e).as_deref() {
                            Some("superscript") => Some(Script::Super),
                            Some("subscript") => Some(Script::Sub),
                            _ => None,
                        }
                    }
                    b"w:highlight" if st.in_rpr => {
                        st.run_fmt.highlight = match val(e).as_deref() {
                            Some("yellow") => Some(Color::rgb(255, 255, 0)),
                            Some("green") => Some(Color::rgb(0, 255, 0)),
                            Some("cyan") => Some(Color::rgb(0, 255, 255)),
                            Some("magenta") => Some(Color::rgb(255, 0, 255)),
                            Some("red") => Some(Color::rgb(255, 0, 0)),
                            Some("lightGray") => Some(Color::rgb(0xd3, 0xd3, 0xd3)),
                            _ => None,
                        }
                    }
                    b"w:shd" if st.in_rpr => {
                        if let Some(c) = attr(e, b"w:fill").filter(|f| f != "auto").and_then(|f| Color::parse(&f)) {
                            st.run_fmt.highlight = Some(c);
                        }
                    }
                    b"wp:extent" => {
                        cx = attr(e, b"cx").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0) / 12700.0;
                        cy = attr(e, b"cy").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0) / 12700.0;
                    }
                    b"a:blip" => {
                        if let Some(target) = attr(e, b"r:embed").and_then(|id| rels.get(&id).cloned()) {
                            let idx = st.paras.len();
                            st.images.push((idx, target, cx, cy));
                        }
                    }
                    b"w:pgSz" => {
                        if let Some(ps) = page.as_deref_mut() {
                            if let Some(w) = twips(attr(e, b"w:w")) {
                                ps.width = w;
                            }
                            if let Some(h) = twips(attr(e, b"w:h")) {
                                ps.height = h;
                            }
                        }
                    }
                    b"w:pgMar" => {
                        if let Some(ps) = page.as_deref_mut() {
                            if let Some(v) = twips(attr(e, b"w:top")) {
                                ps.margin_top = v.abs();
                            }
                            if let Some(v) = twips(attr(e, b"w:bottom")) {
                                ps.margin_bottom = v.abs();
                            }
                            if let Some(v) = twips(attr(e, b"w:left")) {
                                ps.margin_left = v;
                            }
                            if let Some(v) = twips(attr(e, b"w:right")) {
                                ps.margin_right = v;
                            }
                            if let Some(v) = twips(attr(e, b"w:header")) {
                                ps.header_dist = v;
                            }
                            if let Some(v) = twips(attr(e, b"w:footer")) {
                                ps.footer_dist = v;
                            }
                        }
                    }
                    b"w:cols" => {
                        if let Some(ps) = page.as_deref_mut() {
                            if let Some(n) = attr(e, b"w:num").and_then(|v| v.parse::<u8>().ok()) {
                                ps.columns = n.clamp(1, 12);
                            }
                            if let Some(g) = twips(attr(e, b"w:space")) {
                                ps.column_gap = g;
                            }
                            ps.column_rule = attr(e, b"w:sep").is_some_and(|v| v == "1" || v == "true");
                        }
                    }
                    b"w:titlePg" => {
                        if let Some(ps) = page.as_deref_mut() {
                            ps.different_first = on(e);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) if st.in_text => {
                let s = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                push_text(&mut st, &s);
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"w:t" => st.in_text = false,
                b"w:pPr" => {
                    st.in_ppr = false;
                    if let (Some(p), Some((id, lvl))) = (st.para.as_mut(), st.num.take()) {
                        if id == "0" {
                            p.fmt.list = Some(None);
                        } else if let Some(kinds) = nums.get(&id) {
                            let k = kinds.get(lvl as usize).copied().unwrap_or(ListKind::Bullet);
                            p.fmt.list = Some(Some(ListFmt { kind: k, level: lvl, start: None }));
                            // Wcięcie listy liczy nasz skład.
                            p.fmt.indent_left = None;
                            p.fmt.first_line = None;
                        }
                    }
                }
                b"w:rPr" => st.in_rpr = false,
                b"w:r" => st.in_run = false,
                b"w:hyperlink" => st.link = None,
                b"w:fldSimple" => st.field_instr.clear(),
                b"w:p" => {
                    if let Some(mut p) = st.para.take() {
                        if p.runs.is_empty() {
                            p.runs.push(Run::default());
                        }
                        p.normalize();
                        if st.in_table > 0 {
                            if let Some(row) = st.row.as_mut() {
                                if !row.runs.iter().all(|r| r.text.is_empty()) && !row.runs.last().is_some_and(|r| r.text.ends_with('\t')) {
                                    row.runs.push(Run { text: " ".into(), ..Default::default() });
                                }
                                row.runs.extend(p.runs.into_iter().filter(|r| !r.text.is_empty()));
                            }
                        } else {
                            st.paras.push(p);
                        }
                    }
                }
                b"w:tr" => {
                    if let Some(mut row) = st.row.take() {
                        row.normalize();
                        st.paras.push(row);
                    }
                }
                b"w:tbl" => st.in_table = st.in_table.saturating_sub(1),
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    (st.paras, st.images)
}

fn push_text(st: &mut State, s: &str) {
    let Some(p) = st.para.as_mut() else { return };
    let field = match st.field_instr.split_whitespace().next() {
        Some("PAGE") => Some(Field::PageNumber),
        Some("NUMPAGES") => Some(Field::PageCount),
        Some("DATE") => Some(Field::Date),
        Some("TITLE") => Some(Field::Title),
        Some("AUTHOR") => Some(Field::Author),
        _ => None,
    };
    if let Some(f) = field {
        if !p.runs.iter().any(|r| r.field == Some(f)) {
            p.runs.push(Run { text: FIELD_CHAR.to_string(), fmt: st.run_fmt.clone(), field: Some(f) });
        }
        return;
    }
    if st.run_fmt.link.is_some() && st.run_fmt.color.is_none() {
        st.run_fmt.color = Some(Color::rgb(0x05, 0x63, 0xc1));
        st.run_fmt.underline = Some(Underline::Single);
    }
    p.runs.push(Run { text: s.to_string(), fmt: st.run_fmt.clone(), field: None });
}

pub fn import(bytes: &[u8]) -> Result<Document, String> {
    let mut za = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| crate::lang::tr("To nie jest plik DOCX").to_string())?;
    let mut files = HashMap::new();
    for i in 0..za.len() {
        let mut f = za.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().to_string();
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|e| e.to_string())?;
        files.insert(name, data);
    }
    let z = Zip { files };
    let xml = z.text("word/document.xml").ok_or_else(|| crate::lang::tr("Plik nie zawiera dokumentu").to_string())?;
    let rel = rels(&z, "word/_rels/document.xml.rels");
    let nums = numbering(&z);
    let mut doc = Document::new();
    let (paras, images) = parse_story(&xml, &rel, &nums, Some(&mut doc.page));
    if !paras.is_empty() {
        doc.body = Story { paras };
    }
    // Nagłówek i stopka (pierwsze znalezione).
    for (id, target) in &rel {
        let _ = id;
        let part = format!("word/{target}");
        if target.starts_with("header") && doc.header.is_none() {
            if let Some(x) = z.text(&part) {
                let hr = rels(&z, &format!("word/_rels/{target}.rels"));
                let (p, _) = parse_story(&x, &hr, &nums, None);
                if !p.is_empty() {
                    doc.header = Some(Story { paras: p });
                }
            }
        } else if target.starts_with("footer") && doc.footer.is_none() {
            if let Some(x) = z.text(&part) {
                let hr = rels(&z, &format!("word/_rels/{target}.rels"));
                let (p, _) = parse_story(&x, &hr, &nums, None);
                if !p.is_empty() {
                    doc.footer = Some(Story { paras: p });
                }
            }
        }
    }
    // Metadane.
    if let Some(core) = z.text("docProps/core.xml") {
        let grab = |tag: &str| -> String {
            let open = format!("<{tag}>");
            let close = format!("</{tag}>");
            core.find(&open).and_then(|a| core[a + open.len()..].find(&close).map(|b| core[a + open.len()..a + open.len() + b].to_string())).unwrap_or_default()
        };
        doc.meta.title = grab("dc:title");
        doc.meta.author = grab("dc:creator");
        doc.meta.subject = grab("dc:subject");
    }
    // Obrazy: kolejno, po składzie – w miejscu akapitu, „nad i pod”.
    let mut eng = crate::layout::Engine::new();
    for (pi, target, w, h) in images.into_iter().take(60) {
        let path = format!("word/{}", target.trim_start_matches('/').trim_start_matches("word/"));
        let Some(data) = z.files.get(&path).cloned() else { continue };
        let Some(asset) = super::odtp::asset_from(data) else { continue };
        let id = format!("img{}", doc.new_id());
        let lay = eng.layout(&doc);
        let pi = pi.min(doc.body.paras.len().saturating_sub(1));
        let (page, top) = lay.story_lines(StoryRef::Body).iter().map(|&i| &lay.lines[i]).find(|l| l.para >= pi).map(|l| (l.page, l.top)).unwrap_or((0, doc.page.margin_top));
        let cw = doc.page.content_width();
        let (mut w, mut h) = if w > 1.0 && h > 1.0 { (w, h) } else { (asset.width as f32 * 0.75, asset.height as f32 * 0.75) };
        if w > cw {
            h *= cw / w;
            w = cw;
        }
        doc.assets.insert(id.clone(), asset);
        let oid = doc.new_id();
        let x = doc.page.margin_left + (cw - w) / 2.0;
        doc.objects.push(Object {
            id: oid,
            name: String::new(),
            kind: ObjKind::Image { asset: id, fit: ImageFit::Stretch, contour: None },
            page,
            x,
            y: top,
            w,
            h,
            shape: Shape::rect(),
            wrap: Wrap { mode: WrapMode::TopBottom, dist: 6.0, contour: false },
            fill: None,
            stroke: None,
            opacity: 1.0,
            corner: 0.0,
            shadow: false,
            locked: false,
            anchor_dy: 4.0,
            cont_of: None,
        });
    }
    Ok(doc)
}
