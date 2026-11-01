//! Cytowania i bibliografia (jak w WordCraft): style APA 7, MLA 9, Chicago (autor–data) i IEEE,
//! import źródeł z BibTeX (Zotero, JabRef, Google Scholar) i RIS (Mendeley, EndNote) oraz
//! z prostego formularza. Cytowanie w treści to tekst z łączem `cite:<id>`; „Aktualizuj”
//! przelicza jego treść (np. numery IEEE w kolejności pierwszego użycia) i bibliografię
//! (akapity stylu `bibliography`).

use crate::model::*;
use crate::tables::pl_key;

// ---------------------------------------------------------------- nazwiska

/// („Kowalski”, „Jan Maria”) z „Kowalski, Jan Maria” albo „Jan Maria Kowalski”.
fn split_name(a: &str) -> (String, String) {
    let a = a.trim();
    if let Some((last, first)) = a.split_once(',') {
        return (last.trim().to_string(), first.trim().to_string());
    }
    match a.rsplit_once(' ') {
        Some((first, last)) => (last.to_string(), first.to_string()),
        None => (a.to_string(), String::new()),
    }
}

/// „Jan Maria” → „J. M.”
fn initials(first: &str) -> String {
    first
        .split(|c: char| c.is_whitespace())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let parts: Vec<String> = w.split('-').filter_map(|p| p.chars().next()).map(|c| format!("{c}.")).collect();
            parts.join("-")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

struct Words {
    and: &'static str,
    et_al: &'static str,
    no_date: &'static str,
    accessed: &'static str,
    pp: &'static str,
    heading: &'static str,
}

fn words(doc: &Document) -> Words {
    if doc.settings.lang.starts_with("pl") {
        Words { and: "i", et_al: "i in.", no_date: "b.d.", accessed: "dostęp", pp: "s.", heading: "Bibliografia" }
    } else {
        Words { and: "and", et_al: "et al.", no_date: "n.d.", accessed: "accessed", pp: "pp.", heading: "References" }
    }
}

fn year(s: &Source, w: &Words) -> String {
    if s.year.trim().is_empty() {
        w.no_date.to_string()
    } else {
        s.year.trim().to_string()
    }
}

/// Nazwiska do cytowania w tekście.
fn cite_names(s: &Source, style: CiteStyle, w: &Words) -> String {
    let lasts: Vec<String> = s.authors.iter().map(|a| split_name(a).0).filter(|l| !l.is_empty()).collect();
    let amp = if style == CiteStyle::Apa && w.and == "and" { "&" } else { w.and };
    match lasts.len() {
        0 => short_title(&s.title),
        1 => lasts[0].clone(),
        2 => format!("{} {amp} {}", lasts[0], lasts[1]),
        3 if style == CiteStyle::Chicago => format!("{}, {}, {amp} {}", lasts[0], lasts[1], lasts[2]),
        _ => format!("{} {}", lasts[0], w.et_al),
    }
}

fn short_title(t: &str) -> String {
    let words: Vec<&str> = t.split_whitespace().take(4).collect();
    format!("„{}”", words.join(" "))
}

// ---------------------------------------------------------------- cytowanie w tekście

/// Treść cytowania kilku źródeł naraz; `num` – numer IEEE źródła.
pub fn citation_text(doc: &Document, ids: &[u64], num: &dyn Fn(u64) -> usize) -> String {
    let style = doc.cite_style;
    let w = words(doc);
    let parts: Vec<String> = ids
        .iter()
        .filter_map(|id| doc.sources.iter().find(|s| s.id == *id))
        .map(|s| match style {
            CiteStyle::Apa => format!("{}, {}", cite_names(s, style, &w), year(s, &w)),
            CiteStyle::Mla => cite_names(s, style, &w),
            CiteStyle::Chicago => format!("{} {}", cite_names(s, style, &w), year(s, &w)),
            CiteStyle::Ieee => format!("[{}]", num(s.id)),
        })
        .collect();
    if parts.is_empty() {
        return "(?)".into();
    }
    if style == CiteStyle::Ieee {
        parts.join(", ")
    } else {
        format!("({})", parts.join("; "))
    }
}

// ---------------------------------------------------------------- bibliografia

/// Kawałek wpisu: tekst i czy kursywą.
type Piece = (String, bool);

fn plain(s: impl Into<String>) -> Piece {
    (s.into(), false)
}

fn ital(s: impl Into<String>) -> Piece {
    (s.into(), true)
}

fn dash(p: &str) -> String {
    p.replace("--", "–").replace('-', "–")
}

fn end_dot(s: &str) -> String {
    let t = s.trim_end();
    if t.ends_with(['.', '?', '!']) {
        t.to_string()
    } else {
        format!("{t}.")
    }
}

/// Autorzy w bibliografii (APA: „Kowalski, J., & Nowak, A.”, MLA/Chicago: „Kowalski, Jan, and Anna Nowak”,
/// IEEE: „J. Kowalski and A. Nowak”).
fn bib_authors(s: &Source, style: CiteStyle, w: &Words) -> String {
    let names: Vec<(String, String)> = s.authors.iter().map(|a| split_name(a)).collect();
    let n = names.len();
    let join = |v: Vec<String>, amp: &str| -> String {
        match v.len() {
            0 => String::new(),
            1 => v[0].clone(),
            2 if style != CiteStyle::Apa => format!("{} {amp} {}", v[0], v[1]),
            _ => {
                let (last, rest) = v.split_last().unwrap();
                format!("{}, {amp} {}", rest.join(", "), last)
            }
        }
    };
    match style {
        CiteStyle::Apa => {
            let amp = if w.and == "and" { "&" } else { w.and };
            let v: Vec<String> = names.iter().take(20).map(|(l, f)| if f.is_empty() { l.clone() } else { format!("{l}, {}", initials(f)) }).collect();
            join(v, amp)
        }
        CiteStyle::Mla => match n {
            0 => String::new(),
            1 => format_first(&names[0]),
            2 => format!("{}, {} {} {}", names[0].0, names[0].1, w.and, format!("{} {}", names[1].1, names[1].0).trim()),
            _ => format!("{}, {}", format_first(&names[0]), w.et_al),
        },
        CiteStyle::Chicago => {
            let mut v = Vec::new();
            for (i, nm) in names.iter().enumerate() {
                v.push(if i == 0 { format_first(nm) } else { format!("{} {}", nm.1, nm.0).trim().to_string() });
            }
            join(v, w.and)
        }
        CiteStyle::Ieee => {
            let v: Vec<String> = names.iter().map(|(l, f)| format!("{} {l}", initials(f)).trim().to_string()).collect();
            if n > 6 {
                format!("{} {}", v[0], w.et_al)
            } else {
                join(v, w.and)
            }
        }
    }
}

fn format_first(n: &(String, String)) -> String {
    if n.1.is_empty() {
        n.0.clone()
    } else {
        format!("{}, {}", n.0, n.1)
    }
}

fn link_of(s: &Source) -> String {
    if !s.doi.trim().is_empty() {
        let d = s.doi.trim().trim_start_matches("https://doi.org/");
        format!("https://doi.org/{d}")
    } else {
        s.url.trim().to_string()
    }
}

/// Wpis bibliografii jako kawałki tekstu (tytuły kursywą).
pub fn entry(doc: &Document, s: &Source, num: usize) -> Vec<Piece> {
    let style = doc.cite_style;
    let w = words(doc);
    let au = bib_authors(s, style, &w);
    let y = year(s, &w);
    let title = s.title.trim().to_string();
    let cont = s.container.trim().to_string();
    let link = link_of(s);
    let pub_place = match (s.place.trim(), s.publisher.trim()) {
        ("", "") => String::new(),
        ("", p) | (p, "") => p.to_string(),
        (pl, p) => format!("{pl}: {p}"),
    };
    let mut v: Vec<Piece> = Vec::new();
    match style {
        CiteStyle::Apa => {
            if !au.is_empty() {
                v.push(plain(format!("{} ", end_dot(&au))));
            }
            v.push(plain(format!("({y}). ")));
            match s.kind {
                SourceKind::Article => {
                    v.push(plain(format!("{} ", end_dot(&title))));
                    if !cont.is_empty() {
                        v.push(ital(cont));
                        if !s.volume.is_empty() {
                            v.push(plain(", "));
                            v.push(ital(s.volume.clone()));
                        }
                        if !s.issue.is_empty() {
                            v.push(plain(format!("({})", s.issue)));
                        }
                        if !s.pages.is_empty() {
                            v.push(plain(format!(", {}", dash(&s.pages))));
                        }
                        v.push(plain(". "));
                    }
                }
                SourceKind::Chapter => {
                    v.push(plain(format!("{} ", end_dot(&title))));
                    if !cont.is_empty() {
                        v.push(plain(if w.and == "i" { "W: " } else { "In " }));
                        v.push(ital(cont));
                        if !s.pages.is_empty() {
                            v.push(plain(format!(" ({} {})", w.pp, dash(&s.pages))));
                        }
                        v.push(plain(". "));
                    }
                    if !s.publisher.is_empty() {
                        v.push(plain(format!("{} ", end_dot(&s.publisher))));
                    }
                }
                _ => {
                    v.push(ital(end_dot(&title)));
                    v.push(plain(" "));
                    let by = if s.kind == SourceKind::Web { &cont } else { &s.publisher };
                    if !by.trim().is_empty() {
                        v.push(plain(format!("{} ", end_dot(by))));
                    }
                }
            }
            if !link.is_empty() {
                v.push(plain(link));
            }
        }
        CiteStyle::Mla => {
            if !au.is_empty() {
                v.push(plain(format!("{} ", end_dot(&au))));
            }
            if matches!(s.kind, SourceKind::Article | SourceKind::Chapter | SourceKind::Web) && !cont.is_empty() {
                v.push(plain(format!("„{}” ", end_dot(&title))));
                v.push(ital(cont));
                let mut tail = String::new();
                if !s.volume.is_empty() {
                    tail.push_str(&format!(", vol. {}", s.volume));
                }
                if !s.issue.is_empty() {
                    tail.push_str(&format!(", no. {}", s.issue));
                }
                if !s.publisher.is_empty() && s.kind != SourceKind::Article {
                    tail.push_str(&format!(", {}", s.publisher));
                }
                tail.push_str(&format!(", {y}"));
                if !s.pages.is_empty() {
                    tail.push_str(&format!(", {} {}", w.pp, dash(&s.pages)));
                }
                v.push(plain(format!("{tail}. ")));
            } else {
                v.push(ital(end_dot(&title)));
                let by = if s.publisher.is_empty() { String::new() } else { format!("{}, ", s.publisher) };
                v.push(plain(format!(" {by}{y}. ")));
            }
            if !link.is_empty() {
                v.push(plain(link.trim_start_matches("https://").to_string() + "."));
            }
        }
        CiteStyle::Chicago => {
            if !au.is_empty() {
                v.push(plain(format!("{} ", end_dot(&au))));
            }
            v.push(plain(format!("{}. ", y)));
            match s.kind {
                SourceKind::Article => {
                    v.push(plain(format!("„{}” ", end_dot(&title))));
                    v.push(ital(cont));
                    let mut tail = String::new();
                    if !s.volume.is_empty() {
                        tail.push_str(&format!(" {}", s.volume));
                    }
                    if !s.issue.is_empty() {
                        tail.push_str(&format!(" ({})", s.issue));
                    }
                    if !s.pages.is_empty() {
                        tail.push_str(&format!(": {}", dash(&s.pages)));
                    }
                    v.push(plain(format!("{tail}. ")));
                }
                SourceKind::Chapter => {
                    v.push(plain(format!("„{}” ", end_dot(&title))));
                    v.push(plain(if w.and == "i" { "W: " } else { "In " }));
                    v.push(ital(cont));
                    if !s.pages.is_empty() {
                        v.push(plain(format!(", {}", dash(&s.pages))));
                    }
                    v.push(plain(". "));
                    if !pub_place.is_empty() {
                        v.push(plain(format!("{} ", end_dot(&pub_place))));
                    }
                }
                _ => {
                    v.push(ital(end_dot(&title)));
                    v.push(plain(" "));
                    let by = if s.kind == SourceKind::Web { cont.clone() } else { pub_place.clone() };
                    if !by.is_empty() {
                        v.push(plain(format!("{} ", end_dot(&by))));
                    }
                }
            }
            if !link.is_empty() {
                v.push(plain(format!("{link}.")));
            }
        }
        CiteStyle::Ieee => {
            v.push(plain(format!("[{num}]\t")));
            if !au.is_empty() {
                v.push(plain(format!("{au}, ")));
            }
            match s.kind {
                SourceKind::Article | SourceKind::Chapter => {
                    v.push(plain(format!("„{title},” ")));
                    if s.kind == SourceKind::Chapter {
                        v.push(plain(if w.and == "i" { "w: " } else { "in " }));
                    }
                    v.push(ital(cont));
                    let mut tail = String::new();
                    if !s.volume.is_empty() {
                        tail.push_str(&format!(", vol. {}", s.volume));
                    }
                    if !s.issue.is_empty() {
                        tail.push_str(&format!(", no. {}", s.issue));
                    }
                    if !s.pages.is_empty() {
                        tail.push_str(&format!(", {} {}", w.pp, dash(&s.pages)));
                    }
                    v.push(plain(format!("{tail}, {y}.")));
                }
                SourceKind::Web => {
                    v.push(plain(format!("„{title}.” ")));
                    if !cont.is_empty() {
                        v.push(ital(cont));
                        v.push(plain(". "));
                    }
                    if !link.is_empty() {
                        v.push(plain(format!("[Online]. {link}")));
                    }
                    if !s.accessed.is_empty() {
                        v.push(plain(format!(" ({} {})", w.accessed, s.accessed)));
                    }
                    return v;
                }
                _ => {
                    v.push(ital(title));
                    let tail = if pub_place.is_empty() { format!(", {y}.") } else { format!(". {pub_place}, {y}.") };
                    v.push(plain(tail));
                }
            }
            if !link.is_empty() {
                v.push(plain(format!(" {link}")));
            }
        }
    }
    v
}

fn sort_key(s: &Source) -> Vec<(char, u8)> {
    let first = s.authors.first().map(|a| split_name(a).0).unwrap_or_else(|| s.title.clone());
    pl_key(&format!("{first} {} {}", s.year, s.title))
}

/// Kolejność źródeł w bibliografii: IEEE – według pierwszego cytowania (niecytowane na końcu),
/// pozostałe style – alfabetycznie według pierwszego autora.
pub fn order(doc: &Document) -> Vec<u64> {
    let cited = cited_order(doc);
    let mut ids: Vec<u64> = doc.sources.iter().map(|s| s.id).collect();
    if doc.cite_style == CiteStyle::Ieee {
        ids.sort_by_key(|id| cited.iter().position(|c| c == id).unwrap_or(usize::MAX));
    } else {
        ids.sort_by_cached_key(|id| doc.sources.iter().find(|s| s.id == *id).map(sort_key).unwrap_or_default());
    }
    ids
}

/// Akapity bibliografii (bez nagłówka).
pub fn bibliography(doc: &Document) -> Vec<Paragraph> {
    let ids = order(doc);
    ids.iter()
        .enumerate()
        .filter_map(|(i, id)| doc.sources.iter().find(|s| s.id == *id).map(|s| (i, s)))
        .map(|(i, s)| {
            let runs: Vec<Run> = entry(doc, s, i + 1)
                .into_iter()
                .filter(|(t, _)| !t.is_empty())
                .map(|(t, it)| Run { text: t, fmt: CharFmt { italic: it.then_some(true), ..Default::default() }, field: None })
                .collect();
            let mut p = Paragraph { style: BIB_STYLE.into(), fmt: ParaFmt::default(), runs };
            p.normalize();
            p
        })
        .collect()
}

pub const BIB_STYLE: &str = "bibliography";

/// Styl „Bibliografia” (wysunięcie pierwszego wiersza), jeśli dokument go nie ma.
pub fn ensure_style(doc: &mut Document) {
    if doc.style(BIB_STYLE).is_some() {
        return;
    }
    let mut para = ParaFmt::default();
    para.indent_left = Some(24.0);
    para.first_line = Some(-24.0);
    para.space_after = Some(6.0);
    para.tabs = Some(vec![24.0]);
    doc.styles.push(Style {
        id: BIB_STYLE.into(),
        name: "Bibliografia".into(),
        based_on: Some("normal".into()),
        next: None,
        para,
        chars: CharFmt::default(),
        outline: None,
        gallery: false,
    });
}

/// Nagłówek i akapity bibliografii do wstawienia.
pub fn bibliography_block(doc: &mut Document) -> Vec<Paragraph> {
    ensure_style(doc);
    let w = words(doc);
    let mut v = vec![Paragraph::with_text("heading1", w.heading)];
    v.extend(bibliography(doc));
    v
}

// ---------------------------------------------------------------- aktualizacja

fn cite_ids(link: &str) -> Option<Vec<u64>> {
    let rest = link.strip_prefix(CITE_PREFIX)?;
    Some(rest.split(',').filter_map(|x| x.trim().parse().ok()).collect())
}

/// Źródła w kolejności pierwszego cytowania (treść główna, potem przypisy i ramki).
pub fn cited_order(doc: &Document) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    for s in crate::stories(doc) {
        for p in &s.paras {
            for r in &p.runs {
                if let Some(ids) = r.fmt.link.as_deref().and_then(cite_ids) {
                    for id in ids {
                        if !out.contains(&id) {
                            out.push(id);
                        }
                    }
                }
            }
        }
    }
    out
}

