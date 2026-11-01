//! Korespondencja seryjna: w treści dokumentu pola `«Nazwa»` albo `{{Nazwa}}` (nazwa kolumny CSV).
//! Wynikiem jest jeden dokument – kopia treści dla każdego wiersza danych, każda od nowej strony –
//! albo osobny dokument dla każdego wiersza. W scalonym dokumencie nagłówki i stopki są wspólne
//! dla wszystkich stron, więc pola zostają w nich bez zmian.

use crate::csv::Table;
use crate::model::*;

/// Nazwy pól użytych w treści (bez powtórzeń, w kolejności wystąpienia).
pub fn fields(doc: &Document) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in crate::stories(doc) {
        for p in &s.paras {
            for r in &p.runs {
                for name in names(&r.text) {
                    if !out.contains(&name) {
                        out.push(name);
                    }
                }
            }
        }
    }
    out
}

fn names(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (open, close) in [("«", "»"), ("{{", "}}")] {
        let mut rest = text;
        while let Some(a) = rest.find(open) {
            let after = &rest[a + open.len()..];
            let Some(b) = after.find(close) else { break };
            let name = after[..b].trim();
            if !name.is_empty() && name.len() < 64 && !name.contains(['«', '{', '\n']) {
                out.push(name.to_string());
            }
            rest = &after[b + close.len()..];
        }
    }
    out
}

/// Tekst z polami zastąpionymi wartościami wiersza `row` (pola bez kolumny zostają).
pub fn fill(text: &str, data: &Table, row: usize) -> String {
    let mut out = text.to_string();
    for name in names(text) {
        if let Some(v) = data.get(row, &name) {
            for (open, close) in [("«", "»"), ("{{", "}}")] {
                for spaced in [format!("{open}{name}{close}"), format!("{open} {name} {close}")] {
                    out = out.replace(&spaced, v);
                }
            }
        }
    }
    out
}

fn fill_story(s: &mut Story, data: &Table, row: usize) {
    for p in s.paras.iter_mut() {
        // Pole wpisane jednym ciągiem bywa rozbite na przebiegi o tym samym formacie – scalamy je.
        p.normalize();
        for r in p.runs.iter_mut().filter(|r| r.field.is_none()) {
            if r.text.contains('«') || r.text.contains("{{") {
                r.text = fill(&r.text, data, row);
            }
        }
    }
}

/// Dokument scalony: treść powtórzona dla każdego wiersza `data`, ramki tekstowe z pierwszej
/// strony wypełnione wartościami pierwszego wiersza.
pub fn merge(doc: &Document, data: &Table) -> Document {
    let mut out = doc.clone();
    out.versions.clear();
    out.version_data.clear();
    let mut paras: Vec<Paragraph> = Vec::new();
    for row in 0..data.rows.len() {
        let mut s = doc.body.clone();
        fill_story(&mut s, data, row);
        if row > 0 {
            if let Some(first) = s.paras.first_mut() {
                first.fmt.break_before = Some(Break::Page);
            }
        }
        paras.extend(s.paras);
    }
    if !paras.is_empty() {
        out.body.paras = paras;
    }
    if !data.rows.is_empty() {
        for o in out.objects.iter_mut() {
            if let Some(s) = o.story_mut() {
                fill_story(s, data, 0);
            }
        }
        for n in out.notes.iter_mut() {
            fill_story(&mut n.story, data, 0);
        }
    }
    out
}

/// Osobny dokument dla każdego wiersza (np. do zapisania jako oddzielne pliki).
pub fn merge_each(doc: &Document, data: &Table) -> Vec<Document> {
    (0..data.rows.len())
        .map(|row| {
            let mut d = doc.clone();
            d.versions.clear();
            d.version_data.clear();
            for s in crate::stories_mut(&mut d) {
                fill_story(s, data, row);
            }
            d
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv::parse;

    #[test]
    fn fills_fields_and_breaks_pages() {
        let mut doc = Document::new();
        doc.body = Story::plain("Szanowna Pani «Imię»,\nzapraszamy do {{ Miasto }}.", "normal");
        let data = parse("Imię;Miasto\nAnna;Gdańska\nEwa;Torunia\n");
        assert_eq!(fields(&doc), ["Imię", "Miasto"]);
        let m = merge(&doc, &data);
        assert_eq!(m.body.paras.len(), 4);
        assert_eq!(m.body.paras[0].text(), "Szanowna Pani Anna,");
        assert_eq!(m.body.paras[3].text(), "zapraszamy do Torunia.");
        assert_eq!(m.body.paras[2].fmt.break_before, Some(Break::Page));
        assert_eq!(merge_each(&doc, &data).len(), 2);
    }
}
