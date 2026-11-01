//! Operacje edycji na historii tekstu: wstawianie, usuwanie, dzielenie i łączenie akapitów,
//! formatowanie zakresów, fragmenty do schowka oraz ruchy kursora po znakach i wyrazach.

use unicode_segmentation::UnicodeSegmentation;

use crate::model::*;

/// Miejsce w tekście: akapit i przesunięcie w bajtach.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct At {
    pub para: usize,
    pub off: usize,
}

impl At {
    pub fn new(para: usize, off: usize) -> At {
        At { para, off }
    }
}

pub fn clamp(s: &Story, a: At) -> At {
    let para = a.para.min(s.paras.len().saturating_sub(1));
    let text = s.paras[para].text();
    let mut off = a.off.min(text.len());
    while off > 0 && !text.is_char_boundary(off) {
        off -= 1;
    }
    At { para, off }
}

/// Wstawia tekst (znaki nowego wiersza tworzą nowe akapity). Zwraca miejsce za wstawionym tekstem.
pub fn insert_text(s: &mut Story, at: At, text: &str, fmt: &CharFmt) -> At {
    let mut at = clamp(s, at);
    let mut first = true;
    for part in text.split('\n') {
        if !first {
            at = split(s, at);
        }
        first = false;
        if part.is_empty() {
            continue;
        }
        let p = &mut s.paras[at.para];
        let i = p.split_at(at.off);
        p.runs.insert(i, Run { text: part.to_string(), fmt: fmt.clone(), field: None });
        p.normalize();
        at.off += part.len();
    }
    at
}

/// Wstawia pole (numer strony itp.).
pub fn insert_field(s: &mut Story, at: At, f: Field, fmt: &CharFmt) -> At {
    let at = clamp(s, at);
    let p = &mut s.paras[at.para];
    let i = p.split_at(at.off);
    p.runs.insert(i, Run { text: FIELD_CHAR.to_string(), fmt: fmt.clone(), field: Some(f) });
    p.normalize();
    At { para: at.para, off: at.off + FIELD_CHAR.len_utf8() }
}

/// Dzieli akapit w miejscu `at`; nowy akapit dziedziczy styl i formatowanie akapitu.
pub fn split(s: &mut Story, at: At) -> At {
    let at = clamp(s, at);
    let p = &mut s.paras[at.para];
    let i = p.split_at(at.off);
    let tail: Vec<Run> = p.runs.drain(i..).collect();
    let end_fmt = p.runs.last().map(|r| r.fmt.clone()).or_else(|| tail.first().map(|r| r.fmt.clone())).unwrap_or_default();
    p.normalize();
    let mut np = Paragraph { style: p.style.clone(), fmt: p.fmt.clone(), runs: tail };
    np.fmt.break_before = None;
    if np.runs.is_empty() {
        np.runs.push(Run { text: String::new(), fmt: end_fmt, field: None });
    }
    np.normalize();
    s.paras.insert(at.para + 1, np);
    At { para: at.para + 1, off: 0 }
}

/// Usuwa tekst od `a` do `b` (a ≤ b). Zwraca `a`.
pub fn delete(s: &mut Story, a: At, b: At) -> At {
    let a = clamp(s, a);
    let b = clamp(s, b);
    if a >= b {
        return a;
    }
    if a.para == b.para {
        let p = &mut s.paras[a.para];
        let keep_fmt = p.fmt_at(a.off);
        let i = p.split_at(a.off);
        let j = p.split_at(b.off);
        p.runs.drain(i..j);
        if p.runs.iter().all(|r| r.text.is_empty()) {
            p.runs = vec![Run { text: String::new(), fmt: keep_fmt, field: None }];
        }
        p.normalize();
        return a;
    }
    // Ogon ostatniego akapitu doklejamy do początku pierwszego.
    let last = &mut s.paras[b.para];
    let j = last.split_at(b.off);
    let tail: Vec<Run> = last.runs.drain(j..).collect();
    let first = &mut s.paras[a.para];
    let keep_fmt = first.fmt_at(a.off);
    let i = first.split_at(a.off);
    first.runs.truncate(i);
    first.runs.extend(tail);
    if first.runs.iter().all(|r| r.text.is_empty()) {
        first.runs = vec![Run { text: String::new(), fmt: keep_fmt, field: None }];
    }
    first.normalize();
    s.paras.drain(a.para + 1..=b.para);
    a
}

