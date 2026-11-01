//! Eksport do DOCX (Office Open XML). Treść, style (z nazwami wbudowanych stylów Worda, więc
//! działa okienko nawigacji i spis treści Worda), listy, nagłówki i stopki, sekcja z kolumnami,
//! obrazy i kształty zakotwiczone do strony w tym samym miejscu, opływanie (także wielokątem
//! konturu), ramki tekstowe z łańcuchami (`linkedTxbx`) i własną geometrią (`custGeom`).
//!
//! Word nie wpisuje tekstu w dowolny kształt, więc dla kształtów tekstu do wyboru:
//! `exact_shapes` – każdy wiersz jako osobne pole w dokładnym położeniu (wygląd 1:1), albo
//! jedno edytowalne pole tekstowe z kształtem (tekst w prostokącie ramki).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write;

use crate::geom;
use crate::layout::Layout;
use crate::model::*;

pub struct DocxOpts {
    pub exact_shapes: bool,
}

const EMU: f32 = 12700.0;

fn emu(v: f32) -> i64 {
    (v * EMU).round() as i64
}

fn tw(v: f32) -> i64 {
    (v * 20.0).round() as i64
}

pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => {}
            c => o.push(c),
        }
    }
    o
}

fn word_style_id(id: &str) -> String {
    match id {
        "normal" => "Normal".into(),
        "title" => "Title".into(),
        "subtitle" => "Subtitle".into(),
        "heading1" => "Heading1".into(),
        "heading2" => "Heading2".into(),
        "heading3" => "Heading3".into(),
        "quote" => "Quote".into(),
        "caption" => "Caption".into(),
        "toc1" => "TOC1".into(),
        "toc2" => "TOC2".into(),
        "toc3" => "TOC3".into(),
        "header" => "Header".into(),
        "footer" => "Footer".into(),
        "list" => "ListParagraph".into(),
        other => {
            let mut s: String = other.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            if s.is_empty() {
                s = "Custom".into();
            }
            let mut c = s.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or(s)
        }
    }
}

fn word_style_name(s: &Style) -> String {
    match s.id.as_str() {
        "normal" => "Normal".into(),
        "title" => "Title".into(),
        "subtitle" => "Subtitle".into(),
        "heading1" => "heading 1".into(),
        "heading2" => "heading 2".into(),
        "heading3" => "heading 3".into(),
        "quote" => "Quote".into(),
        "caption" => "caption".into(),
        "toc1" => "toc 1".into(),
        "toc2" => "toc 2".into(),
        "toc3" => "toc 3".into(),
        "header" => "header".into(),
        "footer" => "footer".into(),
        "list" => "List Paragraph".into(),
        _ => s.name.clone(),
    }
}

struct Ctx<'a> {
    doc: &'a Document,
    layout: &'a Layout,
    opts: &'a DocxOpts,
    rels: Vec<(String, String, String, bool)>, // id, type, target, external
    media: Vec<(String, Vec<u8>)>,
    asset_rel: HashMap<String, String>,
    next_doc_pr: u32,
    links: HashMap<String, String>,
    txbx_ids: HashMap<u64, u32>,
}

impl Ctx<'_> {
    fn rel(&mut self, kind: &str, target: &str, external: bool) -> String {
        let id = format!("rId{}", self.rels.len() + 10);
        self.rels.push((id.clone(), kind.into(), target.into(), external));
        id
    }

    fn image_rel(&mut self, asset: &str) -> Option<String> {
        if let Some(r) = self.asset_rel.get(asset) {
            return Some(r.clone());
        }
        let a = self.doc.assets.get(asset)?;
        let name = format!("image{}.{}", self.media.len() + 1, super::odtp::ext_of(&a.mime));
        self.media.push((name.clone(), a.data.as_ref().clone()));
        let r = self.rel("http://schemas.openxmlformats.org/officeDocument/2006/relationships/image", &format!("media/{name}"), false);
        self.asset_rel.insert(asset.into(), r.clone());
        Some(r)
    }

    fn png_rel(&mut self, png: Vec<u8>) -> String {
        let name = format!("image{}.png", self.media.len() + 1);
        self.media.push((name.clone(), png));
        self.rel("http://schemas.openxmlformats.org/officeDocument/2006/relationships/image", &format!("media/{name}"), false)
    }

    fn link_rel(&mut self, url: &str) -> String {
        if let Some(r) = self.links.get(url) {
            return r.clone();
        }
        let r = self.rel("http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink", url, true);
        self.links.insert(url.into(), r.clone());
        r
    }
}

