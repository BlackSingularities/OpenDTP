//! Wklejanie HTML ze schowka (Word, przeglądarki, Outlook, Google Docs) z zachowaniem formatowania:
//! kroje, rozmiary, kolory, wyróżnienia, łącza, wyrównanie, wcięcia, odstępy, listy i nagłówki.
//! Tabele trafiają jako wiersze z tabulatorami; obrazy pomijamy.

use crate::model::*;

/// Akapity z fragmentu HTML (format „HTML Format” schowka Windows albo zwykły HTML).
pub fn fragment_from_html(html: &str) -> Vec<Paragraph> {
    let src = fragment(html);
    let mut b = Builder::default();
    b.run(src);
    b.flush();
    b.out
}

/// Sam fragment między znacznikami StartFragment/EndFragment (jeśli są).
fn fragment(html: &str) -> &str {
    let start = html.find("<!--StartFragment-->").map(|i| i + "<!--StartFragment-->".len());
    let end = html.rfind("<!--EndFragment-->");
    match (start, end) {
        (Some(a), Some(b)) if a <= b => &html[a..b],
        (Some(a), None) => &html[a..],
        _ => html.find('<').map(|i| &html[i..]).unwrap_or(html),
    }
}

#[derive(Clone)]
struct Inline {
    tag: String,
    fmt: CharFmt,
    /// Tekst pomijany (znacznik listy Worda „mso-list:Ignore”).
    skip: bool,
}

#[derive(Clone)]
struct Block {
    tag: String,
    style: Option<String>,
    fmt: ParaFmt,
    chars: CharFmt,
}

#[derive(Default)]
struct Builder {
    out: Vec<Paragraph>,
    runs: Vec<Run>,
    inlines: Vec<Inline>,
    blocks: Vec<Block>,
    /// Rodzaje otwartych list (ul/ol) – głębokość to poziom.
    lists: Vec<ListKind>,
    /// Poziom i rodzaj listy z Worda (mso-list) dla bieżącego akapitu.
    mso_list: Option<(u8, bool)>,
    /// Pierwszy tekst pominiętego znacznika listy Worda – po nim poznajemy numerację.
    mso_marker: String,
    /// Głębokość elementów, których treść pomijamy (style, script, head…).
    hidden: usize,
    pre: usize,
    /// Komórka tabeli w bieżącym wierszu (tabulator przed następną).
    cell: usize,
}

impl Builder {
    fn run(&mut self, s: &str) {
        let mut i = 0;
        let bytes = s.as_bytes();
        while i < s.len() {
            if bytes[i] == b'<' {
                if s[i..].starts_with("<!--") {
                    i = s[i..].find("-->").map(|e| i + e + 3).unwrap_or(s.len());
                    continue;
                }
                if s[i..].starts_with("<!") || s[i..].starts_with("<?") {
                    i = s[i..].find('>').map(|e| i + e + 1).unwrap_or(s.len());
                    continue;
                }
                let end = tag_end(s, i);
                self.tag(&s[i + 1..end.saturating_sub(1).max(i + 1)]);
                i = end;
            } else {
                let e = s[i..].find('<').map(|e| i + e).unwrap_or(s.len());
                if self.hidden == 0 {
                    let t = decode(&s[i..e]);
                    self.text(&t);
                }
                i = e;
            }
        }
    }

    fn tag(&mut self, raw: &str) {
        let raw = raw.trim();
        let closing = raw.starts_with('/');
        let body = raw.trim_start_matches('/').trim_end_matches('/');
        let name_end = body.find(|c: char| c.is_whitespace()).unwrap_or(body.len());
        let name = body[..name_end].to_ascii_lowercase();
        // Znaczniki z przestrzenią nazw (Word: <o:p>, <v:shape>) traktujemy jak nieznane wewnątrzwierszowe.
        let name = if name.contains(':') { format!("ns-{}", name.replace(':', "-")) } else { name };
        let attrs = &body[name_end..];
        if matches!(name.as_str(), "style" | "script" | "head" | "title" | "xml" | "template" | "noscript") {
            if closing {
                self.hidden = self.hidden.saturating_sub(1);
            } else if !raw.ends_with('/') {
                self.hidden += 1;
            }
            return;
        }
        if self.hidden > 0 {
            return;
        }
        if closing {
            self.close(&name);
        } else {
            self.open(&name, attrs, raw.ends_with('/'));
        }
    }