/// Formatowanie znaków w zakresie.
pub fn format_chars(s: &mut Story, a: At, b: At, f: &dyn Fn(&mut CharFmt)) {
    let a = clamp(s, a);
    let b = clamp(s, b);
    for pi in a.para..=b.para {
        let p = &mut s.paras[pi];
        let lo = if pi == a.para { a.off } else { 0 };
        let hi = if pi == b.para { b.off } else { p.len() };
        if lo >= hi && !(p.is_empty() && a != b) {
            continue;
        }
        if p.is_empty() {
            for r in p.runs.iter_mut() {
                f(&mut r.fmt);
            }
            continue;
        }
        let i = p.split_at(lo);
        let j = p.split_at(hi);
        for r in p.runs[i..j].iter_mut() {
            f(&mut r.fmt);
        }
        p.normalize();
    }
}

/// Formatowanie akapitów od `a` do `b`.
pub fn format_paras(s: &mut Story, a: usize, b: usize, f: &dyn Fn(&mut Paragraph)) {
    let n = s.paras.len();
    for p in s.paras[a.min(n - 1)..=b.min(n - 1)].iter_mut() {
        f(p);
    }
}

/// Fragment tekstu (pierwszy i ostatni akapit częściowo).
pub fn fragment(s: &Story, a: At, b: At) -> Vec<Paragraph> {
    let a = clamp(s, a);
    let b = clamp(s, b);
    let mut out = Vec::new();
    for pi in a.para..=b.para {
        let mut p = s.paras[pi].clone();
        let lo = if pi == a.para { a.off } else { 0 };
        let hi = if pi == b.para { b.off } else { p.len() };
        let j = p.split_at(hi);
        p.runs.truncate(j);
        let i = p.split_at(lo);
        p.runs.drain(..i);
        p.normalize();
        out.push(p);
    }
    out
}

pub fn fragment_text(frag: &[Paragraph]) -> String {
    frag.iter().map(|p| p.text().replace(FIELD_CHAR, "")).collect::<Vec<_>>().join("\r\n")
}

/// Wkleja fragment. Pierwszy akapit fragmentu łączy się z bieżącym, a gdy fragment ma kilka
/// akapitów – kolejne przyjmują swoje style.
pub fn insert_fragment(s: &mut Story, at: At, frag: &[Paragraph]) -> At {
    let at = clamp(s, at);
    if frag.is_empty() {
        return at;
    }
    if frag.len() == 1 {
        let p = &mut s.paras[at.para];
        let i = p.split_at(at.off);
        let runs: Vec<Run> = frag[0].runs.iter().filter(|r| !r.text.is_empty()).cloned().collect();
        let len: usize = runs.iter().map(|r| r.text.len()).sum();
        for (k, r) in runs.into_iter().enumerate() {
            p.runs.insert(i + k, r);
        }
        p.normalize();
        return At { para: at.para, off: at.off + len };
    }
    let end = split(s, at);
    // Początek fragmentu na końcu pierwszego akapitu.
    {
        let p = &mut s.paras[at.para];
        p.runs.extend(frag[0].runs.iter().filter(|r| !r.text.is_empty()).cloned());
        p.normalize();
    }
    let mut idx = end.para;
    for mid in &frag[1..frag.len() - 1] {
        s.paras.insert(idx, mid.clone());
        idx += 1;
    }
    let last = &frag[frag.len() - 1];
    let p = &mut s.paras[idx];
    let len: usize = last.runs.iter().map(|r| r.text.len()).sum();
    let mut runs: Vec<Run> = last.runs.iter().filter(|r| !r.text.is_empty()).cloned().collect();
    runs.extend(p.runs.drain(..));
    p.runs = runs;
    p.style = last.style.clone();
    p.fmt = last.fmt.clone();
    p.normalize();
    At { para: idx, off: len }
}

// ---------------------------------------------------------------- ruchy kursora

pub fn next_char(text: &str, off: usize) -> usize {
    text[off.min(text.len())..].grapheme_indices(true).nth(1).map(|(i, _)| off + i).unwrap_or(text.len())
}

pub fn prev_char(text: &str, off: usize) -> usize {
    text[..off.min(text.len())].grapheme_indices(true).last().map(|(i, _)| i).unwrap_or(0)
}

pub fn next_word(text: &str, off: usize) -> usize {
    let mut seen_word = false;
    for (i, w) in text.split_word_bound_indices() {
        if i + w.len() <= off {
            continue;
        }
        if i >= off && seen_word && !w.trim().is_empty() {
            return i;
        }
        if !w.trim().is_empty() || i >= off {
            seen_word = true;
        }
    }
    text.len()
}

pub fn prev_word(text: &str, off: usize) -> usize {
    let mut best = 0;
    for (i, w) in text.split_word_bound_indices() {
        if i >= off {
            break;
        }
        if !w.trim().is_empty() {
            best = i;
        }
    }
    best
}

