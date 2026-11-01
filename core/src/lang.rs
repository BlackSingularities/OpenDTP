//! Języki interfejsu. Tekstem źródłowym jest polski; tłumaczenia leżą w `lang/<kod>/LC_MESSAGES/opendtp.po`
//! (gettext – ten sam plik czyta Slint dla `@tr` w plikach .slint i ten moduł dla tekstów z Rusta).
//! Nowy język = nowy folder z plikiem .po (wzór: `lang/opendtp.pot`); build.rs osadza wszystkie
//! znalezione. Nazwę języka do listy w Ustawieniach daje nagłówek `X-Language-Name`.
//!
//! `tr("…")` – zwykły tekst, `trf!("… {x} …", …)` – zamiast `format!` (tłumaczenie może przestawić
//! miejsca `{x}`; `{}` bez nazwy liczą się po kolei). Brak tłumaczenia – tekst polski.

use std::collections::HashMap;
use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/langs.rs"));

/// Język wbudowany (tekst źródłowy).
pub const SOURCE: &str = "pl";

static TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
static CURRENT: OnceLock<String> = OnceLock::new();

/// Dostępne języki: (kod, nazwa), polski pierwszy.
pub fn available() -> Vec<(String, String)> {
    let mut out = vec![(SOURCE.to_string(), "Polski".to_string())];
    for (code, po) in LANGS {
        let name = parse(po).1.unwrap_or_else(|| code.to_string());
        out.push((code.to_string(), name));
    }
    out
}

/// Kod języka interfejsu Windows (np. „en-US”).
fn system_language() -> String {
    use windows::Win32::Globalization::{GetUserDefaultUILanguage, LCIDToLocaleName};
    let mut buf = [0u16; 85];
    let n = unsafe { LCIDToLocaleName(GetUserDefaultUILanguage() as u32, Some(&mut buf), 0) };
    if n <= 1 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..n as usize - 1])
}

/// Wybór języka – raz, przy starcie, przed zbudowaniem interfejsu. `pref` z ustawień: pusty –
/// język Windows (polski, gdy go nie mamy, a Windows jest po polsku; inaczej angielski, jeśli jest).
/// Zwraca kod wybranego języka.
pub fn init(pref: &str) -> String {
    let codes: Vec<&str> = LANGS.iter().map(|(c, _)| *c).collect();
    let base = |l: &str| l.split(['-', '_']).next().unwrap_or("").to_lowercase();
    let pick = if !pref.is_empty() {
        pref.to_string()
    } else {
        let sys = system_language();
        let b = base(&sys);
        if b == SOURCE || b.is_empty() {
            SOURCE.to_string()
        } else if let Some(c) = codes.iter().find(|c| c.eq_ignore_ascii_case(&sys) || base(c) == b) {
            c.to_string()
        } else if codes.contains(&"en") {
            "en".to_string()
        } else {
            SOURCE.to_string()
        }
    };
    let table = LANGS.iter().find(|(c, _)| *c == pick).map(|(_, po)| parse(po).0).unwrap_or_default();
    let code = if table.is_empty() && pick != SOURCE && !codes.contains(&pick.as_str()) { SOURCE.to_string() } else { pick };
    let _ = TABLE.set(table);
    let _ = CURRENT.set(code.clone());
    code
}

/// Bieżący język (kod); przed `init` – polski.
pub fn current() -> &'static str {
    CURRENT.get().map(|s| s.as_str()).unwrap_or(SOURCE)
}

/// Tłumaczenie tekstu; brak – tekst bez zmian.
pub fn tr(s: &'static str) -> &'static str {
    match TABLE.get().and_then(|t| t.get(s)) {
        Some(v) => v.as_str(),
        None => s,
    }
}

/// Tłumaczenie tekstu poza `&'static str` (np. złożonego w czasie działania).
pub fn tr_owned(s: &str) -> String {
    TABLE.get().and_then(|t| t.get(s)).cloned().unwrap_or_else(|| s.to_string())
}

/// `format!` z tłumaczeniem – zob. opis modułu.
#[macro_export]
macro_rules! trf {
    ($lit:literal $($rest:tt)*) => {{
        let __src: &'static str = $lit;
        let __out = format!($lit $($rest)*);
        let __t = $crate::lang::tr(__src);
        if std::ptr::eq(__t, __src) { __out } else { $crate::lang::subst(__src, __t, &__out) }
    }};
}

#[derive(Debug, PartialEq)]
enum Seg {
    Text(String),
    Ph(String),
}

/// Wzór `format!` na kawałki: tekst i miejsca (`{nazwa}`, `{0}`; `{}` – kolejny numer; specyfikacja po `:` pomijana).
fn segments(t: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut auto = 0usize;
    let mut it = t.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                text.push('{');
            }
            '}' if it.peek() == Some(&'}') => {
                it.next();
                text.push('}');
            }
            '{' => {
                let mut inner = String::new();
                for d in it.by_ref() {
                    if d == '}' {
                        break;
                    }
                    inner.push(d);
                }
                let name = inner.split(':').next().unwrap_or("").trim().to_string();
                let key = if name.is_empty() {
                    auto += 1;
                    (auto - 1).to_string()
                } else {
                    name
                };
                out.push(Seg::Text(std::mem::take(&mut text)));
                out.push(Seg::Ph(key));
            }
            _ => text.push(c),
        }
    }
    out.push(Seg::Text(text));
    out
}