    fn open(&mut self, name: &str, attrs: &str, self_closing: bool) {
        let css = attr(attrs, "style").unwrap_or_default();
        let class = attr(attrs, "class").unwrap_or_default().to_ascii_lowercase();
        match name {
            "br" => self.flush(),
            "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "blockquote" | "pre" | "dt" | "dd" | "address" | "center" | "figcaption" => {
                self.flush();
                let parent = self.blocks.last().cloned();
                let mut fmt = parent.as_ref().map(|b| inherit_para(&b.fmt)).unwrap_or_default();
                let mut chars = parent.as_ref().map(|b| b.chars.clone()).unwrap_or_default();
                let mut style = parent.as_ref().and_then(|b| b.style.clone());
                match name {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let n = name[1..].parse::<u8>().unwrap_or(1).min(3);
                        style = Some(format!("heading{n}"));
                    }
                    "blockquote" => style = Some("quote".into()),
                    "pre" => {
                        style = Some("code".into());
                        self.pre += 1;
                    }
                    "center" => fmt.align = Some(Align::Center),
                    "li" => {
                        let kind = self.lists.last().copied().unwrap_or(ListKind::Bullet);
                        let level = self.lists.len().saturating_sub(1).min(8) as u8;
                        fmt.list = Some(Some(ListFmt { kind, level, start: None }));
                        style = Some("list".into());
                    }
                    _ => {}
                }
                if class.contains("msotitle") {
                    style = Some("title".into());
                } else if class.contains("msosubtitle") {
                    style = Some("subtitle".into());
                } else if class.contains("msoquote") {
                    style = Some("quote".into());
                }
                if let Some(a) = attr(attrs, "align") {
                    fmt.align = align(&a);
                }
                let base = chars.size.unwrap_or(11.0);
                css_para(&css, &mut fmt, base);
                css_chars(&css, &mut chars, base);
                // Lista z Worda: akapit z „mso-list: l0 level2 lfo1”.
                if let Some(v) = css_get(&css, "mso-list") {
                    let level = v.split_whitespace().find_map(|w| w.strip_prefix("level")).and_then(|n| n.parse::<u8>().ok()).unwrap_or(1);
                    self.mso_list = Some((level.saturating_sub(1).min(8), false));
                    self.mso_marker.clear();
                }
                self.blocks.push(Block { tag: name.to_string(), style, fmt, chars });
            }
            "ul" | "ol" => {
                self.flush();
                let kind = if name == "ol" {
                    match attr(attrs, "type").as_deref() {
                        Some("a") => ListKind::LowerLetter,
                        Some("A") => ListKind::UpperLetter,
                        Some("i") => ListKind::LowerRoman,
                        Some("I") => ListKind::UpperRoman,
                        _ => ListKind::Number,
                    }
                } else {
                    ListKind::Bullet
                };
                self.lists.push(kind);
            }
            "tr" => {
                self.flush();
                self.cell = 0;
            }
            "td" | "th" => {
                if self.cell > 0 {
                    self.push_text("\t");
                }
                self.cell += 1;
                let mut f = self.cur_fmt();
                if name == "th" {
                    f.bold = Some(true);
                }
                let base = f.size.unwrap_or(11.0);
                css_chars(&css, &mut f, base);
                self.inlines.push(Inline { tag: name.to_string(), fmt: f, skip: false });
            }
            "hr" => self.flush(),
            "img" => {}
            _ => {
                if self_closing {
                    return;
                }
                let mut f = self.cur_fmt();
                let base = f.size.unwrap_or(11.0);
                match name {
                    "b" | "strong" => f.bold = Some(true),
                    "i" | "em" | "cite" | "var" | "dfn" => f.italic = Some(true),
                    "u" | "ins" => f.underline = Some(Underline::Single),
                    "s" | "strike" | "del" => f.strike = Some(true),
                    "sup" => f.script = Some(Script::Super),
                    "sub" => f.script = Some(Script::Sub),
                    "small" => f.size = Some(base * 0.83),
                    "big" => f.size = Some(base * 1.2),
                    "mark" => f.highlight = Some(Color::rgb(0xff, 0xf2, 0x00)),
                    "code" | "kbd" | "samp" | "tt" => f.font = Some("Consolas".into()),
                    "a" => {
                        if let Some(h) = attr(attrs, "href").filter(|h| !h.starts_with('#') && !h.is_empty()) {
                            f.link = Some(h);
                        }
                    }
                    "font" => {
                        if let Some(face) = attr(attrs, "face") {
                            f.font = Some(first_family(&face));
                        }
                        if let Some(c) = attr(attrs, "color").and_then(|c| color(&c)) {
                            f.color = Some(c);
                        }
                        if let Some(n) = attr(attrs, "size").and_then(|n| n.trim().parse::<usize>().ok()) {
                            f.size = Some([7.5, 10.0, 12.0, 13.5, 18.0, 24.0, 36.0][n.clamp(1, 7) - 1]);
                        }
                    }
                    _ => {}
                }
                css_chars(&css, &mut f, base);
                let skip = css_get(&css, "mso-list").is_some_and(|v| v.to_ascii_lowercase().contains("ignore"));
                if skip {
                    if let Some(l) = self.mso_list.as_mut() {
                        l.1 = true;
                    }
                }
                let skip = skip || self.inlines.last().is_some_and(|i| i.skip);
                self.inlines.push(Inline { tag: name.to_string(), fmt: f, skip });
            }
        }
    }

    fn close(&mut self, name: &str) {
        match name {
            "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "blockquote" | "pre" | "dt" | "dd" | "address" | "center" | "figcaption" => {
                self.flush();
                if name == "pre" {
                    self.pre = self.pre.saturating_sub(1);
                }
                if let Some(i) = self.blocks.iter().rposition(|b| b.tag == name) {
                    self.blocks.truncate(i);
                }
                self.mso_list = None;
            }
            "ul" | "ol" => {
                self.flush();
                self.lists.pop();
            }
            "tr" | "table" => {
                self.flush();
                self.cell = 0;
            }
            _ => {
                if let Some(i) = self.inlines.iter().rposition(|t| t.tag == name) {
                    self.inlines.truncate(i);
                }
            }
        }
    }

    fn cur_fmt(&self) -> CharFmt {
        match self.inlines.last() {
            Some(i) => i.fmt.clone(),
            None => self.blocks.last().map(|b| b.chars.clone()).unwrap_or_default(),
        }
    }

    fn text(&mut self, t: &str) {
        if self.inlines.last().is_some_and(|i| i.skip) {
            self.mso_marker.push_str(t.trim());
            return;
        }
        if self.pre > 0 {
            for (k, line) in t.split('\n').enumerate() {
                if k > 0 {
                    self.flush();
                }
                self.push_text(&line.replace('\r', ""));
            }
            return;
        }
        // Białe znaki jak w przeglądarce: ciągi zwijamy do jednej spacji, bez spacji na początku akapitu.
        let mut s = String::with_capacity(t.len());
        let mut space = self.runs.last().map(|r| r.text.ends_with(' ') || r.text.ends_with('\t')).unwrap_or(true);
        for c in t.chars() {
            if c.is_whitespace() && c != '\u{a0}' {
                if !space {
                    s.push(' ');
                    space = true;
                }
            } else {
                s.push(if c == '\u{a0}' { ' ' } else { c });
                space = false;
            }
        }
        if !s.is_empty() {
            self.push_text(&s);
        }
    }

    fn push_text(&mut self, s: &str) {
        let fmt = self.cur_fmt();
        match self.runs.last_mut() {
            Some(r) if r.fmt == fmt => r.text.push_str(s),
            _ => self.runs.push(Run { text: s.to_string(), fmt, field: None }),
        }
    }

    /// Zamyka bieżący akapit (pusty pomijamy, chyba że to celowy pusty wiersz z <br>).
    fn flush(&mut self) {
        let mut runs = std::mem::take(&mut self.runs);
        if let Some(r) = runs.last_mut() {
            let t = r.text.trim_end_matches(' ').len();
            r.text.truncate(t);
        }
        runs.retain(|r| !r.text.is_empty());
        if runs.is_empty() {
            return;
        }
        let blk = self.blocks.last();
        let mut fmt = blk.map(|b| b.fmt.clone()).unwrap_or_default();
        let mut style = blk.and_then(|b| b.style.clone()).unwrap_or_else(|| "normal".into());
        if let Some((level, _)) = self.mso_list {
            let m = self.mso_marker.trim();
            let kind = match m.chars().next() {
                Some(c) if c.is_ascii_digit() => {
                    if m.contains(')') {
                        ListKind::NumberParen
                    } else {
                        ListKind::Number
                    }
                }
                Some(c) if c.is_ascii_lowercase() && m.len() <= 3 && "ivx".contains(c) && m.chars().all(|c| "ivxlc.)".contains(c)) => ListKind::LowerRoman,
                Some(c) if c.is_ascii_uppercase() && m.chars().all(|c| "IVXLC.)".contains(c)) && m.len() <= 4 => ListKind::UpperRoman,
                Some(c) if c.is_ascii_lowercase() => ListKind::LowerLetter,
                Some(c) if c.is_ascii_uppercase() => ListKind::UpperLetter,
                Some('-') | Some('–') => ListKind::Dash,
                _ => ListKind::Bullet,
            };
            fmt.list = Some(Some(ListFmt { kind, level, start: None }));
            // Wcięcia z Worda liczą też miejsce na znacznik – listę wcinamy sami.
            fmt.indent_left = None;
            fmt.first_line = None;
            if style == "normal" {
                style = "list".into();
            }
        }
        let mut p = Paragraph { style, fmt, runs };
        p.normalize();
        self.out.push(p);
        self.mso_marker.clear();
    }
}

