//! Tabele: sortowanie według kolumny (liczby jako liczby, tekst w polskim porządku), zamiana
//! tekstu w tabelę i tabeli w tekst, suma kolumny.

use std::cmp::Ordering;

use crate::model::*;

/// Liczba z komórki: „1 200,50 zł”, „-3.5”, „12%”.
pub fn number(s: &str) -> Option<f64> {
    let t: String = s.chars().filter(|c| !c.is_whitespace() && *c != '\u{a0}').collect();
    let t = t.trim_end_matches(|c: char| c.is_alphabetic() || "%$€£".contains(c));
    let t = t.trim_start_matches(|c: char| c.is_alphabetic() || "$€£".contains(c));
    if t.is_empty() {
        return None;
    }
    let t = if t.contains(',') && !t.contains('.') { t.replace(',', ".") } else { t.replace(',', "") };
    t.parse::<f64>().ok()
}

/// Klucz polskiego porządku alfabetycznego (ą po a, ł po l, ż po ź…).
pub fn pl_key(s: &str) -> Vec<(char, u8)> {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'ą' => ('a', 1),
            'ć' => ('c', 1),
            'ę' => ('e', 1),
            'ł' => ('l', 1),
            'ń' => ('n', 1),
            'ó' => ('o', 1),
            'ś' => ('s', 1),
            'ź' => ('z', 1),
            'ż' => ('z', 2),
            c => (c, 0),
        })
        .collect()
}

fn cmp_cells(a: &str, b: &str) -> Ordering {
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        _ => pl_key(a).cmp(&pl_key(b)),
    }
}

/// Sortuje wiersze tabeli według kolumny `col`; wiersz nagłówka zostaje na górze. Sortowanie
/// stabilne – równe wartości zachowują kolejność.
pub fn sort(t: &mut Table, col: usize, desc: bool) {
    let skip = usize::from(t.header).min(t.rows.len());
    let body = &mut t.rows[skip..];
    body.sort_by(|a, b| {
        let o = cmp_cells(a.get(col).map(|s| s.as_str()).unwrap_or(""), b.get(col).map(|s| s.as_str()).unwrap_or(""));
        if desc {
            o.reverse()
        } else {
            o
        }
    });
}

/// Suma liczb w kolumnie (bez nagłówka); `None`, gdy nie ma w niej żadnej liczby.
pub fn column_sum(t: &Table, col: usize) -> Option<f64> {
    let skip = usize::from(t.header);
    let vals: Vec<f64> = t.rows.iter().skip(skip).filter_map(|r| r.get(col).and_then(|c| number(c))).collect();
    (!vals.is_empty()).then(|| vals.iter().sum())
}

/// Separator kolumn w zaznaczonym tekście: tabulator, średnik albo przecinek (ten, który dzieli
/// najwięcej wierszy na tyle samo części).
pub fn guess_separator(lines: &[String]) -> char {
    let mut best = ('\t', 0usize);
    for d in ['\t', ';', ','] {
        let counts: Vec<usize> = lines.iter().map(|l| l.matches(d).count()).collect();
        let first = counts.first().copied().unwrap_or(0);
        if first == 0 {
            continue;
        }
        let same = counts.iter().filter(|&&c| c == first).count();
        if same > best.1 {
            best = (d, same);
        }
    }
    best.0
}

/// Akapity (po jednym wierszu) na wiersze tabeli.
pub fn text_to_rows(lines: &[String]) -> Vec<Vec<String>> {
    let d = guess_separator(lines);
    let mut rows: Vec<Vec<String>> = lines.iter().filter(|l| !l.trim().is_empty()).map(|l| l.split(d).map(|c| c.trim().to_string()).collect()).collect();
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
    for r in rows.iter_mut() {
        r.resize(cols, String::new());
    }
    rows
}

/// Wiersze tabeli jako akapity z kolumnami oddzielonymi tabulatorem (komórki scalone pomijamy).
pub fn rows_to_text(t: &Table) -> Vec<String> {
    t.rows.iter().map(|r| r.iter().filter(|c| c.as_str() != MERGED).cloned().collect::<Vec<_>>().join("\t")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(number("1 200,50 zł"), Some(1200.5));
        assert_eq!(number("-3.5"), Some(-3.5));
        assert_eq!(number("12%"), Some(12.0));
        assert_eq!(number("1,234.5"), Some(1234.5));
        assert_eq!(number("abc"), None);
    }

    #[test]
    fn sorts_below_header() {
        let mut t = Table::default();
        t.rows = vec![vec!["Nazwa".into(), "Ile".into()], vec!["żurek".into(), "10".into()], vec!["łosoś".into(), "2".into()], vec!["arbuz".into(), "100".into()]];
        sort(&mut t, 1, false);
        assert_eq!(t.rows.iter().map(|r| r[1].as_str()).collect::<Vec<_>>(), ["Ile", "2", "10", "100"]);
        sort(&mut t, 0, false);
        assert_eq!(t.rows.iter().map(|r| r[0].as_str()).collect::<Vec<_>>(), ["Nazwa", "arbuz", "łosoś", "żurek"]);
        assert_eq!(column_sum(&t, 1), Some(112.0));
    }

    #[test]
    fn text_round_trip() {
        let lines = vec!["a;b;c".to_string(), "1;2".to_string()];
        let rows = text_to_rows(&lines);
        assert_eq!(rows, vec![vec!["a", "b", "c"], vec!["1", "2", ""]]);
    }
}
