//! Import DOCX: akapity, style (wbudowane i własne z `styles.xml`, z dziedziczeniem, listami
//! w stylach i czcionkami motywu), formatowanie znaków i akapitów, listy, tabulatory, złamania,
//! odnośniki, nagłówek i stopka, ustawienia strony i kolumny. Pola tekstowe (kształty z tekstem)
//! trafiają jako akapity z lekkim tłem zaraz za akapitem, w którym stoją; zapasowe kopie VML
//! (`mc:Fallback`) pomijamy, żeby tekst się nie dublował. Tabele – wiersze z tabulatorami na
//! szerokościach kolumn. Obrazy – obiekty zakotwiczone w swoim akapicie („nad i pod”).

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

/// Styl wbudowany po nazwie z Worda (nazwy angielskie są stałe niezależnie od języka Worda).
fn builtin_by_name(name: &str) -> Option<&'static str> {
    Some(match name.to_lowercase().as_str() {
        "normal" => "normal",
        "title" => "title",
        "subtitle" => "subtitle",
        "heading 1" => "heading1",
        "heading 2" => "heading2",
        "heading 3" | "heading 4" | "heading 5" | "heading 6" | "heading 7" | "heading 8" | "heading 9" => "heading3",
        "quote" | "intense quote" => "quote",
        "caption" => "caption",
        "list paragraph" => "list",
        "toc 1" => "toc1",
        "toc 2" => "toc2",
        "toc 3" => "toc3",
        _ => return None,
    })
}

