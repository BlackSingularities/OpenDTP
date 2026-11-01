//! Czytanie CSV do korespondencji seryjnej: separator wykrywany sam (średnik z polskiego Excela,
//! przecinek, tabulator), pola w cudzysłowach (także z podziałem wiersza), BOM, UTF-8 albo
//! Windows-1250.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// Wartość kolumny `name` (bez rozróżniania wielkości liter) w wierszu `row`.
    pub fn get(&self, row: usize, name: &str) -> Option<&str> {
        let col = self.headers.iter().position(|h| h.eq_ignore_ascii_case(name) || h.to_lowercase() == name.to_lowercase())?;
        self.rows.get(row).map(|r| r.get(col).map(|s| s.as_str()).unwrap_or(""))
    }
}

/// Bajty pliku na tekst: UTF-8 (z BOM albo bez), w przeciwnym razie Windows-1250.
pub fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| crate::util::cp1250(b)).collect(),
    }
}

fn delimiter(text: &str) -> char {
    let first = text.lines().next().unwrap_or("");
    [';', '\t', ','].into_iter().max_by_key(|d| first.matches(*d).count()).filter(|d| first.contains(*d)).unwrap_or(';')
}

/// Pierwszy wiersz to nazwy kolumn; puste wiersze są pomijane.
pub fn parse(text: &str) -> Table {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let d = delimiter(text);
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut rec: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if field.trim().is_empty() => {
                field.clear();
                quoted = true;
            }
            '\r' => {}
            '\n' => {
                rec.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut rec));
            }
            c if c == d => rec.push(std::mem::take(&mut field)),
            c => field.push(c),
        }
    }
    if !field.is_empty() || !rec.is_empty() {
        rec.push(field);
        records.push(rec);
    }
    records.retain(|r| r.iter().any(|f| !f.trim().is_empty()));
    let mut it = records.into_iter();
    let headers: Vec<String> = it.next().unwrap_or_default().into_iter().map(|h| h.trim().to_string()).collect();
    Table { headers, rows: it.map(|r| r.into_iter().map(|f| f.trim().to_string()).collect()).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semicolons_quotes_and_newlines() {
        let t = parse("Imię;Miasto;Uwagi\r\nAnna;Gdańsk;\"ma \"\"kota\"\"\"\nJan;\"Kraków\";\"dwa\nwiersze\"\n\n");
        assert_eq!(t.headers, ["Imię", "Miasto", "Uwagi"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0][2], "ma \"kota\"");
        assert_eq!(t.rows[1][2], "dwa\nwiersze");
        assert_eq!(t.get(1, "miasto"), Some("Kraków"));
    }

    #[test]
    fn commas_and_cp1250() {
        let t = parse(&decode(b"a,b\n\xb3\xf3d\xbc,x\n"));
        assert_eq!(t.headers, ["a", "b"]);
        assert!(t.rows[0][0].starts_with("łód"));
    }
}