/// Koniec znacznika `<…>` z pominięciem `>` w cudzysłowach.
fn tag_end(s: &str, start: usize) -> usize {
    let mut q: Option<char> = None;
    for (k, c) in s[start..].char_indices() {
        match (q, c) {
            (None, '"') | (None, '\'') => q = Some(c),
            (Some(o), c) if c == o => q = None,
            (None, '>') => return start + k + 1,
            _ => {}
        }
    }
    s.len()
}

fn attr(attrs: &str, name: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let at = from + i;
        let before_ok = at == 0 || lower.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = lower[at + name.len()..].trim_start();
        if before_ok && rest.starts_with('=') {
            let off = attrs.len() - rest.len() + 1;
            let v = attrs[off..].trim_start();
            let val = match v.chars().next() {
                Some(q @ ('"' | '\'')) => v[1..].split(q).next().unwrap_or("").to_string(),
                _ => v.split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or("").to_string(),
            };
            return Some(decode(&val));
        }
        from = at + name.len();
    }
    None
}

fn css_get(css: &str, key: &str) -> Option<String> {
    css.split(';').find_map(|d| {
        let (k, v) = d.split_once(':')?;
        (k.trim().eq_ignore_ascii_case(key)).then(|| v.trim().trim_end_matches("!important").trim().to_string())
    })
}