/// Wartości miejsc odczytane z gotowego tekstu (`out` = `src` po `format!`) wstawione we wzór tłumaczenia.
#[doc(hidden)]
pub fn subst(src: &str, translated: &str, out: &str) -> String {
    let segs = segments(src);
    let mut vals: HashMap<String, String> = HashMap::new();
    let mut rest = out;
    let mut i = 0;
    while i < segs.len() {
        match &segs[i] {
            Seg::Text(t) => match rest.strip_prefix(t.as_str()) {
                Some(r) => rest = r,
                None => return out.to_string(),
            },
            Seg::Ph(k) => {
                // Wartość sięga do następnego niepustego tekstu wzoru (albo do końca).
                let next = segs[i + 1..].iter().find_map(|s| match s {
                    Seg::Text(t) if !t.is_empty() => Some(t.as_str()),
                    _ => None,
                });
                let end = match next {
                    Some(n) => match rest.find(n) {
                        Some(e) => e,
                        None => return out.to_string(),
                    },
                    None => rest.len(),
                };
                vals.insert(k.clone(), rest[..end].to_string());
                rest = &rest[end..];
            }
        }
        i += 1;
    }
    let mut res = String::new();
    for s in segments(translated) {
        match s {
            Seg::Text(t) => res.push_str(&t),
            Seg::Ph(k) => res.push_str(vals.get(&k).map(|s| s.as_str()).unwrap_or("")),
        }
    }
    res
}

/// Plik .po: tłumaczenia (msgid → msgstr, puste pomijane) i nazwa języka z nagłówka.
fn parse(po: &str) -> (HashMap<String, String>, Option<String>) {
    let mut map = HashMap::new();
    let mut name = None;
    let (mut id, mut msg, mut field) = (String::new(), String::new(), 0u8);
    let flush = |id: &mut String, msg: &mut String, map: &mut HashMap<String, String>, name: &mut Option<String>| {
        if id.is_empty() {
            if let Some(l) = msg.lines().find_map(|l| l.strip_prefix("X-Language-Name:")) {
                *name = Some(l.trim().to_string());
            }
        } else if !msg.is_empty() {
            map.insert(std::mem::take(id), std::mem::take(msg));
        }
        id.clear();
        msg.clear();
    };
    for line in po.lines() {
        let l = line.trim();
        if let Some(r) = l.strip_prefix("msgid ") {
            if field == 2 {
                flush(&mut id, &mut msg, &mut map, &mut name);
            }
            id = unquote(r);
            field = 1;
        } else if let Some(r) = l.strip_prefix("msgstr ") {
            msg = unquote(r);
            field = 2;
        } else if l.starts_with('"') {
            match field {
                1 => id.push_str(&unquote(l)),
                2 => msg.push_str(&unquote(l)),
                _ => {}
            }
        } else if l.is_empty() && field == 2 {
            flush(&mut id, &mut msg, &mut map, &mut name);
            field = 0;
        }
    }
    if field == 2 {
        flush(&mut id, &mut msg, &mut map, &mut name);
    }
    (map, name)
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(s);
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subst_reorders_named() {
        let out = format!("Nie udało się {what}: {e}", what = "otworzyć", e = "brak");
        assert_eq!(subst("Nie udało się {what}: {e}", "Error {e} while {what}", &out), "Error brak while otworzyć");
    }

    #[test]
    fn subst_positional() {
        let out = format!("Pliki: {} z {}", 3, 7);
        assert_eq!(subst("Pliki: {} z {}", "{0} of {1} files", &out), "3 of 7 files");
    }

    #[test]
    fn parse_po() {
        let (m, n) = parse("msgid \"\"\nmsgstr \"\"\n\"X-Language-Name: English\\n\"\n\nmsgid \"Kopiuj\"\nmsgstr \"Copy\"\n");
        assert_eq!(n.as_deref(), Some("English"));
        assert_eq!(m.get("Kopiuj").map(|s| s.as_str()), Some("Copy"));
    }
}