fn rpr(f: &CharFmt, out: &mut String) {
    let mut s = String::new();
    if let Some(font) = &f.font {
        let _ = write!(s, r#"<w:rFonts w:ascii="{0}" w:hAnsi="{0}" w:cs="{0}" w:eastAsia="{0}"/>"#, esc(font));
    }
    if let Some(b) = f.bold {
        s.push_str(if b { "<w:b/><w:bCs/>" } else { r#"<w:b w:val="0"/><w:bCs w:val="0"/>"# });
    }
    if let Some(i) = f.italic {
        s.push_str(if i { "<w:i/><w:iCs/>" } else { r#"<w:i w:val="0"/><w:iCs w:val="0"/>"# });
    }
    if let Some(c) = f.caps {
        match c {
            Caps::All => s.push_str("<w:caps/>"),
            Caps::Small => s.push_str("<w:smallCaps/>"),
            Caps::None => s.push_str(r#"<w:caps w:val="0"/><w:smallCaps w:val="0"/>"#),
        }
    }
    if let Some(st) = f.strike {
        s.push_str(if st { "<w:strike/>" } else { r#"<w:strike w:val="0"/>"# });
    }
    if f.outline.is_some_and(|o| o > 0.0) {
        s.push_str("<w:outline/>");
    }
    if let Some(c) = f.color {
        let _ = write!(s, r#"<w:color w:val="{}"/>"#, c.hex6());
    }
    if let Some(t) = f.tracking {
        let _ = write!(s, r#"<w:spacing w:val="{}"/>"#, tw(t));
    }
    if let Some(sc) = f.scale {
        let _ = write!(s, r#"<w:w w:val="{}"/>"#, sc.round() as i64);
    }
    if let Some(b) = f.baseline {
        let _ = write!(s, r#"<w:position w:val="{}"/>"#, (b * 2.0).round() as i64);
    }
    if let Some(sz) = f.size {
        let v = (sz * 2.0).round() as i64;
        let _ = write!(s, r#"<w:sz w:val="{v}"/><w:szCs w:val="{v}"/>"#);
    }
    if let Some(h) = f.highlight {
        if !h.is_none() {
            let _ = write!(s, r#"<w:shd w:val="clear" w:color="auto" w:fill="{}"/>"#, h.hex6());
        }
    }
    if let Some(u) = f.underline {
        let v = match u {
            Underline::None => "none",
            Underline::Single => "single",
            Underline::Double => "double",
            Underline::Dotted => "dotted",
            Underline::Wave => "wave",
        };
        let _ = write!(s, r#"<w:u w:val="{v}"/>"#);
    }
    if let Some(sc) = f.script {
        let v = match sc {
            Script::Normal => "baseline",
            Script::Super => "superscript",
            Script::Sub => "subscript",
        };
        let _ = write!(s, r#"<w:vertAlign w:val="{v}"/>"#);
    }
    if let Some(l) = &f.lang {
        let code = if l.starts_with("en") { "en-US" } else if l.starts_with("pl") { "pl-PL" } else { l.as_str() };
        let _ = write!(s, r#"<w:lang w:val="{code}"/>"#);
    }
    if !s.is_empty() {
        let _ = write!(out, "<w:rPr>{s}</w:rPr>");
    }
}

fn num_id(k: ListKind) -> u32 {
    match k {
        ListKind::Bullet => 1,
        ListKind::Dash => 2,
        ListKind::Check => 3,
        ListKind::Arrow => 4,
        ListKind::Number => 5,
        ListKind::NumberParen => 6,
        ListKind::LowerLetter => 7,
        ListKind::UpperLetter => 8,
        ListKind::LowerRoman => 9,
        ListKind::UpperRoman => 10,
    }
}

fn ppr(f: &ParaFmt, style: Option<&str>, out: &mut String) {
    let mut s = String::new();
    if let Some(st) = style {
        let _ = write!(s, r#"<w:pStyle w:val="{}"/>"#, word_style_id(st));
    }
    if f.keep_next == Some(true) {
        s.push_str("<w:keepNext/>");
    }
    if f.keep_lines == Some(true) {
        s.push_str("<w:keepLines/>");
    }
    if f.break_before == Some(Break::Page) {
        s.push_str("<w:pageBreakBefore/>");
    }
    if let Some(w) = f.widows {
        s.push_str(if w { "<w:widowControl/>" } else { r#"<w:widowControl w:val="0"/>"# });
    }
    if let Some(l) = f.list {
        match l {
            Some(l) => {
                let _ = write!(s, r#"<w:numPr><w:ilvl w:val="{}"/><w:numId w:val="{}"/></w:numPr>"#, l.level.min(8), num_id(l.kind));
            }
            None => s.push_str(r#"<w:numPr><w:ilvl w:val="0"/><w:numId w:val="0"/></w:numPr>"#),
        }
    }
    if let Some(Some(b)) = f.border {
        if b.width > 0.0 {
            let sz = (b.width * 8.0).round().max(2.0) as i64;
            let sp = b.padding.round() as i64;
            let c = b.color.hex6();
            s.push_str("<w:pBdr>");
            for side in ["top", "left", "bottom", "right"] {
                let _ = write!(s, r#"<w:{side} w:val="single" w:sz="{sz}" w:space="{sp}" w:color="{c}"/>"#);
            }
            s.push_str("</w:pBdr>");
        }
    }
    if let Some(sh) = f.shading {
        if !sh.is_none() {
            let _ = write!(s, r#"<w:shd w:val="clear" w:color="auto" w:fill="{}"/>"#, sh.hex6());
        }
    }
    if let Some(tabs) = &f.tabs {
        if !tabs.is_empty() {
            s.push_str("<w:tabs>");
            for t in tabs {
                if *t < 0.0 {
                    s.push_str(r#"<w:tab w:val="right" w:leader="dot" w:pos="9350"/>"#);
                } else {
                    let _ = write!(s, r#"<w:tab w:val="left" w:pos="{}"/>"#, tw(*t));
                }
            }
            s.push_str("</w:tabs>");
        }
    }
    if f.space_before.is_some() || f.space_after.is_some() || f.line.is_some() {
        s.push_str("<w:spacing");
        if let Some(b) = f.space_before {
            let _ = write!(s, r#" w:before="{}""#, tw(b));
        }
        if let Some(a) = f.space_after {
            let _ = write!(s, r#" w:after="{}""#, tw(a));
        }
        match f.line {
            Some(LineSpacing::Multiple(k)) => {
                let _ = write!(s, r#" w:line="{}" w:lineRule="auto""#, (k * 240.0).round() as i64);
            }
            Some(LineSpacing::Exact(v)) => {
                let _ = write!(s, r#" w:line="{}" w:lineRule="exact""#, tw(v));
            }
            Some(LineSpacing::AtLeast(v)) => {
                let _ = write!(s, r#" w:line="{}" w:lineRule="atLeast""#, tw(v));
            }
            None => {}
        }
        s.push_str("/>");
    }
    if f.indent_left.is_some() || f.indent_right.is_some() || f.first_line.is_some() {
        s.push_str("<w:ind");
        if let Some(l) = f.indent_left {
            let _ = write!(s, r#" w:left="{}""#, tw(l));
        }
        if let Some(r) = f.indent_right {
            let _ = write!(s, r#" w:right="{}""#, tw(r));
        }
        if let Some(fl) = f.first_line {
            if fl >= 0.0 {
                let _ = write!(s, r#" w:firstLine="{}""#, tw(fl));
            } else {
                let _ = write!(s, r#" w:hanging="{}""#, tw(-fl));
            }
        }
        s.push_str("/>");
    }
    if let Some(a) = f.align {
        let v = match a {
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
            Align::Justify => "both",
        };
        let _ = write!(s, r#"<w:jc w:val="{v}"/>"#);
    }
    if !s.is_empty() {
        let _ = write!(out, "<w:pPr>{s}</w:pPr>");
    }
}

fn text_runs(text: &str, out: &mut String) {
    let mut buf = String::new();
    let flush = |buf: &mut String, out: &mut String| {
        if !buf.is_empty() {
            let _ = write!(out, r#"<w:t xml:space="preserve">{}</w:t>"#, esc(buf));
            buf.clear();
        }
    };
    for c in text.chars() {
        match c {
            '\t' => {
                flush(&mut buf, out);
                out.push_str("<w:tab/>");
            }
            '\n' | '\u{2028}' => {
                flush(&mut buf, out);
                out.push_str("<w:br/>");
            }
            '\u{00AD}' => {
                flush(&mut buf, out);
                out.push_str("<w:softHyphen/>");
            }
            c => buf.push(c),
        }
    }
    flush(&mut buf, out);
}

fn runs_xml(ctx: &mut Ctx, runs: &[Run], out: &mut String) {
    for r in runs {
        if r.text.is_empty() {
            continue;
        }
        let mut rp = String::new();
        rpr(&r.fmt, &mut rp);
        if let Some(f) = r.field {
            if matches!(f, Field::Anchor(_)) {
                continue;
            }
            let instr = match f {
                Field::PageNumber => "PAGE",
                Field::PageCount => "NUMPAGES",
                Field::Date => r#"DATE \@ "dd.MM.yyyy""#,
                Field::Time => r#"TIME \@ "HH:mm""#,
                Field::Title => "TITLE",
                Field::Author => "AUTHOR",
                Field::Anchor(_) => "",
            };
            let _ = write!(out, r#"<w:fldSimple w:instr="{}"><w:r>{rp}<w:t>1</w:t></w:r></w:fldSimple>"#, esc(instr));
            continue;
        }
        let mut inner = String::new();
        let _ = write!(inner, "<w:r>{rp}");
        text_runs(&r.text, &mut inner);
        inner.push_str("</w:r>");
        match &r.fmt.link {
            Some(url) if !url.is_empty() => {
                let id = ctx.link_rel(url);
                let _ = write!(out, r#"<w:hyperlink r:id="{id}" w:history="1">{inner}</w:hyperlink>"#);
            }
            _ => out.push_str(&inner),
        }
    }
}

fn para_xml(ctx: &mut Ctx, p: &Paragraph, extra_runs: &str, out: &mut String) {
    out.push_str("<w:p>");
    let mut f = p.fmt.clone();
    let mut drop = String::new();
    if f.drop_cap.is_some_and(|d| d >= 2) {
        f.drop_cap = None;
        drop = format!(r#"<w:framePr w:dropCap="drop" w:lines="{}" w:wrap="around" w:vAnchor="text" w:hAnchor="text"/>"#, p.fmt.drop_cap.unwrap_or(3));
    }
    let mut pp = String::new();
    ppr(&f, Some(&p.style), &mut pp);
    if !drop.is_empty() {
        // Inicjał w Wordzie to osobny akapit-ramka z pierwszą literą.
        let text = p.text();
        if let Some(c) = text.chars().next() {
            let mut first = p.clone();
            let mut rest = p.clone();
            let split = c.len_utf8();
            let i = first.split_at(split);
            first.runs.truncate(i);
            let j = rest.split_at(split);
            rest.runs.drain(..j);
            let _ = write!(out, "<w:pPr><w:pStyle w:val=\"{}\"/>{drop}</w:pPr>", word_style_id(&p.style));
            runs_xml(ctx, &first.runs, out);
            out.push_str("</w:p><w:p>");
            out.push_str(&pp);
            out.push_str(extra_runs);
            runs_xml(ctx, &rest.runs, out);
            out.push_str("</w:p>");
            return;
        }
    }
    out.push_str(&pp);
    if f.break_before == Some(Break::Column) {
        out.push_str(r#"<w:r><w:br w:type="column"/></w:r>"#);
    }
    out.push_str(extra_runs);
    runs_xml(ctx, &p.runs, out);
    out.push_str("</w:p>");
}

fn story_xml(ctx: &mut Ctx, s: &Story) -> String {
    let mut out = String::new();
    for p in &s.paras {
        para_xml(ctx, p, "", &mut out);
    }
    out
}

fn fill_xml(f: Option<&Fill>) -> String {
    let clr = |c: &Color| {
        if c.a == 255 {
            format!(r#"<a:srgbClr val="{}"/>"#, c.hex6())
        } else {
            format!(r#"<a:srgbClr val="{}"><a:alpha val="{}"/></a:srgbClr>"#, c.hex6(), (c.a as u32 * 100_000 / 255))
        }
    };
    match f {
        None => "<a:noFill/>".into(),
        Some(Fill::Solid { color }) => format!("<a:solidFill>{}</a:solidFill>", clr(color)),
        Some(Fill::Linear { from, to, angle }) => format!(
            r#"<a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0">{}</a:gs><a:gs pos="100000">{}</a:gs></a:gsLst><a:lin ang="{}" scaled="0"/></a:gradFill>"#,
            clr(from),
            clr(to),
            ((angle.rem_euclid(360.0)) * 60000.0) as i64
        ),
        Some(Fill::Radial { from, to }) => format!(
            r#"<a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0">{}</a:gs><a:gs pos="100000">{}</a:gs></a:gsLst><a:path path="circle"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path></a:gradFill>"#,
            clr(from),
            clr(to)
        ),
    }
}

fn ln_xml(s: Option<&StrokeStyle>) -> String {
    match s {
        Some(s) if s.width > 0.0 => {
            let dash = if s.dash.len() >= 2 { if s.dash[0] <= s.width * 1.5 { r#"<a:prstDash val="sysDot"/>"# } else { r#"<a:prstDash val="dash"/>"# } } else { "" };
            format!(r#"<a:ln w="{}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill>{dash}</a:ln>"#, emu(s.width), s.color.hex6())
        }
        _ => "<a:ln><a:noFill/></a:ln>".into(),
    }
}

fn geom_xml(o: &Object) -> String {
    if o.shape.is_rect() {
        if o.corner > 0.0 {
            let adj = ((o.corner / o.w.min(o.h).max(1.0)) * 100_000.0).min(50_000.0) as i64;
            return format!(r#"<a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val {adj}"/></a:avLst></a:prstGeom>"#);
        }
        return r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#.into();
    }
    let (w, h) = (emu(o.w), emu(o.h));
    let p = |x: f32, y: f32| format!(r#"<a:pt x="{}" y="{}"/>"#, (x * w as f32) as i64, (y * h as f32) as i64);
    let mut s = format!(r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="0" t="0" r="r" b="b"/><a:pathLst><a:path w="{w}" h="{h}">"#);
    for c in &o.shape.contours {
        let n = c.nodes.len();
        if n == 0 {
            continue;
        }
        let _ = write!(s, "<a:moveTo>{}</a:moveTo>", p(c.nodes[0].x, c.nodes[0].y));
        let count = if c.closed { n } else { n - 1 };
        for i in 0..count {
            let a = &c.nodes[i];
            let b = &c.nodes[(i + 1) % n];
            if a.cout.is_none() && b.cin.is_none() {
                let _ = write!(s, "<a:lnTo>{}</a:lnTo>", p(b.x, b.y));
            } else {
                let c1 = a.cout.unwrap_or((a.x, a.y));
                let c2 = b.cin.unwrap_or((b.x, b.y));
                let _ = write!(s, "<a:cubicBezTo>{}{}{}</a:cubicBezTo>", p(c1.0, c1.1), p(c2.0, c2.1), p(b.x, b.y));
            }
        }
        if c.closed {
            s.push_str("<a:close/>");
        }
    }
    s.push_str("</a:path></a:pathLst></a:custGeom>");
    s
}

fn wrap_xml(o: &Object) -> (String, bool) {
    let side = match o.wrap.mode {
        WrapMode::Left => "left",
        WrapMode::Right => "right",
        WrapMode::Largest => "largest",
        _ => "bothSides",
    };
    match o.wrap.mode {
        WrapMode::None => ("<wp:wrapNone/>".into(), false),
        WrapMode::Behind => ("<wp:wrapNone/>".into(), true),
        WrapMode::TopBottom => ("<wp:wrapTopAndBottom/>".into(), false),
        _ => {
            let shape = match &o.kind {
                ObjKind::Image { contour: Some(c), .. } => c,
                _ => &o.shape,
            };
            if o.wrap.contour && !shape.is_rect() {
                let polys = geom::shape_polys(shape, geom::Rect::new(0.0, 0.0, 1.0, 1.0));
                if let Some(poly) = polys.first() {
                    let k = 21600.0;
                    let pt = |q: &geom::Pt| format!(r#"x="{}" y="{}""#, (q.x * k).round() as i64, (q.y * k).round() as i64);
                    let mut s = format!(r#"<wp:wrapTight wrapText="{side}"><wp:wrapPolygon edited="1"><wp:start {}/>"#, pt(&poly[0]));
                    for q in poly.iter().skip(1).step_by((poly.len() / 120).max(1)) {
                        let _ = write!(s, "<wp:lineTo {}/>", pt(q));
                    }
                    let _ = write!(s, "<wp:lineTo {}/></wp:wrapPolygon></wp:wrapTight>", pt(&poly[0]));
                    return (s, false);
                }
            }
            (format!(r#"<wp:wrapSquare wrapText="{side}"/>"#), false)
        }
    }
}

fn anchor(ctx: &mut Ctx, o: &Object, z: usize, x: f32, y: f32, w: f32, h: f32, wrap: (String, bool), graphic: &str) -> String {
    ctx.next_doc_pr += 1;
    let d = emu(o.wrap.dist);
    format!(
        concat!(
            r#"<w:r><w:drawing><wp:anchor distT="{d}" distB="{d}" distL="{d}" distR="{d}" simplePos="0" relativeHeight="{rh}" behindDoc="{behind}" locked="0" layoutInCell="1" allowOverlap="1">"#,
            r#"<wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>{x}</wp:posOffset></wp:positionH>"#,
            r#"<wp:positionV relativeFrom="page"><wp:posOffset>{y}</wp:posOffset></wp:positionV><wp:extent cx="{w}" cy="{h}"/>"#,
            r#"<wp:effectExtent l="0" t="0" r="0" b="0"/>{wrap}<wp:docPr id="{id}" name="{name}"/><wp:cNvGraphicFramePr/>{graphic}</wp:anchor></w:drawing></w:r>"#
        ),
        d = d,
        rh = 251_659_264 + z as i64 * 1024,
        behind = if wrap.1 { 1 } else { 0 },
        x = emu(x),
        y = emu(y),
        w = emu(w).max(1),
        h = emu(h).max(1),
        wrap = wrap.0,
        id = ctx.next_doc_pr,
        name = esc(if o.name.is_empty() { o.kind_name() } else { &o.name }),
        graphic = graphic,
    )
}

fn pic_graphic(rel: &str, w: f32, h: f32, src: (i64, i64, i64, i64), geometry: &str, ln: &str) -> String {
    format!(
        concat!(
            r#"<a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
            r#"<pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="0" name="obraz"/><pic:cNvPicPr/></pic:nvPicPr>"#,
            r#"<pic:blipFill><a:blip r:embed="{rel}"/><a:srcRect l="{l}" t="{t}" r="{r}" b="{b}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>"#,
            r#"<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{w}" cy="{h}"/></a:xfrm>{geometry}{ln}</pic:spPr></pic:pic></a:graphicData></a:graphic>"#
        ),
        rel = rel,
        l = src.0,
        t = src.1,
        r = src.2,
        b = src.3,
        w = emu(w),
        h = emu(h),
        geometry = geometry,
        ln = ln
    )
}

fn wsp_graphic(o: &Object, txbx: &str, body: &str, geometry: &str) -> String {
    format!(
        concat!(
            r#"<a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">"#,
            r#"<wps:wsp><wps:cNvSpPr{tb}/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{w}" cy="{h}"/></a:xfrm>{geometry}{fill}{ln}</wps:spPr>{txbx}{body}</wps:wsp></a:graphicData></a:graphic>"#
        ),
        tb = if txbx.is_empty() { "" } else { r#" txBox="1""# },
        w = emu(o.w),
        h = emu(o.h),
        geometry = geometry,
        fill = fill_xml(o.fill.as_ref()),
        ln = ln_xml(o.stroke.as_ref()),
        txbx = txbx,
        body = body
    )
}

fn render_png(doc: &Document, layout: &Layout, o: &Object) -> Vec<u8> {
    let scale = 300.0 / 72.0;
    let w = (o.w * scale).ceil().max(1.0) as u32;
    let h = (o.h * scale).ceil().max(1.0) as u32;
    let Some(mut pm) = tiny_skia::Pixmap::new(w.min(6000), h.min(6000)) else { return Vec::new() };
    let mut ops = Vec::new();
    match &o.kind {
        ObjKind::Chart { chart } => crate::paint::chart_ops(chart, geom::Rect::new(0.0, 0.0, o.w, o.h), &mut ops),
        ObjKind::Table { table } => crate::paint::table_ops(table, geom::Rect::new(0.0, 0.0, o.w, o.h), &mut ops),
        _ => {
            // Tekst na ścieżce: lista rysowania strony ograniczona do tego obiektu, przesunięta do (0,0).
            let opts = crate::paint::PaintOpts { screen: false, marks: false, guides: false, misspelled: None, hide_obj: None };
            let all = crate::paint::page_ops(doc, layout, o.page, &opts);
            let mut r = crate::raster::Raster::new();
            let Some(mut big) = tiny_skia::Pixmap::new((doc.page.width * scale) as u32, (doc.page.height * scale) as u32) else { return Vec::new() };
            let only: Vec<_> = all.into_iter().filter(|op| matches!(op, crate::paint::Op::Glyphs(g) if g.rot.abs() > 0.0 || g.text.is_empty())).collect();
            r.draw(doc, &only, &mut big, scale);
            let x = (o.x * scale - 20.0).max(0.0) as i32;
            let y = (o.y * scale - 20.0).max(0.0) as i32;
            pm.draw_pixmap(-x, -y, big.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
            return pm.encode_png().unwrap_or_default();
        }
    }
    let mut r = crate::raster::Raster::new();
    r.draw(doc, &ops, &mut pm, scale);
    pm.encode_png().unwrap_or_default()
}

/// Kotwice obiektów strony `page` (wstawiane do pierwszego akapitu tej strony).
fn objects_xml(ctx: &mut Ctx, page: usize) -> String {
    let doc = ctx.doc;
    let layout = ctx.layout;
    let mut out = String::new();
    for (z, o) in doc.objects.iter().enumerate().filter(|(_, o)| o.page == page) {
        let wrap = wrap_xml(o);
        match &o.kind {
            ObjKind::Image { asset, fit, .. } => {
                let a = &doc.assets[asset];
                let rel = if a.mime == "image/svg+xml" {
                    let k = 300.0 / 72.0;
                    match crate::raster::render_svg(&a.data, (o.w * k) as u32, (o.h * k) as u32).and_then(|pm| pm.encode_png().ok()) {
                        Some(png) => ctx.png_rel(png),
                        None => continue,
                    }
                } else {
                    let Some(rel) = ctx.image_rel(asset) else { continue };
                    rel
                };
                let (iw, ih) = (a.width.max(1) as f32, a.height.max(1) as f32);
                // Przycięcie dla „wypełnij” (srcRect w tysięcznych procenta).
                let src = if *fit == ImageFit::Cover {
                    let k = (o.w / iw).max(o.h / ih);
                    let (w, h) = (iw * k, ih * k);
                    let cx = (((w - o.w) / 2.0) / w * 100_000.0) as i64;
                    let cy = (((h - o.h) / 2.0) / h * 100_000.0) as i64;
                    (cx, cy, cx, cy)
                } else {
                    (0, 0, 0, 0)
                };
                let g = pic_graphic(&rel, o.w, o.h, src, &geom_xml(o), &ln_xml(o.stroke.as_ref()));
                out.push_str(&anchor(ctx, o, z, o.x, o.y, o.w, o.h, wrap, &g));
            }
            ObjKind::Table { .. } if o.cont_of.is_some() || doc.body.paras.iter().any(|p| p.runs.iter().any(|r| r.field == Some(Field::Anchor(o.id)))) => continue,
            ObjKind::Chart { .. } | ObjKind::PathText { .. } | ObjKind::Table { .. } => {
                let png = render_png(doc, layout, o);
                if png.is_empty() {
                    continue;
                }
                let rel = ctx.png_rel(png);
                let g = pic_graphic(&rel, o.w, o.h, (0, 0, 0, 0), r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#, "<a:ln><a:noFill/></a:ln>");
                out.push_str(&anchor(ctx, o, z, o.x, o.y, o.w, o.h, wrap, &g));
            }
            ObjKind::Shape => {
                let g = wsp_graphic(o, "", r#"<wps:bodyPr/>"#, &geom_xml(o));
                out.push_str(&anchor(ctx, o, z, o.x, o.y, o.w, o.h, wrap, &g));
            }
            ObjKind::Text { padding, columns, gap, valign, .. } => {
                let head = doc.chain_head(o.id);
                let chain = doc.chain(head);
                let exact = ctx.opts.exact_shapes && (!o.shape.is_rect() || chain.len() > 1 && false);
                let anchor_v = match valign {
                    VAlign::Top => "t",
                    VAlign::Center => "ctr",
                    VAlign::Bottom => "b",
                };
                let ins = emu(*padding);
                let body = format!(r#"<wps:bodyPr rot="0" vert="horz" wrap="square" lIns="{ins}" tIns="{ins}" rIns="{ins}" bIns="{ins}" anchor="{anchor_v}" numCol="{}" spcCol="{}"><a:noAutofit/></wps:bodyPr>"#, (*columns).max(1), emu(*gap));
                if exact {
                    // Sam kształt (bez tekstu) + każdy odcinek wiersza jako pole w dokładnym miejscu.
                    let g = wsp_graphic(o, "", r#"<wps:bodyPr/>"#, &geom_xml(o));
                    out.push_str(&anchor(ctx, o, z, o.x, o.y, o.w, o.h, wrap, &g));
                    let story = doc.object(head).and_then(|h| h.story());
                    for &li in layout.story_lines(StoryRef::Obj(head)) {
                        let l = &layout.lines[li];
                        if l.obj != Some(o.id) {
                            continue;
                        }
                        let Some(p) = story.and_then(|s| s.paras.get(l.para)) else { continue };
                        for sg in &l.segs {
                            if sg.g1 <= sg.g0 {
                                continue;
                            }
                            let a = l.glyphs[sg.g0].g.cluster;
                            let b = l.glyphs.get(sg.g1).map(|g| g.g.cluster).unwrap_or(if l.last { p.len() } else { l.end });
                            let frag = crate::edit::fragment(story.unwrap(), crate::edit::At::new(l.para, a), crate::edit::At::new(l.para, b.max(a)));
                            let mut fp = frag.into_iter().next().unwrap_or_default();
                            fp.fmt.space_before = Some(0.0);
                            fp.fmt.space_after = Some(0.0);
                            fp.fmt.indent_left = Some(0.0);
                            fp.fmt.indent_right = Some(0.0);
                            fp.fmt.first_line = Some(0.0);
                            fp.fmt.line = Some(LineSpacing::Exact(l.height));
                            fp.fmt.list = Some(None);
                            let (pp, _) = doc.resolve_para(p);
                            if pp.align == Align::Justify && !l.last {
                                fp.fmt.align = Some(Align::Justify);
                            }
                            let mut px = String::new();
                            para_xml(ctx, &fp, "", &mut px);
                            let x0 = l.glyphs[sg.g0].x;
                            let w = (sg.x1 - x0).max(4.0) + 2.0;
                            let lo = Object { wrap: Wrap { mode: WrapMode::None, ..Wrap::default() }, fill: None, stroke: None, shape: Shape::rect(), w, h: l.height, ..o.clone() };
                            let txbx = format!("<wps:txbx><w:txbxContent>{px}</w:txbxContent></wps:txbx>");
                            let g = wsp_graphic(&lo, &txbx, r#"<wps:bodyPr rot="0" vert="horz" wrap="none" lIns="0" tIns="0" rIns="0" bIns="0" anchor="t"><a:noAutofit/></wps:bodyPr>"#, r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#);
                            out.push_str(&anchor(ctx, &lo, z, x0, l.top, w, l.height, ("<wp:wrapNone/>".into(), false), &g));
                        }
                    }
                    continue;
                }
                let txbx = if o.id == head {
                    let story = o.story().cloned().unwrap_or_default();
                    let content = story_xml(ctx, &story);
                    if chain.len() > 1 {
                        let n = ctx.txbx_ids.len() as u32 + 1;
                        ctx.txbx_ids.insert(head, n);
                        format!(r#"<wps:txbx id="{n}"><w:txbxContent>{content}</w:txbxContent></wps:txbx>"#)
                    } else {
                        format!("<wps:txbx><w:txbxContent>{content}</w:txbxContent></wps:txbx>")
                    }
                } else {
                    let n = *ctx.txbx_ids.entry(head).or_insert(0);
                    let seq = chain.iter().position(|&c| c == o.id).unwrap_or(1);
                    format!(r#"<wps:linkedTxbx id="{n}" seq="{seq}"/>"#)
                };
                let g = wsp_graphic(o, &txbx, &body, &geom_xml(o));
                out.push_str(&anchor(ctx, o, z, o.x, o.y, o.w, o.h, wrap, &g));
            }
        }
    }
    out
}

/// Tabela zakotwiczona w tekście jako `w:tbl` (cała – Word sam dzieli ją na strony i powtarza nagłówek).
fn table_xml(ctx: &mut Ctx, t: &Table, width: f32) -> String {
    let cols = t.cols();
    let mut full = t.clone();
    full.take = None;
    let edges = crate::paint::table_cells(&full, geom::Rect::new(0.0, 0.0, width, 1000.0));
    let mut widths = vec![width / cols as f32; cols];
    for (ri, c, r) in &edges {
        if *ri == 0 && *c < cols {
            widths[*c] = r.w;
        }
    }
    let line = t.line.hex6();
    let mut s = format!(r#"<w:tbl><w:tblPr><w:tblW w:w="{}" w:type="dxa"/><w:tblBorders>"#, tw(width));
    for side in ["top", "left", "bottom", "right", "insideH", "insideV"] {
        let _ = write!(s, r#"<w:{side} w:val="single" w:sz="4" w:space="0" w:color="{line}"/>"#);
    }
    s.push_str(r#"</w:tblBorders><w:tblLayout w:type="fixed"/><w:tblCellMar><w:left w:w="80" w:type="dxa"/><w:right w:w="80" w:type="dxa"/></w:tblCellMar></w:tblPr><w:tblGrid>"#);
    for w in &widths {
        let _ = write!(s, r#"<w:gridCol w:w="{}"/>"#, tw(*w));
    }
    s.push_str("</w:tblGrid>");
    for (ri, row) in t.rows.iter().enumerate() {
        let head = t.header && ri == 0;
        s.push_str("<w:tr>");
        if head {
            s.push_str("<w:trPr><w:tblHeader/></w:trPr>");
        }
        let mut c = 0;
        while c < cols {
            let mut span = 1;
            while c + span < cols && row.get(c + span).is_some_and(|v| v == MERGED) {
                span += 1;
            }
            let w: f32 = widths[c..c + span].iter().sum();
            let fill = if head {
                Some(t.accent.hex6())
            } else if t.stripes && ri % 2 == 0 {
                let m = |v: u8| (v as f32 * 0.07 + 255.0 * 0.93) as u8;
                Some(Color::rgb(m(t.accent.r), m(t.accent.g), m(t.accent.b)).hex6())
            } else {
                None
            };
            let _ = write!(s, r#"<w:tc><w:tcPr><w:tcW w:w="{}" w:type="dxa"/>"#, tw(w));
            if span > 1 {
                let _ = write!(s, r#"<w:gridSpan w:val="{span}"/>"#);
            }
            if let Some(f) = fill {
                let _ = write!(s, r#"<w:shd w:val="clear" w:color="auto" w:fill="{f}"/>"#);
            }
            s.push_str(r#"<w:vAlign w:val="center"/></w:tcPr><w:p><w:pPr><w:spacing w:before="40" w:after="40"/></w:pPr>"#);
            let mut runs = crate::io::plain::md_inline(row.get(c).map(|v| v.as_str()).unwrap_or(""));
            for r in runs.iter_mut() {
                r.fmt.font = Some(t.font.clone());
                r.fmt.size = Some(t.size);
                if head {
                    r.fmt.bold = Some(true);
                    r.fmt.color = Some(Color::WHITE);
                }
            }
            runs_xml(ctx, &runs, &mut s);
            s.push_str("</w:p></w:tc>");
            c += span;
        }
        s.push_str("</w:tr>");
    }
    s.push_str("</w:tbl>");
    s
}

fn styles_xml(doc: &Document) -> String {
    let mut s = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri" w:eastAsia="Calibri"/><w:sz w:val="22"/><w:szCs w:val="22"/><w:lang w:val="pl-PL"/></w:rPr></w:rPrDefault>"#,
        r#"<w:pPrDefault><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>"#
    ));
    for st in &doc.styles {
        let id = word_style_id(&st.id);
        let _ = write!(s, r#"<w:style w:type="paragraph"{} w:styleId="{id}"><w:name w:val="{}"/>"#, if st.id == "normal" { r#" w:default="1""# } else { "" }, esc(&word_style_name(st)));
        if let Some(b) = &st.based_on {
            let _ = write!(s, r#"<w:basedOn w:val="{}"/>"#, word_style_id(b));
        }
        if let Some(n) = &st.next {
            let _ = write!(s, r#"<w:next w:val="{}"/>"#, word_style_id(n));
        }
        if st.gallery {
            s.push_str("<w:qFormat/>");
        }
        let mut pf = st.para.clone();
        if let Some(l) = st.outline {
            let _ = l;
        }
        let mut pp = String::new();
        ppr(&pf, None, &mut pp);
        if let Some(l) = st.outline {
            let lvl = format!(r#"<w:outlineLvl w:val="{}"/>"#, l.saturating_sub(1));
            if pp.is_empty() {
                pp = format!("<w:pPr>{lvl}</w:pPr>");
            } else {
                pp = pp.replace("</w:pPr>", &format!("{lvl}</w:pPr>"));
            }
        }
        pf.list = None;
        s.push_str(&pp);
        rpr(&st.chars, &mut s);
        s.push_str("</w:style>");
    }
    s.push_str(r#"<w:style w:type="character" w:styleId="Hyperlink"><w:name w:val="Hyperlink"/><w:rPr><w:color w:val="0563C1"/><w:u w:val="single"/></w:rPr></w:style>"#);
    s.push_str("</w:styles>");
    s
}

fn numbering_xml() -> String {
    let mut s = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#);
    let kinds = [
        (ListKind::Bullet, "bullet", "•"),
        (ListKind::Dash, "bullet", "–"),
        (ListKind::Check, "bullet", "✓"),
        (ListKind::Arrow, "bullet", "➔"),
        (ListKind::Number, "decimal", "%1."),
        (ListKind::NumberParen, "decimal", "%1)"),
        (ListKind::LowerLetter, "lowerLetter", "%1)"),
        (ListKind::UpperLetter, "upperLetter", "%1."),
        (ListKind::LowerRoman, "lowerRoman", "%1."),
        (ListKind::UpperRoman, "upperRoman", "%1."),
    ];
    for (k, fmt, txt) in kinds {
        let id = num_id(k);
        let _ = write!(s, r#"<w:abstractNum w:abstractNumId="{id}"><w:multiLevelType w:val="hybridMultilevel"/>"#);
        for lvl in 0..9 {
            let text = if fmt == "bullet" { if k == ListKind::Bullet { ["•", "◦", "▪"][lvl % 3].to_string() } else { txt.to_string() } } else { txt.replace("%1", &format!("%{}", lvl + 1)) };
            let left = 360 * (lvl + 1);
            let _ = write!(
                s,
                r#"<w:lvl w:ilvl="{lvl}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/><w:lvlText w:val="{}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{left}" w:hanging="360"/></w:pPr></w:lvl>"#,
                esc(&text)
            );
        }
        s.push_str("</w:abstractNum>");
    }
    for (k, _, _) in kinds {
        let id = num_id(k);
        let _ = write!(s, r#"<w:num w:numId="{id}"><w:abstractNumId w:val="{id}"/></w:num>"#);
    }
    s.push_str("</w:numbering>");
    s
}

const NS: &str = concat!(
    r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" "#,
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" "#,
    r#"xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" "#,
    r#"xmlns:wp14="http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing" "#,
    r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="wp14""#
);

pub fn export(doc: &Document, layout: &Layout, opts: &DocxOpts) -> Result<Vec<u8>, String> {
    let mut ctx = Ctx { doc, layout, opts, rels: Vec::new(), media: Vec::new(), asset_rel: HashMap::new(), next_doc_pr: 0, links: HashMap::new(), txbx_ids: HashMap::new() };
    let pages = layout.pages.len().max(1);

    // Treść główna: obiekty każdej strony w pierwszym akapicie, który się na niej zaczyna.
    let mut first_para_of_page: HashMap<usize, usize> = HashMap::new();
    for (i, &pg) in layout.body_pages.iter().enumerate() {
        first_para_of_page.entry(pg).or_insert(i);
    }
    let mut body = String::new();
    let mut placed = vec![false; pages];
    for (i, p) in doc.body.paras.iter().enumerate() {
        let pg = layout.body_pages.get(i).copied().unwrap_or(0);
        let mut extra = String::new();
        if first_para_of_page.get(&pg) == Some(&i) && pg < pages && !placed[pg] {
            placed[pg] = true;
            extra = objects_xml(&mut ctx, pg);
        }
        para_xml(&mut ctx, p, &extra, &mut body);
        for r in &p.runs {
            if let Some(Field::Anchor(id)) = r.field {
                if let Some(ObjKind::Table { table }) = doc.object(id).map(|o| &o.kind) {
                    let w = doc.object(id).map(|o| o.w).unwrap_or(400.0);
                    body.push_str(&table_xml(&mut ctx, table, w));
                }
            }
        }
    }
    // Strony bez treści głównej (same obiekty) – puste akapity z podziałem strony.
    for pg in 0..pages {
        if !placed[pg] {
            let extra = objects_xml(&mut ctx, pg);
            let _ = write!(body, r#"<w:p><w:pPr><w:pageBreakBefore/></w:pPr>{extra}</w:p>"#);
        }
    }

    // Nagłówki i stopki.
    let mut hf_parts: Vec<(String, String)> = Vec::new();
    let mut sect_refs = String::new();
    for (story, kind, typ) in [(&doc.header, "header", "default"), (&doc.footer, "footer", "default"), (&doc.header_first, "header", "first"), (&doc.footer_first, "footer", "first")] {
        let Some(s) = story else { continue };
        if typ == "first" && !doc.page.different_first {
            continue;
        }
        let content = story_xml(&mut ctx, s);
        let name = format!("{kind}{}.xml", hf_parts.len() + 1);
        let tag = if kind == "header" { "w:hdr" } else { "w:ftr" };
        hf_parts.push((name.clone(), format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><{tag} {NS}>{content}</{tag}>"#)));
        let rel = ctx.rel(&format!("http://schemas.openxmlformats.org/officeDocument/2006/relationships/{kind}"), &name, false);
        let _ = write!(sect_refs, r#"<w:{kind}Reference w:type="{typ}" r:id="{rel}"/>"#);
    }
    let ps = &doc.page;
    let sect = format!(
        concat!(
            r#"<w:sectPr>{refs}<w:pgSz w:w="{w}" w:h="{h}"{orient}/><w:pgMar w:top="{t}" w:right="{r}" w:bottom="{b}" w:left="{l}" w:header="{hd}" w:footer="{fd}" w:gutter="0"/>"#,
            r#"{borders}<w:pgNumType w:start="{start}"/><w:cols w:num="{cols}" w:space="{gap}"{sep}/>{title}</w:sectPr>"#
        ),
        refs = sect_refs,
        w = tw(ps.width),
        h = tw(ps.height),
        orient = if ps.landscape() { r#" w:orient="landscape""# } else { "" },
        t = tw(ps.margin_top),
        r = tw(ps.margin_right),
        b = tw(ps.margin_bottom),
        l = tw(ps.margin_left),
        hd = tw(ps.header_dist),
        fd = tw(ps.footer_dist),
        start = ps.first_number,
        cols = ps.columns.max(1),
        gap = tw(ps.column_gap),
        sep = if ps.column_rule { r#" w:sep="1""# } else { "" },
        title = if ps.different_first { "<w:titlePg/>" } else { "" },
        borders = match &ps.border {
            Some(b) if b.width > 0.0 => {
                let sz = (b.width * 8.0).round().clamp(2.0, 96.0) as i64;
                let sp = ps.border_inset.round().clamp(0.0, 31.0) as i64;
                let c = b.color.hex6();
                let v = if b.dash.len() >= 2 { "dashed" } else { "single" };
                format!(r#"<w:pgBorders w:offsetFrom="page"><w:top w:val="{v}" w:sz="{sz}" w:space="{sp}" w:color="{c}"/><w:left w:val="{v}" w:sz="{sz}" w:space="{sp}" w:color="{c}"/><w:bottom w:val="{v}" w:sz="{sz}" w:space="{sp}" w:color="{c}"/><w:right w:val="{v}" w:sz="{sz}" w:space="{sp}" w:color="{c}"/></w:pgBorders>"#)
            }
            _ => String::new(),
        },
    );
    let bg = if ps.color != Color::WHITE { format!(r#"<w:background w:color="{}"/>"#, ps.color.hex6()) } else { String::new() };
    let document = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {NS}>{bg}<w:body>{body}{sect}</w:body></w:document>"#);

    // Paczka.
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut put = |name: &str, data: &[u8]| -> Result<(), String> {
            z.start_file(name, o).map_err(|e| e.to_string())?;
            z.write_all(data).map_err(|e| e.to_string())
        };
        let mut ct = String::from(concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
            r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#,
            r#"<Default Extension="png" ContentType="image/png"/><Default Extension="jpeg" ContentType="image/jpeg"/><Default Extension="gif" ContentType="image/gif"/><Default Extension="bmp" ContentType="image/bmp"/><Default Extension="webp" ContentType="image/webp"/>"#,
            r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>"#,
            r#"<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>"#,
            r#"<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>"#,
            r#"<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>"#,
            r#"<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>"#,
            r#"<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>"#
        ));
        for (name, _) in &hf_parts {
            let kind = if name.starts_with("header") { "header" } else { "footer" };
            let _ = write!(ct, r#"<Override PartName="/word/{name}" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.{kind}+xml"/>"#);
        }
        ct.push_str("</Types>");
        put("[Content_Types].xml", ct.as_bytes())?;
        put(
            "_rels/.rels",
            concat!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
                r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>"#,
                r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>"#,
                r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>"#,
                r#"</Relationships>"#
            )
            .as_bytes(),
        )?;
        let now = crate::util::iso_now();
        let core = format!(
            concat!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">"#,
                r#"<dc:title>{}</dc:title><dc:subject>{}</dc:subject><dc:creator>{}</dc:creator><cp:keywords>{}</cp:keywords>"#,
                r#"<dcterms:created xsi:type="dcterms:W3CDTF">{}Z</dcterms:created><dcterms:modified xsi:type="dcterms:W3CDTF">{}Z</dcterms:modified></cp:coreProperties>"#
            ),
            esc(&doc.meta.title),
            esc(&doc.meta.subject),
            esc(&doc.meta.author),
            esc(&doc.meta.keywords),
            if doc.meta.created.is_empty() { now.clone() } else { doc.meta.created.clone() },
            now
        );
        put("docProps/core.xml", core.as_bytes())?;
        put("docProps/app.xml", concat!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>OpenDTP</Application></Properties>"#).as_bytes())?;
        put("word/document.xml", document.as_bytes())?;
        put("word/styles.xml", styles_xml(doc).as_bytes())?;
        put("word/numbering.xml", numbering_xml().as_bytes())?;
        let bg_setting = if ps.color != Color::WHITE { "<w:displayBackgroundShape/>" } else { "" };
        put(
            "word/settings.xml",
            format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{bg_setting}<w:defaultTabStop w:val="720"/><w:characterSpacingControl w:val="doNotCompress"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>"#).as_bytes(),
        )?;
        for (name, xml) in &hf_parts {
            put(&format!("word/{name}"), xml.as_bytes())?;
        }
        let mut rels = String::from(concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
            r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
            r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>"#,
            r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>"#
        ));
        for (id, kind, target, ext) in &ctx.rels {
            let _ = write!(rels, r#"<Relationship Id="{id}" Type="{kind}" Target="{}"{}/>"#, esc(target), if *ext { r#" TargetMode="External""# } else { "" });
        }
        rels.push_str("</Relationships>");
        // Nagłówki i stopki nie mają własnych obrazów ani odnośników – wystarczy wspólny plik relacji.
        put("word/_rels/document.xml.rels", rels.as_bytes())?;
        for (name, _) in &hf_parts {
            put(&format!("word/_rels/{name}.rels"), rels.as_bytes())?;
        }
        for (name, data) in &ctx.media {
            z.start_file(format!("word/media/{name}"), zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)).map_err(|e| e.to_string())?;
            z.write_all(data).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}