/// Długość CSS w punktach; `em` i `%` względem `base` (pt).
fn length(v: &str, base: f32) -> Option<f32> {
    let v = v.trim().to_ascii_lowercase();
    let num = |s: &str| s.trim().parse::<f32>().ok();
    if let Some(n) = v.strip_suffix("pt") {
        num(n)
    } else if let Some(n) = v.strip_suffix("px") {
        num(n).map(|x| x * 0.75)
    } else if let Some(n) = v.strip_suffix("rem") {
        num(n).map(|x| x * 12.0)
    } else if let Some(n) = v.strip_suffix("em") {
        num(n).map(|x| x * base)
    } else if let Some(n) = v.strip_suffix('%') {
        num(n).map(|x| x / 100.0 * base)
    } else if let Some(n) = v.strip_suffix("cm") {
        num(n).map(|x| x * 28.346)
    } else if let Some(n) = v.strip_suffix("mm") {
        num(n).map(|x| x * 2.8346)
    } else if let Some(n) = v.strip_suffix("in") {
        num(n).map(|x| x * 72.0)
    } else if v == "0" {
        Some(0.0)
    } else {
        None
    }
}

fn first_family(v: &str) -> String {
    let fams: Vec<String> = v.split(',').map(|f| f.trim().trim_matches(|c| c == '"' || c == '\'').to_string()).filter(|f| !f.is_empty()).collect();
    let generic = ["serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui", "-apple-system", "inherit", "initial"];
    fams.iter().find(|f| !generic.contains(&f.to_ascii_lowercase().as_str())).cloned().unwrap_or_else(|| match fams.first().map(|f| f.to_ascii_lowercase()) {
        Some(g) if g == "serif" => "Times New Roman".into(),
        Some(g) if g == "monospace" => "Consolas".into(),
        _ => "Calibri".into(),
    })
}