/// Przebieg cytowania do wstawienia w miejscu kursora.
pub fn citation_run(doc: &Document, ids: &[u64]) -> Run {
    let mut order = cited_order(doc);
    for id in ids {
        if !order.contains(id) {
            order.push(*id);
        }
    }
    let num = |id: u64| order.iter().position(|x| *x == id).map(|i| i + 1).unwrap_or(0);
    let link = format!("{CITE_PREFIX}{}", ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","));
    Run { text: citation_text(doc, ids, &num), fmt: CharFmt { link: Some(link), ..Default::default() }, field: None }
}

/// Przelicza treść wszystkich cytowań i odświeża bibliografię (ciąg akapitów stylu
/// `bibliography`). Zwraca (liczba cytowań, czy bibliografia była w dokumencie).
pub fn update(doc: &mut Document) -> (usize, bool) {
    let order = cited_order(doc);
    let snapshot = doc.clone();
    let num = |id: u64| order.iter().position(|x| *x == id).map(|i| i + 1).unwrap_or(0);
    let mut count = 0;
    for s in crate::stories_mut(doc) {
        for p in s.paras.iter_mut() {
            let mut prev: Option<String> = None;
            let mut touched = false;
            for r in p.runs.iter_mut() {
                let link = r.fmt.link.clone();
                match link.as_deref().and_then(cite_ids) {
                    Some(ids) => {
                        if prev.as_deref() == link.as_deref() {
                            // Dalszy kawałek tego samego cytowania (np. po zmianie formatu) – znika.
                            r.text.clear();
                        } else {
                            r.text = citation_text(&snapshot, &ids, &num);
                            count += 1;
                        }
                        touched = true;
                    }
                    None => {}
                }
                prev = link;
            }
            if touched {
                p.normalize();
            }
        }
    }
    let start = doc.body.paras.iter().position(|p| p.style == BIB_STYLE);
    let had = start.is_some();
    if let Some(a) = start {
        let mut b = a;
        while b + 1 < doc.body.paras.len() && doc.body.paras[b + 1].style == BIB_STYLE {
            b += 1;
        }
        let fresh = bibliography(doc);
        if fresh.is_empty() {
            doc.body.paras.drain(a..=b);
            if doc.body.paras.is_empty() {
                doc.body.paras.push(Paragraph::default());
            }
        } else {
            doc.body.paras.splice(a..=b, fresh);
        }
    }
    (count, had)
}

