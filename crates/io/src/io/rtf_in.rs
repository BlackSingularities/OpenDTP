//! Import RTF (WordPad, starsze edytory, „Zapisz jako RTF” z Worda): akapity z wyrównaniem
//! i wcięciami, pogrubienie, kursywa, podkreślenie, przekreślenie, indeksy, kroje, rozmiary
//! i kolory, nagłówki z poziomu konspektu, tytuł i autor z `\info`. Pomijamy obrazy, nagłówki
//! i stopki stron oraz kod pól (zostaje ich wynik).

use crate::model::*;

#[derive(Clone, Default)]
struct State {
    fmt: CharFmt,
    para: ParaFmt,
    style: Option<String>,
    /// Grupa, której treść pomijamy (tabela czcionek, obraz, kod pola…).
    skip: bool,
    /// Ile znaków zastępczych po `\uN` pominąć (`\ucN`).
    uc: usize,
}

struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
    ansi_1250: bool,
    fonts: Vec<(i32, String)>,
    colors: Vec<Color>,
    paras: Vec<Paragraph>,
    cur: Paragraph,
    meta: Meta,
    /// Dokąd trafia tekst grupy `\info` (tytuł, autor…).
    info: Option<&'static str>,
    pending_skip: usize,
}

fn hex(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

pub fn import(bytes: &[u8]) -> Result<Document, String> {
    if !bytes.starts_with(b"{\\rtf") {
        return Err(crate::lang::tr("To nie jest plik RTF").to_string());
    }
    let text = String::from_utf8_lossy(bytes);
    let mut p = Parser {
        src: bytes,
        pos: 0,
        ansi_1250: text.contains("\\ansicpg1250"),
        fonts: Vec::new(),
        colors: Vec::new(),
        paras: Vec::new(),
        cur: Paragraph { style: "normal".into(), fmt: ParaFmt::default(), runs: Vec::new() },
        meta: Meta::default(),
        info: None,
        pending_skip: 0,
    };
    p.run();
    let mut doc = Document::new();
    let mut paras = p.paras;
    if !p.cur.runs.is_empty() {
        p.cur.normalize();
        paras.push(p.cur);
    }
    if !paras.is_empty() {
        doc.body = Story { paras };
    }
    doc.meta.title = p.meta.title.trim().to_string();
    doc.meta.author = p.meta.author.trim().to_string();
    doc.meta.subject = p.meta.subject.trim().to_string();
    doc.meta.keywords = p.meta.keywords.trim().to_string();
    Ok(doc)
}

impl Parser<'_> {
    fn run(&mut self) {
        let mut stack: Vec<State> = Vec::new();
        let mut st = State { uc: 1, ..Default::default() };
        // Pierwszy znak w grupie po `{` – do rozpoznania `{\*\…}` (grupa do pominięcia, jeśli nieznana).
        let mut group_start = false;
        while self.pos < self.src.len() {
            let c = self.src[self.pos];
            match c {
                b'{' => {
                    stack.push(st.clone());
                    self.pos += 1;
                    group_start = true;
                    continue;
                }
                b'}' => {
                    if let Some(prev) = stack.pop() {
                        st = prev;
                    }
                    self.info = None;
                    self.pos += 1;
                }
                b'\\' => {
                    self.pos += 1;
                    self.control(&mut st, group_start);
                }
                b'\r' | b'\n' => self.pos += 1,
                _ => {
                    self.pos += 1;
                    if self.pending_skip > 0 {
                        self.pending_skip -= 1;
                    } else {
                        let ch = if c < 0x80 { c as char } else { self.byte_char(c) };
                        self.text(&st, ch);
                    }
                }
            }
            group_start = false;
        }
    }

    fn byte_char(&self, b: u8) -> char {
        if self.ansi_1250 {
            crate::util::cp1250(b)
        } else {
            b as char
        }
    }

    fn text(&mut self, st: &State, ch: char) {
        if let Some(field) = self.info {
            match field {
                "title" => self.meta.title.push(ch),
                "author" => self.meta.author.push(ch),
                "subject" => self.meta.subject.push(ch),
                _ => self.meta.keywords.push(ch),
            }
            return;
        }
        if st.skip {
            return;
        }
        if self.cur.runs.is_empty() {
            self.cur.fmt = st.para.clone();
            if let Some(s) = &st.style {
                self.cur.style = s.clone();
            }
        }
        match self.cur.runs.last_mut() {
            Some(r) if r.fmt == st.fmt => r.text.push(ch),
            _ => self.cur.runs.push(Run { text: ch.to_string(), fmt: st.fmt.clone(), field: None }),
        }
    }

    fn end_para(&mut self, st: &State) {
        let mut p = std::mem::replace(&mut self.cur, Paragraph { style: "normal".into(), fmt: ParaFmt::default(), runs: Vec::new() });
        if p.runs.is_empty() {
            p.fmt = st.para.clone();
            if let Some(s) = &st.style {
                p.style = s.clone();
            }
            p.runs.push(Run::default());
        }
        p.normalize();
        self.paras.push(p);
    }

    fn control(&mut self, st: &mut State, _group_start: bool) {
        let Some(&c) = self.src.get(self.pos) else { return };
        if !c.is_ascii_alphabetic() {
            self.pos += 1;
            match c {
                b'\'' => {
                    let h = self.src.get(self.pos).copied().and_then(hex);
                    let l = self.src.get(self.pos + 1).copied().and_then(hex);
                    self.pos += 2;
                    if let (Some(h), Some(l)) = (h, l) {
                        if self.pending_skip > 0 {
                            self.pending_skip -= 1;
                        } else {
                            let ch = self.byte_char(h * 16 + l);
                            self.text(st, ch);
                        }
                    }
                }
                // `{\*\…}` – rozszerzenie, którego nie znamy: cała grupa do pominięcia.
                b'*' => st.skip = true,
                b'~' => self.text(st, '\u{a0}'),
                b'-' => {}
                b'_' => self.text(st, '\u{2011}'),
                b'\\' | b'{' | b'}' => self.text(st, c as char),
                b'\n' | b'\r' => self.end_para(st),
                _ => {}
            }
            return;
        }
        let start = self.pos;
        while self.pos < self.src.len() && self.src[self.pos].is_ascii_alphabetic() {
            self.pos += 1;
        }
        let word = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("").to_string();
        let nstart = self.pos;
        if self.src.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        while self.pos < self.src.len() && self.src[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let num: Option<i32> = std::str::from_utf8(&self.src[nstart..self.pos]).ok().and_then(|s| s.parse().ok());
        if self.src.get(self.pos) == Some(&b' ') {
            self.pos += 1;
        }
        let on = num != Some(0);
        let n = num.unwrap_or(0);
        // W pomijanej grupie liczą się tylko metadane i znaki Unicode (żeby pominąć ich zastępniki).
        if st.skip && !matches!(word.as_str(), "title" | "author" | "subject" | "keywords" | "u" | "uc") {
            return;
        }
        match word.as_str() {
            // Grupy, których treść nie jest tekstem dokumentu.
            "fonttbl" => {
                st.skip = true;
                self.font_table();
            }
            "colortbl" => {
                st.skip = true;
                self.color_table();
            }
            "stylesheet" | "pict" | "header" | "footer" | "headerl" | "headerr" | "headerf" | "footerl" | "footerr" | "footerf" | "fldinst" | "object" | "themedata" | "colorschememapping" | "latentstyles" | "datastore" | "xmlnstbl" | "listtable" | "listoverridetable" | "rsidtbl" | "generator" | "filetbl" | "revtbl" | "bkmkstart" | "bkmkend" | "shp" | "nonshppict" => st.skip = true,
            "info" => st.skip = true,
            "title" | "author" | "subject" | "keywords" => {
                st.skip = true;
                self.info = Some(match word.as_str() {
                    "title" => "title",
                    "author" => "author",
                    "subject" => "subject",
                    _ => "keywords",
                });
            }
            "par" => self.end_para(st),
            "line" => self.text(st, '\u{2028}'),
            "page" => {
                self.end_para(st);
                st.para.break_before = Some(Break::Page);
            }
            "tab" => self.text(st, '\t'),
            "cell" => self.text(st, '\t'),
            "row" => self.end_para(st),
            "emdash" => self.text(st, '—'),
            "endash" => self.text(st, '–'),
            "bullet" => self.text(st, '•'),
            "lquote" => self.text(st, '‘'),
            "rquote" => self.text(st, '’'),
            "ldblquote" => self.text(st, '„'),
            "rdblquote" => self.text(st, '”'),
            "u" => {
                let v = if n < 0 { n + 65536 } else { n } as u32;
                if let Some(ch) = char::from_u32(v) {
                    self.text(st, ch);
                }
                self.pending_skip = st.uc;
            }
            "uc" => st.uc = n.max(0) as usize,
            "pard" => {
                st.para = ParaFmt::default();
                st.style = None;
            }
            "plain" => st.fmt = CharFmt::default(),
            "b" => st.fmt.bold = Some(on),
            "i" => st.fmt.italic = Some(on),
            "ul" => st.fmt.underline = Some(if on { Underline::Single } else { Underline::None }),
            "ulnone" => st.fmt.underline = Some(Underline::None),
            "strike" => st.fmt.strike = Some(on),
            "super" => st.fmt.script = Some(Script::Super),
            "sub" => st.fmt.script = Some(Script::Sub),
            "nosupersub" => st.fmt.script = None,
            "caps" => st.fmt.caps = Some(if on { Caps::All } else { Caps::None }),
            "scaps" => st.fmt.caps = Some(if on { Caps::Small } else { Caps::None }),
            "fs" if n > 0 => st.fmt.size = Some(n as f32 / 2.0),
            "f" => st.fmt.font = self.fonts.iter().find(|(id, _)| *id == n).map(|(_, f)| f.clone()),
            "cf" => st.fmt.color = if n > 0 { self.colors.get(n as usize).copied() } else { None },
            "highlight" | "chcbpat" => st.fmt.highlight = if n > 0 { self.colors.get(n as usize).copied() } else { None },
            "ql" => st.para.align = Some(Align::Left),
            "qc" => st.para.align = Some(Align::Center),
            "qr" => st.para.align = Some(Align::Right),
            "qj" => st.para.align = Some(Align::Justify),
            "li" => st.para.indent_left = Some(n as f32 / 20.0),
            "ri" => st.para.indent_right = Some(n as f32 / 20.0),
            "fi" => st.para.first_line = Some(n as f32 / 20.0),
            "sb" => st.para.space_before = Some(n as f32 / 20.0),
            "sa" => st.para.space_after = Some(n as f32 / 20.0),
            "outlinelevel" if (0..3).contains(&n) => st.style = Some(format!("heading{}", n + 1)),
            _ => {}
        }
    }

    /// `{\fonttbl{\f0\fswiss Arial;}{\f1 Times New Roman;}}` – numery i nazwy krojów.
    fn font_table(&mut self) {
        let mut depth = 1;
        let mut id = 0;
        let mut name = String::new();
        while self.pos < self.src.len() && depth > 0 {
            let c = self.src[self.pos];
            self.pos += 1;
            match c {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                b'\\' => {
                    let s = self.pos;
                    while self.pos < self.src.len() && self.src[self.pos].is_ascii_alphabetic() {
                        self.pos += 1;
                    }
                    let w = std::str::from_utf8(&self.src[s..self.pos]).unwrap_or("").to_string();
                    let ns = self.pos;
                    while self.pos < self.src.len() && (self.src[self.pos].is_ascii_digit() || self.src[self.pos] == b'-') {
                        self.pos += 1;
                    }
                    let n: i32 = std::str::from_utf8(&self.src[ns..self.pos]).ok().and_then(|x| x.parse().ok()).unwrap_or(0);
                    if self.src.get(self.pos) == Some(&b' ') {
                        self.pos += 1;
                    }
                    if w == "f" {
                        id = n;
                        name.clear();
                    }
                }
                b';' => {
                    let n = name.trim().to_string();
                    if !n.is_empty() {
                        self.fonts.push((id, n));
                    }
                    name.clear();
                }
                c if depth <= 2 && c > 0x20 || (c == b' ' && !name.is_empty()) => name.push(self.byte_char(c)),
                _ => {}
            }
        }
        // Zamykający `}` grupy tabeli zjedliśmy – oddajemy go głównej pętli, żeby zdjęła stan ze stosu.
        self.pos -= 1;
    }

    /// `{\colortbl;\red0\green0\blue0;\red255\green0\blue0;}` – indeks 0 to kolor domyślny.
    fn color_table(&mut self) {
        let mut cur = Color { r: 0, g: 0, b: 0, a: 255 };
        let mut any = false;
        while self.pos < self.src.len() {
            let c = self.src[self.pos];
            if c == b'}' {
                break;
            }
            self.pos += 1;
            match c {
                b'\\' => {
                    let s = self.pos;
                    while self.pos < self.src.len() && self.src[self.pos].is_ascii_alphabetic() {
                        self.pos += 1;
                    }
                    let w = std::str::from_utf8(&self.src[s..self.pos]).unwrap_or("").to_string();
                    let ns = self.pos;
                    while self.pos < self.src.len() && self.src[self.pos].is_ascii_digit() {
                        self.pos += 1;
                    }
                    let n: u8 = std::str::from_utf8(&self.src[ns..self.pos]).ok().and_then(|x| x.parse().ok()).unwrap_or(0);
                    if self.src.get(self.pos) == Some(&b' ') {
                        self.pos += 1;
                    }
                    any = true;
                    match w.as_str() {
                        "red" => cur.r = n,
                        "green" => cur.g = n,
                        "blue" => cur.b = n,
                        _ => {}
                    }
                }
                b';' => {
                    self.colors.push(if any { cur } else { Color { r: 0, g: 0, b: 0, a: 255 } });
                    cur = Color { r: 0, g: 0, b: 0, a: 255 };
                    any = false;
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_formatting_and_polish() {
        let rtf = br"{\rtf1\ansi\ansicpg1250{\fonttbl{\f0\fswiss Arial;}}{\colortbl;\red255\green0\blue0;}{\info{\title Raport}{\author Anna}}
\pard\qc\b Tytu\'b3\b0\par
\pard Zwyk\'b3y {\i kursywa} i \cf1 czerwony\cf0  \u8222?cytat\u8221?\par}";
        let d = import(rtf).unwrap();
        assert_eq!(d.meta.title, "Raport");
        assert_eq!(d.meta.author, "Anna");
        assert_eq!(d.body.paras[0].text(), "Tytuł");
        assert_eq!(d.body.paras[0].fmt.align, Some(Align::Center));
        assert_eq!(d.body.paras[0].runs[0].fmt.bold, Some(true));
        let p = &d.body.paras[1];
        assert_eq!(p.text(), "Zwykły kursywa i czerwony „cytat”");
        assert!(p.runs.iter().any(|r| r.text == "kursywa" && r.fmt.italic == Some(true)));
        assert!(p.runs.iter().any(|r| r.text.contains("czerwony") && r.fmt.color.is_some_and(|c| c.r == 255)));
    }
}