fn color(v: &str) -> Option<Color> {
    let v = v.trim().to_ascii_lowercase();
    if v.starts_with('#') {
        return Color::parse(&v);
    }
    if let Some(inner) = v.strip_prefix("rgba(").or_else(|| v.strip_prefix("rgb(")) {
        let p: Vec<f32> = inner.trim_end_matches(')').split(|c| c == ',' || c == ' ' || c == '/').filter(|s| !s.is_empty()).filter_map(|s| s.trim_end_matches('%').parse().ok()).collect();
        if p.len() >= 3 {
            let a = p.get(3).map(|a| if *a <= 1.0 { a * 255.0 } else { *a }).unwrap_or(255.0);
            if a < 1.0 {
                return None;
            }
            return Some(Color::rgb(p[0] as u8, p[1] as u8, p[2] as u8));
        }
        return None;
    }
    let named = match v.as_str() {
        "black" | "windowtext" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "lime" => (0, 255, 0),
        "blue" => (0, 0, 255),
        "navy" => (0, 0, 128),
        "yellow" => (255, 255, 0),
        "orange" => (255, 165, 0),
        "purple" => (128, 0, 128),
        "fuchsia" | "magenta" => (255, 0, 255),
        "aqua" | "cyan" => (0, 255, 255),
        "teal" => (0, 128, 128),
        "maroon" => (128, 0, 0),
        "olive" => (128, 128, 0),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "darkred" => (139, 0, 0),
        "darkblue" => (0, 0, 139),
        "darkgreen" => (0, 100, 0),
        "darkgray" | "darkgrey" => (169, 169, 169),
        "lightgray" | "lightgrey" => (211, 211, 211),
        _ => return None,
    };
    Some(Color::rgb(named.0, named.1, named.2))
}

fn align(v: &str) -> Option<Align> {
    match v.trim().to_ascii_lowercase().as_str() {
        "center" | "middle" => Some(Align::Center),
        "right" | "end" => Some(Align::Right),
        "justify" => Some(Align::Justify),
        "left" | "start" => Some(Align::Left),
        _ => None,
    }
}

/// Formatowanie akapitu dziedziczone przez zagnieżdżony blok (bez odstępów i list).
fn inherit_para(p: &ParaFmt) -> ParaFmt {
    ParaFmt { align: p.align.clone(), indent_left: p.indent_left, indent_right: p.indent_right, line: p.line.clone(), shading: p.shading.clone(), ..ParaFmt::default() }
}