// ---------------------------------------------------------------- import

fn kind_from(word: &str) -> SourceKind {
    let w = word.trim().to_lowercase();
    match w.as_str() {
        "article" | "jour" | "artykuł" | "artykul" | "czasopismo" | "mgzn" | "news" => SourceKind::Article,
        "incollection" | "inbook" | "inproceedings" | "conference" | "chap" | "rozdział" | "rozdzial" | "conf" | "cpaper" => SourceKind::Chapter,
        "online" | "misc-web" | "elec" | "web" | "strona" | "internet" | "blog" => SourceKind::Web,
        "techreport" | "report" | "rprt" | "raport" => SourceKind::Report,
        "phdthesis" | "mastersthesis" | "thes" | "thesis" | "praca" => SourceKind::Thesis,
        _ => SourceKind::Book,
    }
}

/// Polskie znaki zapisane po LaTeX-owemu (`{\l}`, `\'o`, `\k{a}`) i nawiasy klamrowe.
fn delatex(s: &str) -> String {
    let mut t = s.to_string();
    for (a, b) in [
        ("\\k{a}", "ą"),
        ("\\k{e}", "ę"),
        ("\\k{A}", "Ą"),
        ("\\k{E}", "Ę"),
        ("{\\l}", "ł"),
        ("{\\L}", "Ł"),
        ("\\l{}", "ł"),
        ("\\L{}", "Ł"),
        ("\\l ", "ł"),
        ("\\L ", "Ł"),
        ("\\.z", "ż"),
        ("\\.Z", "Ż"),
        ("\\.{z}", "ż"),
        ("\\.{Z}", "Ż"),
        ("--", "–"),
        ("\\&", "&"),
    ] {
        t = t.replace(a, b);
    }
    for (c, r) in [('o', 'ó'), ('O', 'Ó'), ('s', 'ś'), ('S', 'Ś'), ('c', 'ć'), ('C', 'Ć'), ('n', 'ń'), ('N', 'Ń'), ('z', 'ź'), ('Z', 'Ź'), ('e', 'é'), ('a', 'á')] {
        t = t.replace(&format!("\\'{c}"), &r.to_string()).replace(&format!("\\'{{{c}}}"), &r.to_string());
    }
    t.replace(['{', '}'], "").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn set_field(s: &mut Source, key: &str, val: String) {
    match key {
        "author" | "editor" if s.authors.is_empty() || key == "author" => {
            s.authors = val.split(" and ").map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
        }
        "title" => s.title = val,
        "journal" | "journaltitle" | "booktitle" => s.container = val,
        "howpublished" | "organization" | "institution" | "school" if s.publisher.is_empty() => s.publisher = val,
        "publisher" => s.publisher = val,
        "address" | "location" => s.place = val,
        "year" => s.year = val,
        "date" if s.year.is_empty() => s.year = val.chars().take(4).collect(),
        "volume" => s.volume = val,
        "number" | "issue" => s.issue = val,
        "pages" => s.pages = val.replace("--", "–"),
        "url" => s.url = val,
        "doi" => s.doi = val,
        "urldate" => s.accessed = val,
        _ => {}
    }
}

/// Źródła z pliku BibTeX / BibLaTeX.
pub fn parse_bibtex(text: &str) -> Vec<Source> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let mut i = 0;
    while let Some(at) = text[i..].find('@') {
        i += at + 1;
        let Some(open) = text[i..].find(['{', '(']) else { break };
        let typ = text[i..i + open].trim().to_lowercase();
        i += open + 1;
        if matches!(typ.as_str(), "comment" | "string" | "preamble") {
            continue;
        }
        // Treść wpisu do zamykającego nawiasu na poziomie 0.
        let mut depth = 1;
        let start = i;
        while i < b.len() && depth > 0 {
            match b[i] {
                b'{' | b'(' => depth += 1,
                b'}' | b')' => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        let body = &text[start..i.saturating_sub(1)];
        let mut src = Source { kind: kind_from(&typ), ..Default::default() };
        if typ == "misc" || typ == "online" {
            src.kind = SourceKind::Web;
        }
        // Pola: klucz = {…} | "…" | liczba, rozdzielone przecinkami na poziomie 0.
        let rest = body.split_once(',').map(|x| x.1).unwrap_or("");
        let rb = rest.as_bytes();
        let mut k = 0;
        while k < rb.len() {
            let Some(eq) = rest[k..].find('=') else { break };
            let key = rest[k..k + eq].trim().trim_start_matches(',').trim().to_lowercase();
            k += eq + 1;
            while k < rb.len() && rb[k].is_ascii_whitespace() {
                k += 1;
            }
            let val;
            if k < rb.len() && (rb[k] == b'{' || rb[k] == b'"') {
                let close = if rb[k] == b'{' { b'}' } else { b'"' };
                let mut d = 0;
                let s0 = k + 1;
                let mut j = k;
                loop {
                    if j >= rb.len() {
                        break;
                    }
                    let c = rb[j];
                    if c == b'{' {
                        d += 1;
                    } else if c == b'}' {
                        d -= 1;
                    }
                    if j > k && ((close == b'}' && d == 0 && c == b'}') || (close == b'"' && c == b'"' && d == 0)) {
                        break;
                    }
                    j += 1;
                }
                val = rest[s0..j.min(rb.len())].to_string();
                k = j + 1;
            } else {
                let e = rest[k..].find(',').map(|x| k + x).unwrap_or(rb.len());
                val = rest[k..e].trim().to_string();
                k = e;
            }
            set_field(&mut src, &key, delatex(&val));
        }
        if !src.title.is_empty() || !src.authors.is_empty() {
            out.push(src);
        }
    }
    out
}

/// Źródła z pliku RIS.
pub fn parse_ris(text: &str) -> Vec<Source> {
    let mut out = Vec::new();
    let mut cur: Option<Source> = None;
    let (mut sp, mut ep) = (String::new(), String::new());
    for line in text.lines() {
        if line.get(2..5) != Some("  -") {
            continue;
        }
        let Some(tag) = line.get(..2) else { continue };
        let val = line.get(6..).unwrap_or("").trim().to_string();
        match tag {
            "TY" => {
                cur = Some(Source { kind: kind_from(&val), ..Default::default() });
                sp.clear();
                ep.clear();
            }
            "ER" => {
                if let Some(mut s) = cur.take() {
                    s.pages = match (sp.is_empty(), ep.is_empty()) {
                        (false, false) => format!("{sp}–{ep}"),
                        (false, true) => sp.clone(),
                        _ => s.pages,
                    };
                    if !s.title.is_empty() || !s.authors.is_empty() {
                        out.push(s);
                    }
                }
            }
            _ => {
                let Some(s) = cur.as_mut() else { continue };
                match tag {
                    "AU" | "A1" => s.authors.push(val),
                    "TI" | "T1" => s.title = val,
                    "T2" | "JO" | "JF" | "JA" | "BT" => {
                        if s.container.is_empty() {
                            s.container = val;
                        }
                    }
                    "PB" => s.publisher = val,
                    "CY" => s.place = val,
                    "PY" | "Y1" | "DA" => {
                        if s.year.is_empty() {
                            s.year = val.chars().take(4).collect();
                        }
                    }
                    "VL" => s.volume = val,
                    "IS" => s.issue = val,
                    "SP" => sp = val,
                    "EP" => ep = val,
                    "UR" => s.url = val,
                    "DO" => s.doi = val,
                    "Y2" => s.accessed = val,
                    _ => {}
                }
            }
        }
    }
    out
}

/// Źródło z formularza „klucz: wartość” (po polsku albo po angielsku), autorzy po średniku.
pub fn parse_form(text: &str) -> Option<Source> {
    let mut s = Source::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once(':') else { continue };
        let v = v.trim().to_string();
        if v.is_empty() {
            continue;
        }
        match k.trim().to_lowercase().as_str() {
            "rodzaj" | "type" => s.kind = kind_from(&v),
            "autorzy" | "autor" | "authors" | "author" => s.authors = v.split(';').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect(),
            "tytuł" | "tytul" | "title" => s.title = v,
            "czasopismo" | "w" | "książka" | "serwis" | "journal" | "container" => s.container = v,
            "wydawca" | "publisher" => s.publisher = v,
            "miejsce" | "place" => s.place = v,
            "rok" | "year" => s.year = v,
            "tom" | "volume" => s.volume = v,
            "numer" | "issue" => s.issue = v,
            "strony" | "pages" => s.pages = v,
            "url" | "adres" => s.url = v,
            "doi" => s.doi = v,
            "dostęp" | "dostep" | "accessed" => s.accessed = v,
            _ => {}
        }
    }
    (!s.title.is_empty()).then_some(s)
}

/// Dodaje źródła, pomijając te, które już są (ten sam tytuł i rok); zwraca liczbę dodanych.
pub fn add_sources(doc: &mut Document, new: Vec<Source>) -> usize {
    let mut n = 0;
    for mut s in new {
        let dup = doc.sources.iter().any(|x| x.title.to_lowercase() == s.title.to_lowercase() && x.year == s.year);
        if dup {
            continue;
        }
        s.id = doc.new_id();
        doc.sources.push(s);
        n += 1;
    }
    n
}

/// Krótki opis źródła do listy w oknie („Kowalski, 2020 – Tytuł”).
pub fn label(s: &Source) -> String {
    let who = s.authors.first().map(|a| split_name(a).0).unwrap_or_default();
    let more = if s.authors.len() > 1 { " i in." } else { "" };
    format!("{who}{more}, {} – {}", if s.year.is_empty() { "b.d." } else { s.year.as_str() }, s.title)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(style: CiteStyle) -> Document {
        let mut d = Document::new();
        d.settings.lang = "pl".into();
        d.cite_style = style;
        let n = add_sources(
            &mut d,
            parse_bibtex(
                r#"@book{k2020, author = {Kowalski, Jan and Nowak, Anna}, title = {Sk{\l}ad tekstu}, publisher = {PWN}, address = {Warszawa}, year = 2020}
@article{z19, author = "Zieli\'nska, Ewa", title = {Typografia}, journal = {Prace}, volume = {12}, number = {3}, pages = {45--67}, year = {2019}, doi = {10.1/x}}"#,
            ),
        );
        assert_eq!(n, 2);
        d
    }

    #[test]
    fn bibtex_polish_letters() {
        let d = doc(CiteStyle::Apa);
        assert_eq!(d.sources[0].title, "Skład tekstu");
        assert_eq!(d.sources[1].authors, ["Zielińska, Ewa"]);
        assert_eq!(d.sources[1].pages, "45–67");
    }

    #[test]
    fn apa_citation_and_entry() {
        let d = doc(CiteStyle::Apa);
        let id = d.sources[0].id;
        assert_eq!(citation_run(&d, &[id]).text, "(Kowalski i Nowak, 2020)");
        let e: String = entry(&d, &d.sources[1], 1).into_iter().map(|x| x.0).collect();
        assert_eq!(e, "Zielińska, E. (2019). Typografia. Prace, 12(3), 45–67. https://doi.org/10.1/x");
    }

    #[test]
    fn ieee_numbers_follow_citation_order() {
        let mut d = doc(CiteStyle::Ieee);
        let (a, b) = (d.sources[0].id, d.sources[1].id);
        d.body.paras[0].runs = vec![citation_run(&d, &[b]), Run { text: " i ".into(), ..Default::default() }, citation_run(&d, &[a])];
        ensure_style(&mut d);
        d.body.paras.push(Paragraph::with_text(BIB_STYLE, "x"));
        let (n, had) = update(&mut d);
        assert_eq!((n, had), (2, true));
        assert_eq!(d.body.paras[0].text(), "[1] i [2]");
        assert!(d.body.paras[1].text().starts_with("[1]\tE. Zielińska"));
        assert_eq!(d.body.paras.len(), 3);
    }

    #[test]
    fn ris_and_form() {
        let r = parse_ris("TY  - JOUR\nAU  - Lis, Adam\nTI  - O kerningu\nJO  - Typo\nPY  - 2021/05/01\nSP  - 1\nEP  - 9\nER  - \n");
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].year.as_str(), r[0].pages.as_str(), r[0].kind), ("2021", "1–9", SourceKind::Article));
        let f = parse_form("Rodzaj: strona\nAutorzy: Lis, Adam; Kot, Ola\nTytuł: Fonty\nURL: https://x.pl").unwrap();
        assert_eq!(f.authors.len(), 2);
        assert_eq!(f.kind, SourceKind::Web);
    }
}
