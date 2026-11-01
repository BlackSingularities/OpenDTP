//! Tekst zwykły, Markdown i HTML – eksport oraz import tekstu i Markdown.

use std::fmt::Write as _;

use crate::model::*;

pub fn to_text(doc: &Document) -> String {
    doc.body.paras.iter().map(|p| p.text().replace(FIELD_CHAR, "")).collect::<Vec<_>>().join("\r\n")
}

fn md_runs(p: &Paragraph) -> String {
    let mut s = String::new();
    for r in &p.runs {
        let t = r.text.replace(FIELD_CHAR, "");
        if t.is_empty() {
            continue;
        }
        let (b, i) = (r.fmt.bold == Some(true), r.fmt.italic == Some(true));
        let mark = match (b, i) {
            (true, true) => "***",
            (true, false) => "**",
            (false, true) => "*",
            _ => "",
        };
        let body = if let Some(l) = &r.fmt.link { format!("[{t}]({l})") } else { t };
        let _ = write!(s, "{mark}{body}{mark}");
    }
    s
}

pub fn to_markdown(doc: &Document) -> String {
    let mut out = String::new();
    let mut counters = [0u32; 9];
    for p in &doc.body.paras {
        let (pp, _) = doc.resolve_para(p);
        let text = md_runs(p);
        if let Some(l) = doc.outline_level(p) {
            let _ = writeln!(out, "{} {}\n", "#".repeat(l as usize), text);
            continue;
        }
        if p.style == "title" {
            let _ = writeln!(out, "# {text}\n");
            continue;
        }
        if p.style == "quote" {
            let _ = writeln!(out, "> {text}\n");
            continue;
        }
        if p.style == "code" {
            let _ = writeln!(out, "    {}", p.text());
            continue;
        }
        if let Some(l) = pp.list {
            let ind = "  ".repeat(l.level as usize);
            if l.kind.numbered() {
                counters[l.level as usize % 9] += 1;
                let _ = writeln!(out, "{ind}{}. {text}", counters[l.level as usize % 9]);
            } else {
                let _ = writeln!(out, "{ind}- {text}");
            }
            continue;
        }
        counters = [0; 9];
        let _ = writeln!(out, "{text}\n");
    }
    out
}

fn html_esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn to_html(doc: &Document) -> String {
    let mut out = String::from("<!doctype html>\n<html lang=\"pl\"><head><meta charset=\"utf-8\">");
    let _ = write!(out, "<title>{}</title>", html_esc(&doc.meta.title));
    out.push_str("<style>body{max-width:46em;margin:3em auto;padding:0 1em;line-height:1.5}</style></head><body>\n");
    let mut in_list: Option<bool> = None;
    for p in &doc.body.paras {
        let (pp, base) = doc.resolve_para(p);
        let mut inner = String::new();
        for r in &p.runs {
            let t = html_esc(&r.text.replace(FIELD_CHAR, "")).replace('\n', "<br>");
            if t.is_empty() {
                continue;
            }
            let mut cp = base.clone();
            cp.apply(&r.fmt);
            let mut css = String::new();
            if r.fmt.font.is_some() {
                let _ = write!(css, "font-family:'{}';", cp.font);
            }
            if r.fmt.size.is_some() {
                let _ = write!(css, "font-size:{}pt;", cp.size);
            }
            if r.fmt.color.is_some() {
                let _ = write!(css, "color:{};", cp.color.hex());
            }
            if !cp.highlight.is_none() {
                let _ = write!(css, "background:{};", cp.highlight.hex());
            }
            let mut piece = if css.is_empty() { t } else { format!("<span style=\"{css}\">{t}</span>") };
            if cp.bold {
                piece = format!("<strong>{piece}</strong>");
            }
            if cp.italic {
                piece = format!("<em>{piece}</em>");
            }
            if cp.underline != Underline::None {
                piece = format!("<u>{piece}</u>");
            }
            if cp.strike {
                piece = format!("<s>{piece}</s>");
            }
            match cp.script {
                Script::Super => piece = format!("<sup>{piece}</sup>"),
                Script::Sub => piece = format!("<sub>{piece}</sub>"),
                Script::Normal => {}
            }
            if let Some(l) = &cp.link {
                piece = format!("<a href=\"{}\">{piece}</a>", html_esc(l));
            }
            inner.push_str(&piece);
        }
        let ordered = pp.list.map(|l| l.kind.numbered());
        if in_list != ordered {
            if let Some(o) = in_list {
                out.push_str(if o { "</ol>\n" } else { "</ul>\n" });
            }
            if let Some(o) = ordered {
                out.push_str(if o { "<ol>\n" } else { "<ul>\n" });
            }
            in_list = ordered;
        }
        let align = match pp.align {
            Align::Left => "",
            Align::Center => " style=\"text-align:center\"",
            Align::Right => " style=\"text-align:right\"",
            Align::Justify => " style=\"text-align:justify\"",
        };
        if ordered.is_some() {
            let _ = writeln!(out, "<li{align}>{inner}</li>");
        } else if let Some(l) = doc.outline_level(p) {
            let _ = writeln!(out, "<h{l}{align}>{inner}</h{l}>");
        } else if p.style == "title" {
            let _ = writeln!(out, "<h1{align}>{inner}</h1>");
        } else if p.style == "quote" {
            let _ = writeln!(out, "<blockquote{align}>{inner}</blockquote>");
        } else {
            let _ = writeln!(out, "<p{align}>{inner}</p>");
        }
    }
    if let Some(o) = in_list {
        out.push_str(if o { "</ol>\n" } else { "</ul>\n" });
    }
    out.push_str("</body></html>\n");
    out
}