fn css_para(css: &str, f: &mut ParaFmt, base: f32) {
    if let Some(a) = css_get(css, "text-align").and_then(|v| align(&v)) {
        f.align = Some(a);
    }
    let mut margins = [None; 4];
    if let Some(m) = css_get(css, "margin") {
        let v: Vec<Option<f32>> = m.split_whitespace().map(|x| length(x, base)).collect();
        margins = match v.len() {
            1 => [v[0], v[0], v[0], v[0]],
            2 => [v[0], v[1], v[0], v[1]],
            3 => [v[0], v[1], v[2], v[1]],
            n if n >= 4 => [v[0], v[1], v[2], v[3]],
            _ => margins,
        };
    }
    for (k, i) in [("margin-top", 0), ("margin-right", 1), ("margin-bottom", 2), ("margin-left", 3)] {
        if let Some(v) = css_get(css, k).and_then(|v| length(&v, base)) {
            margins[i] = Some(v);
        }
    }
    if let Some(v) = margins[0] {
        f.space_before = Some(v.max(0.0));
    }
    if let Some(v) = margins[2] {
        f.space_after = Some(v.max(0.0));
    }
    if let Some(v) = margins[1] {
        f.indent_right = Some(v.max(0.0));
    }
    if let Some(v) = margins[3] {
        f.indent_left = Some(v.max(0.0));
    }
    if let Some(v) = css_get(css, "text-indent").and_then(|v| length(&v, base)) {
        f.first_line = Some(v);
    }
    if let Some(v) = css_get(css, "line-height") {
        let t = v.trim().to_ascii_lowercase();
        if let Ok(k) = t.parse::<f32>() {
            f.line = Some(LineSpacing::Multiple((k / 1.15).clamp(0.5, 4.0)));
        } else if let Some(p) = t.strip_suffix('%').and_then(|p| p.trim().parse::<f32>().ok()) {
            f.line = Some(LineSpacing::Multiple((p / 100.0).clamp(0.5, 4.0)));
        } else if let Some(pt) = length(&t, base) {
            f.line = Some(LineSpacing::Exact(pt));
        }
    }
    if let Some(c) = css_get(css, "background-color").or_else(|| css_get(css, "background")).and_then(|v| color(&v)) {
        if c != Color::WHITE {
            f.shading = Some(c);
        }
    }
    if css_get(css, "page-break-before").or_else(|| css_get(css, "break-before")).is_some_and(|v| v.contains("always") || v.contains("page")) {
        f.break_before = Some(Break::Page);
    }
}

fn css_chars(css: &str, f: &mut CharFmt, base: f32) {
    if css.is_empty() {
        return;
    }
    if let Some(v) = css_get(css, "font-family") {
        f.font = Some(first_family(&v));
    }
    if let Some(v) = css_get(css, "font-size") {
        let s = match v.trim() {
            "xx-small" => Some(7.0),
            "x-small" => Some(7.5),
            "small" => Some(10.0),
            "medium" => Some(12.0),
            "large" => Some(13.5),
            "x-large" => Some(18.0),
            "xx-large" => Some(24.0),
            "smaller" => Some(base * 0.83),
            "larger" => Some(base * 1.2),
            o => length(o, base),
        };
        if let Some(s) = s.filter(|s| *s > 1.0 && *s < 1000.0) {
            f.size = Some((s * 2.0).round() / 2.0);
        }
    }
    if let Some(v) = css_get(css, "font-weight") {
        let v = v.to_ascii_lowercase();
        f.bold = Some(v == "bold" || v == "bolder" || v.parse::<u32>().is_ok_and(|w| w >= 600));
    }
    if let Some(v) = css_get(css, "font-style") {
        f.italic = Some(v.contains("italic") || v.contains("oblique"));
    }
    for key in ["text-decoration", "text-decoration-line"] {
        if let Some(v) = css_get(css, key) {
            let v = v.to_ascii_lowercase();
            if v.contains("underline") {
                f.underline = Some(if v.contains("double") { Underline::Double } else if v.contains("dotted") { Underline::Dotted } else if v.contains("wavy") { Underline::Wave } else { Underline::Single });
            } else if v.contains("none") {
                f.underline = Some(Underline::None);
            }
            if v.contains("line-through") {
                f.strike = Some(true);
            }
        }
    }
    if let Some(c) = css_get(css, "color").and_then(|v| color(&v)) {
        f.color = Some(c);
    }
    if let Some(c) = css_get(css, "mso-highlight").or_else(|| css_get(css, "background-color")).or_else(|| css_get(css, "background")).and_then(|v| color(&v)) {
        if c != Color::WHITE {
            f.highlight = Some(c);
        }
    }
    if let Some(v) = css_get(css, "vertical-align") {
        match v.trim() {
            "super" => f.script = Some(Script::Super),
            "sub" => f.script = Some(Script::Sub),
            _ => {}
        }
    }
    if css_get(css, "text-transform").is_some_and(|v| v.contains("uppercase")) {
        f.caps = Some(Caps::All);
    }
    if css_get(css, "font-variant").or_else(|| css_get(css, "font-variant-caps")).is_some_and(|v| v.contains("small-caps")) {
        f.caps = Some(Caps::Small);
    }
    if let Some(v) = css_get(css, "letter-spacing").and_then(|v| length(&v, base)) {
        f.tracking = Some(v);
    }
}