/// Styl po identyfikatorze, gdy dokument nie ma `styles.xml` (starsze pliki).
fn our_style(id: &str) -> String {
    match id.to_lowercase().as_str() {
        "normal" | "normalny" => "normal",
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

/// Czcionki motywu: (nagłówkowa – „major”, tekstowa – „minor”) i kolory motywu (accent1, dk1…).
#[derive(Default)]
struct ThemeFonts {
    major: String,
    minor: String,
    colors: HashMap<String, Color>,
}

fn theme_fonts(z: &Zip) -> ThemeFonts {
    let mut out = ThemeFonts::default();
    let Some(xml) = z.text("word/theme/theme1.xml") else { return out };
    let mut r = Reader::from_str(&xml);
    let mut mode = 0;
    let mut slot: Option<String> = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"a:majorFont" => mode = 1,
                b"a:minorFont" => mode = 2,
                b"a:dk1" | b"a:lt1" | b"a:dk2" | b"a:lt2" | b"a:accent1" | b"a:accent2" | b"a:accent3" | b"a:accent4" | b"a:accent5" | b"a:accent6" | b"a:hlink" | b"a:folHlink" => {
                    slot = Some(String::from_utf8_lossy(&e.name().as_ref()[2..]).to_string());
                }
                _ => {}
            },
            Ok(Event::Empty(e)) if matches!(e.name().as_ref(), b"a:srgbClr" | b"a:sysClr") => {
                if let Some(k) = slot.take() {
                    let v = attr(&e, b"lastClr").or_else(|| attr(&e, b"val")).unwrap_or_default();
                    if let Some(c) = Color::parse(&v) {
                        out.colors.insert(k, c);
                    }
                }
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"a:latin" => {
                let f = attr(&e, b"typeface").unwrap_or_default();
                match mode {
                    1 if out.major.is_empty() => out.major = f,
                    2 if out.minor.is_empty() => out.minor = f,
                    _ => {}
                }
            }
            Ok(Event::End(e)) if matches!(e.name().as_ref(), b"a:majorFont" | b"a:minorFont") => mode = 0,
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
    // Poziom zbieramy w całości (numFmt bywa przed lvlText) i zapisujemy na końcu w:lvl.
    let mut fmt: Option<String> = None;
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
                b"w:lvl" => {
                    fmt = None;
                    lvl_text.clear();
                }
                b"w:lvlText" => lvl_text = val(&e).unwrap_or_default(),
                b"w:numFmt" => fmt = val(&e),
                _ => {}
            },
            Ok(Event::End(e)) if e.name().as_ref() == b"w:lvl" => {
                if let Some(a) = &cur_abs {
                    let k = match fmt.as_deref() {
                        Some("bullet") | Some("none") => {
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
                        _ => {
                            if lvl_text.contains(')') {
                                ListKind::NumberParen
                            } else {
                                ListKind::Number
                            }
                        }
                    };
                    abs.entry(a.clone()).or_default().push(k);
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"w:abstractNum" => cur_abs = None,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    nums.into_iter().filter_map(|(n, a)| abs.get(&a).map(|v| (n, v.clone()))).collect()
}

/// Właściwość znaku z `w:rPr` (treść i style). Zwraca, czy element rozpoznano.
fn char_prop(name: &[u8], e: &BytesStart, f: &mut CharFmt, th: &ThemeFonts) -> bool {
    match name {
        b"w:rFonts" => {
            let theme = |a: Option<String>| match a.as_deref() {
                Some(t) if t.starts_with("major") && !th.major.is_empty() => Some(th.major.clone()),
                Some(t) if t.starts_with("minor") && !th.minor.is_empty() => Some(th.minor.clone()),
                _ => None,
            };
            if let Some(font) = attr(e, b"w:ascii").or_else(|| attr(e, b"w:hAnsi")).or_else(|| theme(attr(e, b"w:asciiTheme"))).or_else(|| theme(attr(e, b"w:hAnsiTheme"))) {
                f.font = Some(font);
            }
        }
        b"w:b" => f.bold = Some(on(e)),
        b"w:i" => f.italic = Some(on(e)),
        b"w:strike" | b"w:dstrike" => f.strike = Some(on(e)),
        b"w:caps" => f.caps = Some(if on(e) { Caps::All } else { Caps::None }),
        b"w:smallCaps" => {
            if on(e) {
                f.caps = Some(Caps::Small);
            }
        }
        b"w:u" => {
            f.underline = Some(match val(e).as_deref() {
                Some("none") => Underline::None,
                Some("double") => Underline::Double,
                Some("dotted") | Some("dottedHeavy") => Underline::Dotted,
                Some("wave") | Some("wavyHeavy") | Some("wavyDouble") => Underline::Wave,
                _ => Underline::Single,
            })
        }
        b"w:sz" => {
            if let Some(v) = val(e).and_then(|v| v.parse::<f32>().ok()) {
                f.size = Some(v / 2.0);
            }
        }
        b"w:color" => {
            if let Some(c) = val(e).filter(|v| v != "auto").and_then(|v| Color::parse(&v)) {
                f.color = Some(c);
            }
        }
        b"w:spacing" => f.tracking = twips(val(e)),
        b"w:vertAlign" => {
            f.script = match val(e).as_deref() {
                Some("superscript") => Some(Script::Super),
                Some("subscript") => Some(Script::Sub),
                _ => Some(Script::Normal),
            }
        }
        b"w:highlight" => {
            f.highlight = match val(e).as_deref() {
                Some("yellow") => Some(Color::rgb(255, 255, 0)),
                Some("green") => Some(Color::rgb(0, 255, 0)),
                Some("cyan") => Some(Color::rgb(0, 255, 255)),
                Some("magenta") => Some(Color::rgb(255, 0, 255)),
                Some("red") => Some(Color::rgb(255, 0, 0)),
                Some("blue") => Some(Color::rgb(0, 0, 255)),
                Some("darkBlue") => Some(Color::rgb(0, 0, 139)),
                Some("darkRed") => Some(Color::rgb(139, 0, 0)),
                Some("darkGreen") => Some(Color::rgb(0, 100, 0)),
                Some("darkYellow") => Some(Color::rgb(128, 128, 0)),
                Some("darkGray") => Some(Color::rgb(0xa9, 0xa9, 0xa9)),
                Some("lightGray") => Some(Color::rgb(0xd3, 0xd3, 0xd3)),
                Some("black") => Some(Color::rgb(0, 0, 0)),
                _ => None,
            }
        }
        b"w:shd" => {
            if let Some(c) = attr(e, b"w:fill").filter(|f| f != "auto").and_then(|f| Color::parse(&f)) {
                f.highlight = Some(c);
            }
        }
        _ => return false,
    }
    true
}

/// Właściwość akapitu z `w:pPr` (treść i style). `num` zbiera numId/ilvl listy.
fn para_prop(name: &[u8], e: &BytesStart, f: &mut ParaFmt, num: &mut Option<(String, u8)>) -> bool {
    match name {
        b"w:jc" => {
            f.align = Some(match val(e).as_deref() {
                Some("center") => Align::Center,
                Some("right") | Some("end") => Align::Right,
                Some("both") | Some("distribute") => Align::Justify,
                _ => Align::Left,
            });
        }
        b"w:ind" => {
            if let Some(v) = twips(attr(e, b"w:left").or_else(|| attr(e, b"w:start"))) {
                f.indent_left = Some(v);
            }
            if let Some(v) = twips(attr(e, b"w:right").or_else(|| attr(e, b"w:end"))) {
                f.indent_right = Some(v);
            }
            if let Some(v) = twips(attr(e, b"w:firstLine")) {
                f.first_line = Some(v);
            }
            if let Some(h) = twips(attr(e, b"w:hanging")) {
                f.first_line = Some(-h);
            }
        }
        b"w:spacing" => {
            if let Some(v) = twips(attr(e, b"w:before")) {
                f.space_before = Some(v);
            }
            if let Some(v) = twips(attr(e, b"w:after")) {
                f.space_after = Some(v);
            }
            if let Some(l) = attr(e, b"w:line").and_then(|v| v.parse::<f32>().ok()) {
                f.line = Some(match attr(e, b"w:lineRule").as_deref() {
                    Some("exact") => LineSpacing::Exact(l / 20.0),
                    Some("atLeast") => LineSpacing::AtLeast(l / 20.0),
                    _ => LineSpacing::Multiple(l / 240.0),
                });
            }
        }
        b"w:keepNext" => f.keep_next = Some(on(e)),
        b"w:keepLines" => f.keep_lines = Some(on(e)),
        b"w:pageBreakBefore" => {
            if on(e) {
                f.break_before = Some(Break::Page);
            }
        }
        b"w:shd" => {
            if let Some(c) = attr(e, b"w:fill").filter(|v| v != "auto").and_then(|v| Color::parse(&v)) {
                f.shading = Some(c);
            }
        }
        b"w:ilvl" => {
            let lvl = val(e).and_then(|v| v.parse().ok()).unwrap_or(0);
            *num = Some((num.take().map(|n| n.0).unwrap_or_default(), lvl));
        }
        b"w:numId" => {
            let lvl = num.as_ref().map(|n| n.1).unwrap_or(0);
            *num = Some((val(e).unwrap_or_default(), lvl));
        }
        b"w:tab" => {
            if val(e).as_deref() != Some("clear") {
                let pos = twips(attr(e, b"w:pos")).unwrap_or(0.0);
                let right = val(e).as_deref() == Some("right") && attr(e, b"w:leader").is_some_and(|l| l != "none");
                f.tabs.get_or_insert_with(Vec::new).push(if right { -1.0 } else { pos });
            }
        }
        _ => return false,
    }
    true
}

/// Lista z numId/ilvl („0” – lista wyłączona, także gdy styl ją ma).
fn apply_num(f: &mut ParaFmt, num: Option<(String, u8)>, nums: &HashMap<String, Vec<ListKind>>) {
    let Some((id, lvl)) = num else { return };
    if id == "0" {
        f.list = Some(None);
    } else if let Some(kinds) = nums.get(&id) {
        let k = kinds.get(lvl as usize).copied().unwrap_or(ListKind::Bullet);
        f.list = Some(Some(ListFmt { kind: k, level: lvl.min(8), start: None }));
        // Wcięcie listy liczy nasz skład.
        f.indent_left = None;
        f.first_line = None;
    } else if !id.is_empty() {
        f.list = Some(Some(ListFmt { kind: ListKind::Bullet, level: lvl.min(8), start: None }));
    }
}

/// Styl z `styles.xml`.
#[derive(Default, Clone)]
struct DStyle {
    paragraph: bool,
    name: String,
    based: Option<String>,
    next: Option<String>,
    para: ParaFmt,
    chars: CharFmt,
    outline: Option<u8>,
    quick: bool,
}

/// Style dokumentu i wartości domyślne (docDefaults).
fn read_styles(z: &Zip, th: &ThemeFonts, nums: &HashMap<String, Vec<ListKind>>) -> (HashMap<String, DStyle>, ParaFmt, CharFmt) {
    let mut out: HashMap<String, DStyle> = HashMap::new();
    let mut def_p = ParaFmt::default();
    let mut def_c = CharFmt::default();
    let Some(xml) = z.text("word/styles.xml") else { return (out, def_p, def_c) };
    let mut r = Reader::from_str(&xml);
    let mut cur: Option<(String, DStyle)> = None;
    let mut in_defaults = false;
    let (mut in_ppr, mut in_rpr) = (false, false);
    let mut num: Option<(String, u8)> = None;
    loop {
        let ev = r.read_event();
        match ev {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let empty = matches!(ev, Ok(Event::Empty(_)));
                let n = e.name();
                let n = n.as_ref();
                match n {
                    b"w:docDefaults" if !empty => in_defaults = true,
                    b"w:style" if !empty => {
                        let id = attr(e, b"w:styleId").unwrap_or_default();
                        let paragraph = attr(e, b"w:type").as_deref() != Some("character");
                        let table_or_list = matches!(attr(e, b"w:type").as_deref(), Some("table") | Some("numbering"));
                        cur = if table_or_list { None } else { Some((id, DStyle { paragraph, ..Default::default() })) };
                        num = None;
                    }
                    b"w:name" => {
                        if let Some((_, s)) = cur.as_mut() {
                            s.name = val(e).unwrap_or_default();
                        }
                    }
                    b"w:basedOn" => {
                        if let Some((_, s)) = cur.as_mut() {
                            s.based = val(e);
                        }
                    }
                    b"w:next" => {
                        if let Some((_, s)) = cur.as_mut() {
                            s.next = val(e);
                        }
                    }
                    b"w:qFormat" => {
                        if let Some((_, s)) = cur.as_mut() {
                            s.quick = true;
                        }
                    }
                    b"w:pPr" if !empty => in_ppr = true,
                    b"w:rPr" if !empty => in_rpr = true,
                    b"w:outlineLvl" if in_ppr => {
                        if let Some((_, s)) = cur.as_mut() {
                            s.outline = val(e).and_then(|v| v.parse::<u8>().ok()).filter(|l| *l < 9).map(|l| l + 1);
                        }
                    }
                    _ if in_rpr => {
                        if let Some((_, s)) = cur.as_mut() {
                            char_prop(n, e, &mut s.chars, th);
                        } else if in_defaults {
                            char_prop(n, e, &mut def_c, th);
                        }
                    }
                    _ if in_ppr => {
                        if let Some((_, s)) = cur.as_mut() {
                            para_prop(n, e, &mut s.para, &mut num);
                        } else if in_defaults {
                            para_prop(n, e, &mut def_p, &mut num);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"w:docDefaults" => in_defaults = false,
                b"w:pPr" => {
                    in_ppr = false;
                    if let Some((_, s)) = cur.as_mut() {
                        apply_num(&mut s.para, num.take(), nums);
                        // Wcięcie z listy w stylu zostawiamy składowi.
                    }
                }
                b"w:rPr" => in_rpr = false,
                b"w:style" => {
                    if let Some((id, s)) = cur.take() {
                        out.insert(id, s);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    (out, def_p, def_c)
}

/// Pełne formatowanie stylu (z łańcuchem basedOn); akapitowe zaczynają od docDefaults.
fn resolve(id: &str, styles: &HashMap<String, DStyle>, def_p: &ParaFmt, def_c: &CharFmt, depth: usize) -> (ParaFmt, CharFmt) {
    let Some(s) = styles.get(id) else { return (def_p.clone(), def_c.clone()) };
    let (mut p, mut c) = match s.based.as_deref() {
        Some(b) if depth < 16 && styles.contains_key(b) => resolve(b, styles, def_p, def_c, depth + 1),
        _ if s.paragraph => (def_p.clone(), def_c.clone()),
        _ => (ParaFmt::default(), CharFmt::default()),
    };
    p.merge(&s.para);
    c.merge(&s.chars);
    (p, c)
}

/// Wspólne dane do czytania treści: łącza, listy, czcionki motywu i style.
struct Ctx<'a> {
    rels: &'a HashMap<String, String>,
    nums: &'a HashMap<String, Vec<ListKind>>,
    theme: &'a ThemeFonts,
    /// Identyfikator stylu akapitu z Worda → nasz.
    pstyles: &'a HashMap<String, String>,
    /// Styl znaku z Worda → pełne formatowanie.
    cstyles: &'a HashMap<String, CharFmt>,
}

/// Stan akapitu odłożony na czas pola tekstowego, które w nim stoi.
struct Outer {
    para: Option<Paragraph>,
    run_fmt: CharFmt,
    in_ppr: bool,
    in_rpr: bool,
    in_run: bool,
    in_text: bool,
    link: Option<String>,
    num: Option<(String, u8)>,
    /// Ile akapitów pól tekstowych było przed tym polem (jego akapity idą do kształtu).
    boxes_mark: usize,
}

/// Przekształcenie grupy kształtów (EMU): położenie i rozmiar grupy oraz układ jej dzieci.
#[derive(Clone, Copy, Default)]
struct Xf {
    off: (f32, f32),
    ext: (f32, f32),
    ch_off: (f32, f32),
    ch_ext: (f32, f32),
}

/// Kształt (albo obraz) w rysunku Worda.
#[derive(Default)]
struct Shp {
    prst: String,
    /// (x, y, szer., wys.) w EMU, w układzie najbliższej grupy.
    xf: Option<(f32, f32, f32, f32)>,
    flip: (bool, bool),
    fill: Option<Option<Color>>,
    line: Option<Option<Color>>,
    line_w: Option<f32>,
    dash: bool,
    style_fill: Option<Color>,
    style_line: Option<Color>,
    /// Marginesy tekstu (pt): lewy, górny, prawy, dolny.
    ins: [f32; 4],
    valign: VAlign,
    text: Vec<Paragraph>,
    image: Option<String>,
    groups: Vec<Xf>,
}

/// Rysunek (wp:anchor albo wp:inline) – położenie na stronie i jego kształty.
#[derive(Default)]
struct Drw {
    anchor: bool,
    h_rel: String,
    h_off: f32,
    h_align: Option<String>,
    v_rel: String,
    v_off: f32,
    v_align: Option<String>,
    w: f32,
    h: f32,
    wrap: WrapMode,
    behind: bool,
    dist: f32,
    shapes: Vec<Shp>,
    para: usize,
}

/// Wiersz tabeli w budowie: akapit z tabulatorami, bieżąca kolumna siatki i rozpiętość komórki.
struct Row {
    p: Paragraph,
    col: usize,
    span: usize,
    first: bool,
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
    rows: Vec<Row>,
    grids: Vec<Vec<f32>>,
    field_instr: String,
    /// Akapity pól tekstowych czekające na koniec akapitu (albo wiersza), w którym stoją.
    boxes: Vec<Paragraph>,
    outer: Vec<Outer>,
    /// Głębokość `mc:Fallback` – zapasowe kopie pomijamy.
    fallback: usize,
    drawings: Vec<Drw>,
    drw: Option<Drw>,
    shp: Option<Shp>,
    groups: Vec<Xf>,
    /// Przekształcenie grupy w trakcie czytania jej `grpSpPr`.
    grp_xf: Option<Xf>,
    in_sppr: bool,
    in_ln: bool,
    in_fill: bool,
    in_grad: bool,
    /// Odwołanie stylu kształtu: 1 – obrys (lnRef), 2 – wypełnienie (fillRef).
    style_ref: u8,
    /// Kolor w budowie (z modyfikatorami jasności).
    clr: Option<Color>,
    /// Czytany tekst położenia: 1 – posOffset, 2 – align; oś: 1 – pozioma, 2 – pionowa.
    pos_text: u8,
    pos_axis: u8,
    /// Rozmiar kształtu VML (style="width:…;height:…") dla obrazów `v:imagedata`.
    vml_size: (f32, f32),
    /// Rysunki i kształty zagnieżdżone (np. obraz w polu tekstowym kształtu) – pomijamy je,
    /// żeby nie nadpisały zewnętrznych.
    drw_nested: usize,
    shp_nested: usize,
    /// Przypisy: (identyfikator z Worda, końcowy).
    note_refs: Vec<(String, bool)>,
}

fn emu(v: Option<String>) -> f32 {
    v.and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0) / 12700.0
}

/// Kolor DrawingML: srgbClr, sysClr albo kolor motywu (schemeClr).
fn base_color(e: &BytesStart, th: &ThemeFonts) -> Option<Color> {
    match e.name().as_ref() {
        b"a:srgbClr" => attr(e, b"val").and_then(|v| Color::parse(&v)),
        b"a:sysClr" => attr(e, b"lastClr").and_then(|v| Color::parse(&v)),
        b"a:schemeClr" => {
            let v = attr(e, b"val").unwrap_or_default();
            let k = match v.as_str() {
                "tx1" => "dk1",
                "bg1" => "lt1",
                "tx2" => "dk2",
                "bg2" => "lt2",
                o => o,
            };
            th.colors.get(k).copied().or(match k {
                "dk1" => Some(Color::rgb(0, 0, 0)),
                "lt1" => Some(Color::WHITE),
                "accent1" => Some(Color::rgb(0x44, 0x72, 0xc4)),
                _ => None,
            })
        }
        _ => None,
    }
}

/// Modyfikator koloru (lumMod, lumOff, shade, tint) – w przybliżeniu na składowych RGB.
fn modify(c: Color, e: &BytesStart) -> Color {
    let v = attr(e, b"val").and_then(|v| v.parse::<f32>().ok()).unwrap_or(100000.0) / 100000.0;
    let name = e.name();
    let f = |x: u8| -> u8 {
        let x = x as f32;
        let y = match name.as_ref() {
            b"a:lumMod" | b"a:shade" => x * v,
            b"a:lumOff" => x + 255.0 * v,
            b"a:tint" => x + (255.0 - x) * (1.0 - v),
            _ => x,
        };
        y.round().clamp(0.0, 255.0) as u8
    };
    Color { r: f(c.r), g: f(c.g), b: f(c.b), a: c.a }
}

/// Gotowy kształt Worda → nasz wzór.
fn preset_of(prst: &str) -> &'static str {
    match prst {
        "roundRect" | "snipRoundRect" | "round2SameRect" | "flowChartAlternateProcess" => "rounded",
        "ellipse" | "flowChartConnector" | "cloud" | "donut" => "ellipse",
        "triangle" | "rtTriangle" | "flowChartExtract" => "triangle",
        "diamond" | "flowChartDecision" => "diamond",
        "pentagon" | "homePlate" => "pentagon",
        "hexagon" | "flowChartPreparation" => "hexagon",
        "octagon" => "octagon",
        "star5" | "star4" | "star6" | "star7" => "star",
        "star8" | "star10" | "star12" | "star16" | "star24" | "star32" => "star8",
        "heart" => "heart",
        "wave" | "doubleWave" => "wave",
        "parallelogram" | "flowChartInputOutput" => "parallelogram",
        "trapezoid" | "flowChartManualOperation" => "trapezoid",
        "rightArrow" | "notchedRightArrow" | "stripedRightArrow" => "arrow",
        "chevron" => "chevron",
        "plus" | "mathPlus" => "cross",
        "wedgeRectCallout" | "wedgeRoundRectCallout" | "wedgeEllipseCallout" | "cloudCallout" => "speech",
        _ => "rect",
    }
}

type Parsed = (Vec<Paragraph>, Vec<(usize, String, f32, f32)>, Vec<Drw>, Vec<(String, bool)>);

fn parse_story(xml: &str, ctx: &Ctx, page: Option<&mut PageSetup>) -> Parsed {
    let mut r = Reader::from_str(xml);
    let mut st = State::default();
    let mut page = page;
    // Pierwsza sekcja wyznacza stronę dokumentu (kolejne sectPr jej nie nadpisują).
    let mut page_done = false;
    let mut cx = 0.0f32;
    let mut cy = 0.0f32;
    loop {
        let ev = r.read_event();
        // Zapasowe kopie (VML) tych samych rysunków i pól tekstowych – pomijamy w całości.
        if st.fallback > 0 {
            match &ev {
                Ok(Event::Start(e)) if e.name().as_ref() == b"mc:Fallback" => st.fallback += 1,
                Ok(Event::End(e)) if e.name().as_ref() == b"mc:Fallback" => st.fallback -= 1,
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
            continue;
        }
        match ev {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let empty = matches!(ev, Ok(Event::Empty(_)));
                let name = e.name();
                let n = name.as_ref();
                match n {
                    b"mc:Fallback" if !empty => st.fallback = 1,
                    b"wp:anchor" | b"wp:inline" if !empty && st.drw.is_some() => st.drw_nested += 1,
                    b"wp:anchor" | b"wp:inline" if !empty => {
                        st.drw = Some(Drw {
                            anchor: n == b"wp:anchor",
                            behind: attr(e, b"behindDoc").is_some_and(|v| v == "1" || v == "true"),
                            dist: emu(attr(e, b"distL")).max(emu(attr(e, b"distT"))),
                            wrap: WrapMode::TopBottom,
                            ..Default::default()
                        });
                    }
                    b"wp:positionH" if !empty => {
                        st.pos_axis = 1;
                        if let Some(d) = st.drw.as_mut() {
                            d.h_rel = attr(e, b"relativeFrom").unwrap_or_default();
                        }
                    }
                    b"wp:positionV" if !empty => {
                        st.pos_axis = 2;
                        if let Some(d) = st.drw.as_mut() {
                            d.v_rel = attr(e, b"relativeFrom").unwrap_or_default();
                        }
                    }
                    b"wp:posOffset" if !empty => st.pos_text = 1,
                    b"wp:align" if !empty => st.pos_text = 2,
                    b"wp:wrapNone" | b"wp:wrapSquare" | b"wp:wrapTight" | b"wp:wrapThrough" | b"wp:wrapTopAndBottom" => {
                        if let Some(d) = st.drw.as_mut() {
                            d.wrap = match n {
                                b"wp:wrapNone" => WrapMode::None,
                                b"wp:wrapTopAndBottom" => WrapMode::TopBottom,
                                _ => WrapMode::Both,
                            };
                        }
                    }
                    b"wps:wsp" | b"pic:pic" if !empty && st.shp.is_some() => st.shp_nested += 1,
                    b"wps:wsp" | b"pic:pic" if !empty && st.drw.is_some() && st.drw_nested == 0 => {
                        st.shp = Some(Shp { ins: [7.2, 3.6, 7.2, 3.6], groups: st.groups.clone(), ..Default::default() });
                    }
                    b"wpg:grpSpPr" if !empty => st.grp_xf = Some(Xf::default()),
                    b"wps:spPr" | b"pic:spPr" if !empty => st.in_sppr = true,
                    b"a:xfrm" if st.shp.is_some() && st.in_sppr => {
                        if let Some(s) = st.shp.as_mut() {
                            s.flip = (attr(e, b"flipH").is_some_and(|v| v == "1"), attr(e, b"flipV").is_some_and(|v| v == "1"));
                        }
                    }
                    b"a:off" | b"a:ext" | b"a:chOff" | b"a:chExt" if st.grp_xf.is_some() || (st.shp.is_some() && st.in_sppr) => {
                        let (a, b) = if n == b"a:off" || n == b"a:chOff" { (b"x" as &[u8], b"y" as &[u8]) } else { (b"cx" as &[u8], b"cy" as &[u8]) };
                        if let (Some(x), Some(y)) = (attr(e, a).and_then(|v| v.parse::<f32>().ok()), attr(e, b).and_then(|v| v.parse::<f32>().ok())) {
                            if let Some(g) = st.grp_xf.as_mut() {
                                match n {
                                    b"a:off" => g.off = (x, y),
                                    b"a:ext" => g.ext = (x, y),
                                    b"a:chOff" => g.ch_off = (x, y),
                                    _ => g.ch_ext = (x, y),
                                }
                            } else if let Some(s) = st.shp.as_mut() {
                                let mut xf = s.xf.unwrap_or_default();
                                if n == b"a:off" {
                                    xf.0 = x;
                                    xf.1 = y;
                                } else if n == b"a:ext" {
                                    xf.2 = x;
                                    xf.3 = y;
                                }
                                s.xf = Some(xf);
                            }
                        }
                    }
                    b"a:prstGeom" if st.in_sppr => {
                        if let Some(s) = st.shp.as_mut() {
                            s.prst = attr(e, b"prst").unwrap_or_default();
                        }
                    }
                    b"wps:cNvCnPr" => {
                        if let Some(s) = st.shp.as_mut() {
                            s.prst = "line".into();
                        }
                    }
                    b"a:ln" if st.in_sppr => {
                        st.in_ln = !empty;
                        if let Some(s) = st.shp.as_mut() {
                            if let Some(w) = attr(e, b"w").and_then(|v| v.parse::<f32>().ok()) {
                                s.line_w = Some(w / 12700.0);
                            }
                        }
                    }
                    b"a:noFill" if st.in_sppr => {
                        if let Some(s) = st.shp.as_mut() {
                            if st.in_ln {
                                s.line = Some(None);
                            } else if !st.in_grad {
                                s.fill = Some(None);
                            }
                        }
                    }
                    b"a:solidFill" if !empty => st.in_fill = true,
                    b"a:gradFill" if !empty => st.in_grad = true,
                    b"a:prstDash" if st.in_ln => {
                        if let Some(s) = st.shp.as_mut() {
                            s.dash = attr(e, b"val").is_some_and(|v| v != "solid");
                        }
                    }
                    b"a:lnRef" | b"a:fillRef" if !empty && st.shp.is_some() => {
                        let idx = attr(e, b"idx").and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
                        st.style_ref = if idx == 0 { 0 } else if n == b"a:lnRef" { 1 } else { 2 };
                    }
                    b"a:srgbClr" | b"a:schemeClr" | b"a:sysClr" if st.shp.is_some() => {
                        st.clr = base_color(e, ctx.theme);
                        if empty {
                            commit_color(&mut st);
                        }
                    }
                    b"a:lumMod" | b"a:lumOff" | b"a:shade" | b"a:tint" if st.clr.is_some() => {
                        st.clr = st.clr.map(|c| modify(c, e));
                    }
                    b"wps:bodyPr" => {
                        if let Some(s) = st.shp.as_mut() {
                            for (k, a) in [b"lIns" as &[u8], b"tIns", b"rIns", b"bIns"].iter().enumerate() {
                                if let Some(v) = attr(e, a).and_then(|v| v.parse::<f32>().ok()) {
                                    s.ins[k] = v / 12700.0;
                                }
                            }
                            s.valign = match attr(e, b"anchor").as_deref() {
                                Some("ctr") => VAlign::Center,
                                Some("b") => VAlign::Bottom,
                                _ => VAlign::Top,
                            };
                        }
                    }
                    b"v:shape" | b"v:rect" | b"v:roundrect" => {
                        let style = attr(e, b"style").unwrap_or_default();
                        let get = |k: &str| style.split(';').find_map(|d| {
                            let (a, b) = d.split_once(':')?;
                            (a.trim() == k).then(|| b.trim().trim_end_matches("pt").parse::<f32>().ok()).flatten()
                        });
                        st.vml_size = (get("width").unwrap_or(0.0), get("height").unwrap_or(0.0));
                    }
                    b"w:txbxContent" if !empty => {
                        st.outer.push(Outer {
                            para: st.para.take(),
                            run_fmt: std::mem::take(&mut st.run_fmt),
                            in_ppr: st.in_ppr,
                            in_rpr: st.in_rpr,
                            in_run: st.in_run,
                            in_text: st.in_text,
                            link: st.link.take(),
                            num: st.num.take(),
                            boxes_mark: st.boxes.len(),
                        });
                        st.in_ppr = false;
                        st.in_rpr = false;
                        st.in_run = false;
                        st.in_text = false;
                    }
                    b"w:tbl" if !empty => st.grids.push(Vec::new()),
                    b"w:gridCol" => {
                        if let (Some(g), Some(w)) = (st.grids.last_mut(), twips(attr(e, b"w:w"))) {
                            g.push(w);
                        }
                    }
                    b"w:tr" if !empty => st.rows.push(Row { p: Paragraph { runs: Vec::new(), ..Default::default() }, col: 0, span: 1, first: true }),
                    b"w:tc" if !empty => {
                        let grid = st.grids.last().cloned().unwrap_or_default();
                        if let Some(row) = st.rows.last_mut() {
                            if !row.first {
                                // Tabulator na lewej krawędzi tej kolumny siatki.
                                let x: f32 = grid.iter().take(row.col).sum();
                                if x > 0.0 {
                                    row.p.fmt.tabs.get_or_insert_with(Vec::new).push(x);
                                }
                                row.p.runs.push(Run { text: "\t".into(), ..Default::default() });
                            }
                            row.first = false;
                            row.span = 1;
                        }
                    }
                    b"w:gridSpan" => {
                        if let Some(row) = st.rows.last_mut() {
                            row.span = val(e).and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
                        }
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
                    b"w:hyperlink" if !empty => st.link = attr(e, b"r:id").and_then(|id| ctx.rels.get(&id).cloned()),
                    b"w:t" if !empty => st.in_text = true,
                    b"w:instrText" if !empty => st.in_text = false,
                    b"w:fldSimple" => st.field_instr = attr(e, b"w:instr").unwrap_or_default(),
                    b"w:tab" if st.in_run && !st.in_ppr => push_text(&mut st, "\t"),
                    b"w:noBreakHyphen" if st.in_run => push_text(&mut st, "\u{2011}"),
                    b"w:footnoteReference" | b"w:endnoteReference" if st.in_run => {
                        // Tymczasowy numer przypisu – po wczytaniu treści przypisów dostaje właściwy.
                        let end = n == b"w:endnoteReference";
                        let wid = attr(e, b"w:id").unwrap_or_default();
                        st.note_refs.push((wid, end));
                        let tmp = st.note_refs.len() as u64;
                        let mut fmt = st.run_fmt.clone();
                        fmt.script = Some(Script::Super);
                        if let Some(p) = st.para.as_mut() {
                            p.runs.push(Run { text: FIELD_CHAR.to_string(), fmt, field: Some(Field::Footnote(u64::MAX - tmp)) });
                        }
                    }
                    b"w:softHyphen" if st.in_run => push_text(&mut st, "\u{ad}"),
                    b"w:br" if st.in_run => match attr(e, b"w:type").as_deref() {
                        Some("page") => st.pending_page_break = true,
                        Some("column") => {}
                        _ => push_text(&mut st, "\n"),
                    },
                    b"w:pStyle" if st.in_ppr => {
                        if let (Some(p), Some(v)) = (st.para.as_mut(), val(e)) {
                            p.style = ctx.pstyles.get(&v).cloned().unwrap_or_else(|| our_style(&v));
                        }
                    }
                    b"w:rStyle" if st.in_rpr && !st.in_ppr => {
                        // Styl znaku jako podstawa – właściwości bezpośrednie (dalej w rPr) go nadpiszą.
                        if let Some(base) = val(e).and_then(|v| ctx.cstyles.get(&v)) {
                            let link = st.run_fmt.link.take();
                            st.run_fmt = base.clone();
                            st.run_fmt.link = link;
                        }
                    }
                    // Formatowanie znaku akapitu (rPr w pPr) dotyczy tylko znaku ¶ – pomijamy.
                    _ if st.in_rpr && st.in_ppr => {}
                    _ if st.in_rpr => {
                        char_prop(n, e, &mut st.run_fmt, ctx.theme);
                    }
                    _ if st.in_ppr && n == b"w:sectPr" => {}
                    _ if st.in_ppr && !matches!(n, b"w:pgSz" | b"w:pgMar" | b"w:cols" | b"w:titlePg") => {
                        if let Some(p) = st.para.as_mut() {
                            para_prop(n, e, &mut p.fmt, &mut st.num);
                        }
                    }
                    b"wp:extent" => {
                        cx = attr(e, b"cx").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0) / 12700.0;
                        cy = attr(e, b"cy").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0) / 12700.0;
                        if let (Some(d), 0) = (st.drw.as_mut(), st.drw_nested) {
                            d.w = cx;
                            d.h = cy;
                        }
                    }
                    b"a:blip" => {
                        if let Some(target) = attr(e, b"r:embed").and_then(|id| ctx.rels.get(&id).cloned()) {
                            match st.shp.as_mut() {
                                Some(s) => s.image = Some(target),
                                None => {
                                    let idx = st.paras.len();
                                    st.images.push((idx, target, cx, cy));
                                }
                            }
                        }
                    }
                    // Obraz tylko w VML (starsze pliki, bez wersji DrawingML).
                    b"v:imagedata" => {
                        if let Some(target) = attr(e, b"r:id").and_then(|id| ctx.rels.get(&id).cloned()) {
                            let idx = st.paras.len();
                            st.images.push((idx, target, st.vml_size.0, st.vml_size.1));
                        }
                    }
                    b"w:pgSz" if !page_done => {
                        if let Some(ps) = page.as_deref_mut() {
                            if let Some(w) = twips(attr(e, b"w:w")) {
                                ps.width = w;
                            }
                            if let Some(h) = twips(attr(e, b"w:h")) {
                                ps.height = h;
                            }
                        }
                    }
                    b"w:pgMar" if !page_done => {
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
                    b"w:cols" if !page_done => {
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
                    b"w:titlePg" if !page_done => {
                        if let Some(ps) = page.as_deref_mut() {
                            ps.different_first = on(e);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) if st.pos_text > 0 => {
                let s = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                if let Some(d) = st.drw.as_mut() {
                    match (st.pos_text, st.pos_axis) {
                        (1, 1) => d.h_off = s.trim().parse::<f32>().unwrap_or(0.0) / 12700.0,
                        (1, _) => d.v_off = s.trim().parse::<f32>().unwrap_or(0.0) / 12700.0,
                        (_, 1) => d.h_align = Some(s.trim().to_string()),
                        _ => d.v_align = Some(s.trim().to_string()),
                    }
                }
            }
            Ok(Event::Text(t)) if st.in_text => {
                let s = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                push_text(&mut st, &s);
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"w:t" => st.in_text = false,
                b"wp:posOffset" | b"wp:align" => st.pos_text = 0,
                b"wpg:grpSpPr" => {
                    if let Some(g) = st.grp_xf.take() {
                        st.groups.push(g);
                    }
                }
                b"wpg:grpSp" | b"wpg:wgp" => {
                    st.groups.pop();
                }
                b"wps:spPr" | b"pic:spPr" => st.in_sppr = false,
                b"a:ln" => st.in_ln = false,
                b"a:solidFill" => st.in_fill = false,
                b"a:gradFill" => st.in_grad = false,
                b"a:lnRef" | b"a:fillRef" => st.style_ref = 0,
                b"a:srgbClr" | b"a:schemeClr" | b"a:sysClr" => commit_color(&mut st),
                b"wps:wsp" | b"pic:pic" if st.shp_nested > 0 => st.shp_nested -= 1,
                b"wps:wsp" | b"pic:pic" => {
                    if let (Some(s), Some(d)) = (st.shp.take(), st.drw.as_mut()) {
                        d.shapes.push(s);
                    }
                }
                b"wp:anchor" | b"wp:inline" if st.drw_nested > 0 => st.drw_nested -= 1,
                b"wp:anchor" | b"wp:inline" => {
                    if let Some(mut d) = st.drw.take() {
                        d.para = st.paras.len();
                        st.groups.clear();
                        st.drawings.push(d);
                    }
                }
                b"w:pPr" => {
                    st.in_ppr = false;
                    let num = st.num.take();
                    if let Some(p) = st.para.as_mut() {
                        apply_num(&mut p.fmt, num, ctx.nums);
                    }
                }
                b"w:sectPr" => page_done = page.is_some(),
                b"w:rPr" => st.in_rpr = false,
                b"w:r" => st.in_run = false,
                b"w:hyperlink" => st.link = None,
                b"w:fldSimple" => st.field_instr.clear(),
                b"w:txbxContent" => {
                    if let Some(o) = st.outer.pop() {
                        // Pole tekstowe kształtu – jego akapity należą do kształtu, nie do treści.
                        if let Some(shp) = st.shp.as_mut() {
                            let mark = o.boxes_mark.min(st.boxes.len());
                            shp.text.extend(st.boxes.drain(mark..));
                        }
                        st.para = o.para;
                        st.run_fmt = o.run_fmt;
                        st.in_ppr = o.in_ppr;
                        st.in_rpr = o.in_rpr;
                        st.in_run = o.in_run;
                        st.in_text = o.in_text;
                        st.link = o.link;
                        st.num = o.num;
                    }
                }
                b"w:p" => {
                    if let Some(mut p) = st.para.take() {
                        if p.runs.is_empty() {
                            p.runs.push(Run::default());
                        }
                        p.normalize();
                        if !st.outer.is_empty() {
                            // Akapit pola tekstowego – z lekkim tłem, za akapitem, w którym stoi pole
                            // (pole w kształcie trafia do kształtu, bez tła).
                            if p.fmt.shading.is_none() && st.shp.is_none() {
                                p.fmt.shading = Some(Color::rgb(0xf3, 0xf5, 0xf8));
                            }
                            st.boxes.push(p);
                        } else if let Some(row) = st.rows.last_mut() {
                            if !row.p.runs.iter().all(|r| r.text.is_empty()) && !row.p.runs.last().is_some_and(|r| r.text.ends_with('\t')) {
                                row.p.runs.push(Run { text: " ".into(), ..Default::default() });
                            }
                            if row.p.runs.is_empty() {
                                row.p.style = p.style.clone();
                                row.p.fmt.align = p.fmt.align;
                            }
                            row.p.runs.extend(p.runs.into_iter().filter(|r| !r.text.is_empty()));
                        } else {
                            st.paras.push(p);
                            st.paras.append(&mut st.boxes);
                        }
                    }
                }
                b"w:tc" => {
                    if let Some(row) = st.rows.last_mut() {
                        row.col += row.span;
                    }
                }
                b"w:tr" => {
                    if let Some(mut row) = st.rows.pop() {
                        row.p.normalize();
                        match st.rows.last_mut() {
                            // Tabela w komórce – jej wiersz dopisujemy do komórki zewnętrznej.
                            Some(parent) => {
                                parent.p.runs.push(Run { text: " ".into(), ..Default::default() });
                                parent.p.runs.extend(row.p.runs.into_iter().map(|mut r| {
                                    r.text = r.text.replace('\t', " ");
                                    r
                                }));
                            }
                            None => {
                                if row.p.runs.is_empty() {
                                    row.p.runs.push(Run::default());
                                }
                                st.paras.push(row.p);
                                st.paras.append(&mut st.boxes);
                            }
                        }
                    }
                }
                b"w:tbl" => {
                    st.grids.pop();
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    // Pola tekstowe poza akapitem (nie powinno się zdarzyć) – na końcu, żeby nic nie zginęło.
    st.paras.append(&mut st.boxes);
    (st.paras, st.images, st.drawings, st.note_refs)
}

/// Gotowy kolor trafia tam, gdzie go czytamy: wypełnienie, obrys albo styl kształtu.
fn commit_color(st: &mut State) {
    let Some(c) = st.clr.take() else { return };
    let (in_sppr, in_ln, in_fill, in_grad, sref) = (st.in_sppr, st.in_ln, st.in_fill, st.in_grad, st.style_ref);
    let Some(s) = st.shp.as_mut() else { return };
    if in_sppr && in_ln && in_fill {
        s.line = Some(Some(c));
    } else if in_sppr && !in_ln && (in_fill || (in_grad && s.fill.is_none())) {
        s.fill = Some(Some(c));
    } else if sref == 1 {
        s.style_line = Some(c);
    } else if sref == 2 {
        s.style_fill = Some(c);
    }
}

fn push_text(st: &mut State, s: &str) {
    let Some(p) = st.para.as_mut() else { return };
    let field = match st.field_instr.split_whitespace().next() {
        Some("PAGE") => Some(Field::PageNumber),
        Some("NUMPAGES") => Some(Field::PageCount),
        Some("DATE") => Some(Field::Date),
        Some("TITLE") => Some(Field::Title),
        Some("AUTHOR") => Some(Field::Author),
        Some("SUBJECT") => Some(Field::Meta(MetaKey::Subject)),
        Some("KEYWORDS") => Some(Field::Meta(MetaKey::Keywords)),
        Some("CREATEDATE") => Some(Field::Meta(MetaKey::Created)),
        Some("SAVEDATE") => Some(Field::Meta(MetaKey::Modified)),
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

/// Style Worda do dokumentu: wbudowane (Normal, nagłówki…) uzupełniają nasze, własne dochodzą
/// jako nowe („dx-…”). Zwraca mapy stylów akapitu i znaku do czytania treści.
fn apply_styles(doc: &mut Document, styles: &HashMap<String, DStyle>, def_p: &ParaFmt, def_c: &CharFmt) -> (HashMap<String, String>, HashMap<String, CharFmt>) {
    let mut pmap = HashMap::new();
    let mut cmap = HashMap::new();
    let mut ids: Vec<&String> = styles.keys().collect();
    ids.sort();
    for id in &ids {
        let s = &styles[*id];
        if !s.paragraph {
            let (_, c) = resolve(id, styles, def_p, def_c, 0);
            cmap.insert((*id).clone(), c);
            continue;
        }
        let ours = builtin_by_name(&s.name).map(String::from).unwrap_or_else(|| format!("dx-{id}"));
        pmap.insert((*id).clone(), ours);
    }
    for id in ids {
        let s = &styles[id];
        if !s.paragraph {
            continue;
        }
        let ours = pmap[id].clone();
        let (p, c) = resolve(id, styles, def_p, def_c, 0);
        let outline = s.outline.or_else(|| {
            let mut cur = s.based.as_deref();
            let mut n = 0;
            while let (Some(b), true) = (cur, n < 16) {
                if let Some(o) = styles.get(b).and_then(|x| x.outline) {
                    return Some(o);
                }
                cur = styles.get(b).and_then(|x| x.based.as_deref());
                n += 1;
            }
            None
        });
        match doc.styles.iter_mut().find(|x| x.id == ours) {
            Some(st) => {
                // Wbudowany: pełne formatowanie z Worda (dziedziczenie już policzone).
                st.para.merge(&p);
                st.chars.merge(&c);
                if outline.is_some() {
                    st.outline = outline;
                }
            }
            None => {
                let next = s.next.as_ref().and_then(|n| pmap.get(n)).cloned();
                doc.styles.push(Style { id: ours, name: s.name.clone(), based_on: Some("normal".into()), next, para: p, chars: c, outline, gallery: s.quick });
            }
        }
    }
    // Lista ze stylu „Normal” nie przechodzi na nasze style, które w Wordzie jej nie mają.
    let normal_list = doc.style("normal").is_some_and(|s| matches!(s.para.list, Some(Some(_))));
    if normal_list {
        for st in doc.styles.iter_mut().filter(|s| s.id != "normal" && s.para.list.is_none()) {
            st.para.list = Some(None);
        }
    }
    (pmap, cmap)
}

/// Rysunki Worda → obiekty. Położenie względem akapitu – obiekt zakotwiczony (jedzie z tekstem),
/// względem strony albo marginesu – stały na stronie akapitu (ze składu).
fn add_drawings(doc: &mut Document, z: &Zip, drawings: Vec<Drw>) {
    if drawings.is_empty() {
        return;
    }
    let ps = doc.page.clone();
    let cw = ps.content_width();
    let n_paras = doc.body.paras.len();
    // Strona akapitów – jeden skład (tylko jeśli są rysunki przypięte do strony).
    let needs_pages = drawings.iter().any(|d| d.anchor && !matches!(d.v_rel.as_str(), "paragraph" | "line" | ""));
    let pages: Vec<usize> = if needs_pages {
        let lay = crate::layout::Engine::new().layout(doc);
        let mut v = vec![0; n_paras];
        for &i in lay.story_lines(StoryRef::Body) {
            let l = &lay.lines[i];
            if l.para < n_paras && (l.para == 0 || v[l.para] == 0) {
                v[l.para] = l.page;
            }
        }
        v
    } else {
        Vec::new()
    };
    for d in drawings {
        let pi = d.para.min(n_paras.saturating_sub(1));
        // Lewy górny róg rysunku: poziomo zawsze bezwzględnie, pionowo – od akapitu albo od strony.
        let x0 = if !d.anchor {
            ps.margin_left
        } else {
            let (base, span) = match d.h_rel.as_str() {
                "page" => (0.0, ps.width),
                "leftMargin" | "insideMargin" => (0.0, ps.margin_left),
                "rightMargin" | "outsideMargin" => (ps.width - ps.margin_right, ps.margin_right),
                _ => (ps.margin_left, cw),
            };
            match d.h_align.as_deref() {
                Some("center") => base + (span - d.w) / 2.0,
                Some("right") | Some("outside") => base + span - d.w,
                Some(_) => base,
                None => base + d.h_off,
            }
        };
        let rel_para = !d.anchor || matches!(d.v_rel.as_str(), "paragraph" | "line" | "");
        let y0 = if rel_para {
            d.v_off
        } else {
            let (base, span) = match d.v_rel.as_str() {
                "page" => (0.0, ps.height),
                "topMargin" => (0.0, ps.margin_top),
                "bottomMargin" => (ps.height - ps.margin_bottom, ps.margin_bottom),
                _ => (ps.margin_top, ps.height - ps.margin_top - ps.margin_bottom),
            };
            match d.v_align.as_deref() {
                Some("center") => base + (span - d.h) / 2.0,
                Some("bottom") | Some("outside") => base + span - d.h,
                Some(_) => base,
                None => base + d.v_off,
            }
        };
        let page = pages.get(pi).copied().unwrap_or(0);
        let wrap = if d.behind { WrapMode::Behind } else { d.wrap };
        // Układ grupy: współrzędne dzieci → punkty względem rogu rysunku.
        for s in d.shapes {
            let (mut x, mut y, mut w, mut h) = match s.xf {
                Some((ox, oy, ew, eh)) if !s.groups.is_empty() || ew > 0.0 => (ox, oy, ew, eh),
                _ => (0.0, 0.0, d.w * 12700.0, d.h * 12700.0),
            };
            for g in s.groups.iter().rev() {
                let kx = if g.ch_ext.0 > 0.0 { g.ext.0 / g.ch_ext.0 } else { 1.0 };
                let ky = if g.ch_ext.1 > 0.0 { g.ext.1 / g.ch_ext.1 } else { 1.0 };
                x = g.off.0 + (x - g.ch_off.0) * kx;
                y = g.off.1 + (y - g.ch_off.1) * ky;
                w *= kx;
                h *= ky;
            }
            if let Some(top) = s.groups.first() {
                x -= top.off.0;
                y -= top.off.1;
            } else {
                x = 0.0;
                y = 0.0;
            }
            let (x, y, w, h) = (x / 12700.0, y / 12700.0, (w / 12700.0).max(0.5), (h / 12700.0).max(0.5));
            let is_line = s.prst == "line" || s.prst.contains("Connector");
            let shape = if is_line {
                let (a, b) = if w < 1.0 {
                    ((0.5, 0.0), (0.5, 1.0))
                } else if h < 1.0 {
                    ((0.0, 0.5), (1.0, 0.5))
                } else if s.flip.0 != s.flip.1 {
                    ((0.0, 1.0), (1.0, 0.0))
                } else {
                    ((0.0, 0.0), (1.0, 1.0))
                };
                Shape { contours: vec![Contour { nodes: vec![Node::corner(a.0, a.1), Node::corner(b.0, b.1)], closed: false }] }
            } else {
                crate::geom::preset(preset_of(&s.prst)).unwrap_or_else(Shape::rect)
            };
            let fill = match s.fill {
                Some(c) => c,
                None => s.style_fill,
            };
            let line = match s.line {
                Some(c) => c,
                None => s.style_line,
            };
            let lw = s.line_w.unwrap_or(0.75).max(0.25);
            let stroke = line.map(|c| StrokeStyle { color: c, width: lw, dash: if s.dash { vec![lw * 3.0, lw * 2.0] } else { vec![] } });
            let kind = if let Some(target) = &s.image {
                let path = format!("word/{}", target.trim_start_matches('/').trim_start_matches("word/"));
                let Some(asset) = z.files.get(&path).cloned().and_then(super::odtp::asset_from) else { continue };
                let key = format!("img{}", doc.new_id());
                doc.assets.insert(key.clone(), asset);
                ObjKind::Image { asset: key, fit: ImageFit::Stretch, contour: None }
            } else if !s.text.is_empty() {
                let pad = (s.ins[0] + s.ins[1] + s.ins[2] + s.ins[3]) / 4.0;
                ObjKind::Text { story: Story { paras: s.text }, padding: pad, columns: 1, gap: 12.0, valign: s.valign, next: None }
            } else {
                if fill.is_none() && stroke.is_none() {
                    continue;
                }
                ObjKind::Shape
            };
            let is_image = matches!(kind, ObjKind::Image { .. });
            let oid = doc.new_id();
            let anchored = rel_para;
            if anchored {
                crate::edit::insert_field(&mut doc.body, crate::edit::At::new(pi, 0), Field::Anchor(oid), &CharFmt::default());
            }
            doc.objects.push(Object {
                id: oid,
                name: String::new(),
                kind,
                page,
                x: x0 + x,
                y: y0 + y,
                w,
                h,
                shape: if is_image { Shape::rect() } else { shape },
                wrap: Wrap { mode: wrap, dist: d.dist.max(2.0), contour: false },
                fill: if is_image { None } else { fill.map(|c| Fill::Solid { color: c }) },
                stroke: if is_image { None } else { stroke },
                opacity: 1.0,
                corner: if s.prst == "roundRect" { w.min(h) * 0.12 } else { 0.0 },
                shadow: false,
                locked: false,
                anchor_dy: if anchored { y0 + y } else { 4.0 },
                cont_of: None,
                repeat: Default::default(),
            });
        }
    }
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
    let theme = theme_fonts(&z);
    let (dstyles, def_p, def_c) = read_styles(&z, &theme, &nums);
    let mut doc = Document::new();
    let (pstyles, cstyles) = apply_styles(&mut doc, &dstyles, &def_p, &def_c);
    let ctx = Ctx { rels: &rel, nums: &nums, theme: &theme, pstyles: &pstyles, cstyles: &cstyles };
    let (paras, images, drawings, note_refs) = parse_story(&xml, &ctx, Some(&mut doc.page));
    if !paras.is_empty() {
        doc.body = Story { paras };
    }
    // Nagłówek i stopka (pierwsze znalezione).
    for target in rel.values() {
        let part = format!("word/{target}");
        let is_header = target.starts_with("header") && doc.header.is_none();
        let is_footer = target.starts_with("footer") && doc.footer.is_none();
        if !is_header && !is_footer {
            continue;
        }
        let Some(x) = z.text(&part) else { continue };
        let hr = rels(&z, &format!("word/_rels/{target}.rels"));
        let hctx = Ctx { rels: &hr, ..ctx };
        let (p, _, _, _) = parse_story(&x, &hctx, None);
        if !p.is_empty() {
            if is_header {
                doc.header = Some(Story { paras: p });
            } else {
                doc.footer = Some(Story { paras: p });
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
        doc.meta.keywords = grab("cp:keywords");
    }
    // Przypisy dolne i końcowe: treść z footnotes.xml / endnotes.xml, numery – nasze.
    if !note_refs.is_empty() {
        let mut texts: HashMap<(String, bool), Vec<Paragraph>> = HashMap::new();
        for (part, end) in [("word/footnotes.xml", false), ("word/endnotes.xml", true)] {
            let Some(x) = z.text(part) else { continue };
            let tag = if end { "<w:endnote " } else { "<w:footnote " };
            let close = if end { "</w:endnote>" } else { "</w:footnote>" };
            let nrel = rels(&z, &format!("word/_rels/{}.rels", part.trim_start_matches("word/")));
            let nctx = Ctx { rels: &nrel, ..ctx };
            let mut rest = x.as_str();
            while let Some(a) = rest.find(tag) {
                let Some(b) = rest[a..].find(close) else { break };
                let one = &rest[a..a + b + close.len()];
                rest = &rest[a + b + close.len()..];
                let head = &one[..one.find('>').unwrap_or(0)];
                if head.contains("w:type=") && !head.contains("w:type=\"normal\"") {
                    continue;
                }
                let id = head.split("w:id=\"").nth(1).and_then(|t| t.split('"').next()).unwrap_or("").to_string();
                let (mut ps, _, _, _) = parse_story(one, &nctx, None);
                for p in ps.iter_mut() {
                    // Pierwszy akapit zaczyna się od numeru przypisu i spacji – numer daje nasz skład.
                    if let Some(r) = p.runs.first_mut() {
                        if r.text.starts_with(' ') {
                            r.text = r.text.trim_start().to_string();
                        }
                    }
                }
                ps.retain(|p| !p.runs.iter().all(|r| r.text.trim().is_empty()));
                texts.insert((id, end), ps);
            }
        }
        let mut ids = Vec::new();
        for (wid, end) in &note_refs {
            let nid = doc.new_id();
            let mut paras = texts.remove(&(wid.clone(), *end)).unwrap_or_default();
            if paras.is_empty() {
                paras.push(Paragraph::default());
            }
            doc.notes.push(Note { id: nid, end: *end, story: Story { paras } });
            ids.push(nid);
        }
        for p in doc.body.paras.iter_mut() {
            for r in p.runs.iter_mut() {
                if let Some(Field::Footnote(t)) = r.field {
                    if t > u64::MAX - 1_000_000 {
                        let k = (u64::MAX - t) as usize;
                        if let Some(nid) = ids.get(k.wrapping_sub(1)) {
                            r.field = Some(Field::Footnote(*nid));
                        }
                    }
                }
            }
        }
    }
    // Rysunki (kształty, pola tekstowe, obrazy w grupach) – jako obiekty na stronie.
    add_drawings(&mut doc, &z, drawings);
    // Obrazy: zakotwiczone w swoim akapicie – skład sam stawia je przy tekście (bez składania tutaj).
    // Formaty, których nie umiemy pokazać (np. WMF), pomijamy.
    let cw = doc.page.content_width();
    let n_paras = doc.body.paras.len();
    for (pi, target, w, h) in images.into_iter().take(400) {
        let path = format!("word/{}", target.trim_start_matches('/').trim_start_matches("word/"));
        let Some(data) = z.files.get(&path).cloned() else { continue };
        let Some(asset) = super::odtp::asset_from(data) else { continue };
        let id = format!("img{}", doc.new_id());
        let (mut w, mut h) = if w > 1.0 && h > 1.0 { (w, h) } else { (asset.width as f32 * 0.75, asset.height as f32 * 0.75) };
        if w > cw {
            h *= cw / w;
            w = cw;
        }
        doc.assets.insert(id.clone(), asset);
        let oid = doc.new_id();
        let pi = pi.min(n_paras.saturating_sub(1));
        crate::edit::insert_field(&mut doc.body, crate::edit::At::new(pi, 0), Field::Anchor(oid), &CharFmt::default());
        doc.objects.push(Object {
            id: oid,
            name: String::new(),
            kind: ObjKind::Image { asset: id, fit: ImageFit::Stretch, contour: None },
            page: 0,
            x: doc.page.margin_left + (cw - w) / 2.0,
            y: doc.page.margin_top,
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
            repeat: Default::default(),
        });
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_parse(xml: &str) -> Vec<Paragraph> {
        let rels = HashMap::new();
        let nums = HashMap::new();
        let th = ThemeFonts::default();
        let ps = HashMap::new();
        let cs = HashMap::new();
        let ctx = Ctx { rels: &rels, nums: &nums, theme: &th, pstyles: &ps, cstyles: &cs };
        parse_story(xml, &ctx, None).0
    }

    #[test]
    fn text_box_keeps_outer_text_and_skips_fallback() {
        let xml = r#"<w:body><w:p><w:r><w:t>Przed </w:t></w:r><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wps:txbx><w:txbxContent><w:p><w:r><w:t>Pole</w:t></w:r></w:p></w:txbxContent></wps:txbx></w:drawing></mc:Choice><mc:Fallback><w:pict><v:textbox><w:txbxContent><w:p><w:r><w:t>Pole</w:t></w:r></w:p></w:txbxContent></v:textbox></w:pict></mc:Fallback></mc:AlternateContent></w:r><w:r><w:t>po</w:t></w:r></w:p></w:body>"#;
        let ps = ctx_parse(xml);
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[0].text(), "Przed po");
        assert_eq!(ps[1].text(), "Pole");
    }

    #[test]
    fn drawing_shape_with_text() {
        let xml = r#"<w:body><w:p><w:r><w:drawing><wp:anchor behindDoc="0"><wp:positionH relativeFrom="column"><wp:posOffset>127000</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>254000</wp:posOffset></wp:positionV><wp:extent cx="1270000" cy="635000"/><wp:wrapSquare/><a:graphic><a:graphicData><wpg:wgp><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/><a:chOff x="0" y="0"/><a:chExt cx="1270000" cy="635000"/></a:xfrm></wpg:grpSpPr><wps:wsp><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="ellipse"/><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill><a:ln w="25400"><a:solidFill><a:srgbClr val="0000FF"/></a:solidFill></a:ln></wps:spPr><wps:txbx><w:txbxContent><w:p><w:r><w:t>W kole</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr anchor="ctr"/></wps:wsp></wpg:wgp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r><w:r><w:t>Tekst</w:t></w:r></w:p></w:body>"#;
        let rels = HashMap::new();
        let nums = HashMap::new();
        let th = ThemeFonts::default();
        let ps = HashMap::new();
        let cs = HashMap::new();
        let ctx = Ctx { rels: &rels, nums: &nums, theme: &th, pstyles: &ps, cstyles: &cs };
        let (paras, _, drw, _) = parse_story(xml, &ctx, None);
        assert_eq!(paras.len(), 1);
        assert_eq!(paras[0].text(), "Tekst");
        assert_eq!(drw.len(), 1);
        let d = &drw[0];
        assert!((d.h_off - 10.0).abs() < 0.01 && (d.v_off - 20.0).abs() < 0.01);
        assert_eq!(d.wrap, WrapMode::Both);
        let s = &d.shapes[0];
        assert_eq!(s.prst, "ellipse");
        assert_eq!(s.fill, Some(Some(Color::rgb(255, 0, 0))));
        assert_eq!(s.line, Some(Some(Color::rgb(0, 0, 255))));
        assert_eq!(s.line_w, Some(2.0));
        assert_eq!(s.valign, VAlign::Center);
        assert_eq!(s.text.len(), 1);
        assert_eq!(s.text[0].text(), "W kole");
    }

    #[test]
    fn table_row_uses_grid_tabs() {
        let xml = r#"<w:body><w:tbl><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body>"#;
        let ps = ctx_parse(xml);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].text(), "A\tB");
        assert_eq!(ps[0].fmt.tabs.as_deref(), Some(&[100.0][..]));
    }
}