pub fn from_text(text: &str) -> Document {
    let mut doc = Document::new();
    let t = text.replace("\r\n", "\n").replace('\r', "\n");
    doc.body = Story::plain(&t, "normal");
    doc
}

/// Wyróżnienia `**`, `*`, `_` i odnośniki `[tekst](adres)` w jednym wierszu Markdown.
/// Kolor wyróżnienia `==tekst==` (zdania przykładowe, akcenty).
pub const MARK_COLOR: Color = Color::rgb(0x1f, 0x5f, 0xc0);

pub fn md_inline(s: &str) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    let mut bold = false;
    let mut ital = false;
    let mut mark = false;
    let mut buf = String::new();
    let chars: Vec<char> = s.chars().collect();
    let flush = |buf: &mut String, runs: &mut Vec<Run>, bold: bool, ital: bool, mark: bool, link: Option<String>| {
        if !buf.is_empty() {
            let color = if link.is_some() { Some(Color::rgb(0x05, 0x63, 0xc1)) } else if mark { Some(MARK_COLOR) } else { None };
            let fmt = CharFmt { bold: bold.then_some(true), italic: ital.then_some(true), link: link.clone(), color, underline: link.as_ref().map(|_| Underline::Single), ..Default::default() };
            runs.push(Run { text: std::mem::take(buf), fmt, field: None });
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '=' && chars.get(i + 1) == Some(&'=') {
            flush(&mut buf, &mut runs, bold, ital, mark, None);
            mark = !mark;
            i += 2;
            continue;
        }
        if c == '*' && chars.get(i + 1) == Some(&'*') {
            flush(&mut buf, &mut runs, bold, ital, mark, None);
            bold = !bold;
            i += 2;
            continue;
        }
        if c == '*' || (c == '_' && (i == 0 || !chars[i - 1].is_alphanumeric())) {
            flush(&mut buf, &mut runs, bold, ital, mark, None);
            ital = !ital;
            i += 1;
            continue;
        }
        if c == '[' {
            if let Some(close) = chars[i..].iter().position(|&x| x == ']') {
                let j = i + close;
                if chars.get(j + 1) == Some(&'(') {
                    if let Some(end) = chars[j..].iter().position(|&x| x == ')') {
                        let text: String = chars[i + 1..j].iter().collect();
                        let url: String = chars[j + 2..j + end].iter().collect();
                        flush(&mut buf, &mut runs, bold, ital, mark, None);
                        buf = text;
                        flush(&mut buf, &mut runs, bold, ital, mark, Some(url));
                        i = j + end + 1;
                        continue;
                    }
                }
            }
        }
        buf.push(c);
        i += 1;
    }
    flush(&mut buf, &mut runs, bold, ital, mark, None);
    if runs.is_empty() {
        runs.push(Run::default());
    }
    runs
}

pub fn from_markdown(text: &str) -> Document {
    let mut doc = Document::new();
    let mut paras = Vec::new();
    let mut code = false;
    let mut pending = String::new();
    let push_pending = |pending: &mut String, paras: &mut Vec<Paragraph>| {
        if !pending.trim().is_empty() {
            paras.push(Paragraph { style: "normal".into(), fmt: ParaFmt::default(), runs: md_inline(pending.trim()) });
        }
        pending.clear();
    };
    for line in text.replace("\r\n", "\n").lines() {
        if line.trim_start().starts_with("```") {
            push_pending(&mut pending, &mut paras);
            code = !code;
            continue;
        }
        if code {
            paras.push(Paragraph::with_text("code", line));
            continue;
        }
        let t = line.trim_end();
        if t.trim().is_empty() {
            push_pending(&mut pending, &mut paras);
            continue;
        }
        let lead = t.len() - t.trim_start().len();
        let s = t.trim_start();
        let hashes = s.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) && s[hashes..].starts_with(' ') {
            push_pending(&mut pending, &mut paras);
            let style = match hashes {
                1 => "heading1",
                2 => "heading2",
                _ => "heading3",
            };
            paras.push(Paragraph { style: style.into(), fmt: ParaFmt::default(), runs: md_inline(s[hashes + 1..].trim()) });
            continue;
        }
        if let Some(q) = s.strip_prefix("> ") {
            push_pending(&mut pending, &mut paras);
            paras.push(Paragraph { style: "quote".into(), fmt: ParaFmt::default(), runs: md_inline(q) });
            continue;
        }
        let level = (lead / 2).min(8) as u8;
        if let Some(item) = s.strip_prefix("- ").or_else(|| s.strip_prefix("* ")).or_else(|| s.strip_prefix("+ ")) {
            push_pending(&mut pending, &mut paras);
            let fmt = ParaFmt { list: Some(Some(ListFmt { kind: ListKind::Bullet, level, start: None })), ..Default::default() };
            paras.push(Paragraph { style: "list".into(), fmt, runs: md_inline(item) });
            continue;
        }
        let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && s[digits..].starts_with(". ") {
            push_pending(&mut pending, &mut paras);
            let fmt = ParaFmt { list: Some(Some(ListFmt { kind: ListKind::Number, level, start: None })), ..Default::default() };
            paras.push(Paragraph { style: "list".into(), fmt, runs: md_inline(&s[digits + 2..]) });
            continue;
        }
        if !pending.is_empty() {
            pending.push(' ');
        }
        pending.push_str(s);
    }
    push_pending(&mut pending, &mut paras);
    if !paras.is_empty() {
        doc.body = Story { paras };
    }
    doc
}