/// Encje HTML: nazwane najczęstsze i liczbowe.
fn decode(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..end];
        let ch = if let Some(n) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
            u32::from_str_radix(n, 16).ok().and_then(char::from_u32).map(String::from)
        } else if let Some(n) = ent.strip_prefix('#') {
            n.parse::<u32>().ok().and_then(char::from_u32).map(String::from)
        } else {
            match ent {
                "amp" => Some("&"),
                "lt" => Some("<"),
                "gt" => Some(">"),
                "quot" => Some("\""),
                "apos" => Some("'"),
                "nbsp" => Some("\u{a0}"),
                "ndash" => Some("–"),
                "mdash" => Some("—"),
                "hellip" => Some("…"),
                "bdquo" => Some("„"),
                "ldquo" => Some("“"),
                "rdquo" => Some("”"),
                "lsquo" => Some("‘"),
                "rsquo" => Some("’"),
                "sbquo" => Some("‚"),
                "laquo" => Some("«"),
                "raquo" => Some("»"),
                "bull" => Some("•"),
                "middot" => Some("·"),
                "copy" => Some("©"),
                "reg" => Some("®"),
                "trade" => Some("™"),
                "deg" => Some("°"),
                "euro" => Some("€"),
                "sect" => Some("§"),
                "para" => Some("¶"),
                "times" => Some("×"),
                "shy" => Some("\u{ad}"),
                "ensp" | "emsp" | "thinsp" => Some(" "),
                _ => None,
            }
            .map(String::from)
        };
        match ch {
            Some(c) => {
                out.push_str(&c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_like_fragment() {
        let html = r#"<html><body><!--StartFragment--><h1>Tytuł</h1><p class=MsoNormal style='text-align:center;margin-bottom:6pt'><b><span style='font-size:14.0pt;font-family:"Georgia",serif;color:#C00000'>Ważne</span></b> zwykły&nbsp;tekst</p><ul><li>jeden</li><li><i>dwa</i></li></ul><!--EndFragment--></body></html>"#;
        let ps = fragment_from_html(html);
        assert_eq!(ps.len(), 4);
        assert_eq!(ps[0].style, "heading1");
        assert_eq!(ps[1].fmt.align, Some(Align::Center));
        assert_eq!(ps[1].fmt.space_after, Some(6.0));
        let r = &ps[1].runs[0];
        assert_eq!(r.text, "Ważne");
        assert_eq!(r.fmt.bold, Some(true));
        assert_eq!(r.fmt.size, Some(14.0));
        assert_eq!(r.fmt.font.as_deref(), Some("Georgia"));
        assert_eq!(r.fmt.color, Some(Color::rgb(0xc0, 0, 0)));
        assert_eq!(ps[1].text(), "Ważne zwykły tekst");
        assert!(ps[2].fmt.list.flatten().is_some_and(|l| l.kind == ListKind::Bullet));
        assert_eq!(ps[3].runs[0].fmt.italic, Some(true));
    }

    #[test]
    fn word_list_marker_is_skipped() {
        let html = r#"<p class=MsoListParagraph style='mso-list:l0 level2 lfo1'><span style='mso-list:Ignore'>1.<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp; </span></span>Punkt</p>"#;
        let ps = fragment_from_html(html);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].text(), "Punkt");
        let l = ps[0].fmt.list.flatten().unwrap();
        assert_eq!(l.kind, ListKind::Number);
        assert_eq!(l.level, 1);
    }
}