/// Wyraz wokół miejsca (dwuklik).
pub fn word_at(text: &str, off: usize) -> (usize, usize) {
    for (i, w) in text.split_word_bound_indices() {
        if off >= i && off < i + w.len() || (off == text.len() && i + w.len() == off) {
            if w.trim().is_empty() && off > 0 {
                continue;
            }
            return (i, i + w.len());
        }
    }
    (off, off)
}

/// Zdanie wokół miejsca (potrójne kliknięcie zaznacza akapit, Ctrl+klik – zdanie).
pub fn sentence_at(text: &str, off: usize) -> (usize, usize) {
    for (i, s) in text.split_sentence_bound_indices() {
        if off >= i && off <= i + s.len() {
            return (i, i + s.trim_end().len());
        }
    }
    (0, text.len())
}

// ---------------------------------------------------------------- wyszukiwanie

/// Wszystkie wystąpienia `needle` w historii: (akapit, początek, koniec).
pub fn find_all(s: &Story, needle: &str, case: bool, whole: bool) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let n = if case { needle.to_string() } else { needle.to_lowercase() };
    for (pi, p) in s.paras.iter().enumerate() {
        let t = p.text();
        let hay = if case { t.clone() } else { t.to_lowercase() };
        // Zmiana wielkości liter może zmienić długość – wtedy szukamy w oryginale.
        let (hay, nn) = if hay.len() == t.len() { (hay, n.clone()) } else { (t.clone(), needle.to_string()) };
        let mut from = 0;
        while let Some(i) = hay[from..].find(&nn) {
            let a = from + i;
            let b = a + nn.len();
            let ok = !whole || {
                let before = t[..a].chars().last().is_none_or(|c| !c.is_alphanumeric());
                let after = t[b..].chars().next().is_none_or(|c| !c.is_alphanumeric());
                before && after
            };
            if ok {
                out.push((pi, a, b));
            }
            from = b.max(a + 1);
            while from < hay.len() && !hay.is_char_boundary(from) {
                from += 1;
            }
            if from >= hay.len() {
                break;
            }
        }
    }
    out
}

/// Liczba wyrazów i znaków (bez spacji / ze spacjami).
pub fn stats(s: &Story) -> (usize, usize, usize) {
    let mut words = 0;
    let mut chars = 0;
    let mut nospace = 0;
    for p in &s.paras {
        let t = p.text();
        words += t.unicode_words().count();
        for c in t.chars() {
            if c == FIELD_CHAR {
                continue;
            }
            chars += 1;
            if !c.is_whitespace() {
                nospace += 1;
            }
        }
    }
    (words, nospace, chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_split() {
        let mut s = Story::plain("Ala ma kota", "normal");
        let at = insert_text(&mut s, At::new(0, 3), " Nowak", &CharFmt::default());
        assert_eq!(s.paras[0].text(), "Ala Nowak ma kota");
        assert_eq!(at.off, 9);
        let at = split(&mut s, At::new(0, 9));
        assert_eq!(s.paras.len(), 2);
        assert_eq!(s.paras[1].text(), " ma kota");
        let a = delete(&mut s, At::new(0, 3), At::new(1, 3));
        assert_eq!(s.paras.len(), 1);
        assert_eq!(s.paras[0].text(), "Ala kota");
        assert_eq!(a.off, 3);
        let _ = at;
    }

    #[test]
    fn bold_range() {
        let mut s = Story::plain("abcdef", "normal");
        format_chars(&mut s, At::new(0, 1), At::new(0, 3), &|f| f.bold = Some(true));
        assert_eq!(s.paras[0].runs.len(), 3);
        format_chars(&mut s, At::new(0, 0), At::new(0, 6), &|f| f.bold = Some(true));
        assert_eq!(s.paras[0].runs.len(), 1);
    }

    #[test]
    fn fragment_roundtrip() {
        let s = Story::plain("jeden\ndwa\ntrzy", "normal");
        let f = fragment(&s, At::new(0, 2), At::new(2, 2));
        assert_eq!(f.len(), 3);
        let mut t = Story::plain("XY", "normal");
        let end = insert_fragment(&mut t, At::new(0, 1), &f);
        assert_eq!(t.text(), "Xden\ndwa\ntrY");
        assert_eq!(end, At::new(2, 2));
    }

    #[test]
    fn words() {
        let t = "Ala ma kota";
        assert_eq!(next_word(t, 0), 4);
        assert_eq!(prev_word(t, 7), 4);
        assert_eq!(word_at(t, 5), (4, 6));
    }
}
