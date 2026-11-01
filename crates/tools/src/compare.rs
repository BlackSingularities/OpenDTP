//! Porównanie dwóch wersji dokumentu (jak „Porównaj dokumenty” w WordCraft): wynik to nowa
//! wersja treści ze śledzonymi zmianami – usunięte akapity i słowa jako usunięcia, dopisane jako
//! wstawienia. Zmiany przyjmuje się albo odrzuca zwykłymi poleceniami recenzji.

use crate::model::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Same(usize, usize),
    Del(usize),
    Ins(usize),
}

/// Największy wspólny podciąg (najpierw wspólny początek i koniec). Przy bardzo długich
/// fragmentach środek traktujemy jako zastąpiony w całości, żeby nie zająć setek MB pamięci.
fn diff<T: PartialEq>(a: &[T], b: &[T]) -> Vec<Op> {
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf] {
        suf += 1;
    }
    let (ma, mb) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let mut ops: Vec<Op> = (0..pre).map(|i| Op::Same(i, i)).collect();
    if ma.len().saturating_mul(mb.len()) > 4_000_000 {
        ops.extend((0..ma.len()).map(|i| Op::Del(pre + i)));
        ops.extend((0..mb.len()).map(|j| Op::Ins(pre + j)));
    } else {
        let (n, m) = (ma.len(), mb.len());
        let mut t = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                t[i * (m + 1) + j] = if ma[i] == mb[j] { t[(i + 1) * (m + 1) + j + 1] + 1 } else { t[(i + 1) * (m + 1) + j].max(t[i * (m + 1) + j + 1]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && ma[i] == mb[j] {
                ops.push(Op::Same(pre + i, pre + j));
                i += 1;
                j += 1;
            } else if j < m && (i == n || t[i * (m + 1) + j + 1] >= t[(i + 1) * (m + 1) + j]) {
                ops.push(Op::Ins(pre + j));
                j += 1;
            } else {
                ops.push(Op::Del(pre + i));
                i += 1;
            }
        }
    }
    ops.extend((0..suf).map(|k| Op::Same(a.len() - suf + k, b.len() - suf + k)));
    ops
}

fn mark(p: &Paragraph, rev: &Rev) -> Paragraph {
    let mut p = p.clone();
    for r in p.runs.iter_mut() {
        r.fmt.rev = Some(rev.clone());
    }
    p
}

/// Słowa razem z odstępami po nich – granice, na których porównujemy zmieniony akapit.
fn words(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_space = false;
    for (i, c) in s.char_indices() {
        let space = c.is_whitespace();
        if in_space && !space {
            out.push(&s[start..i]);
            start = i;
        }
        in_space = space;
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// Zmieniony akapit: wspólne słowa bez oznaczeń, reszta jako usunięcia i wstawienia.
fn word_diff(old: &Paragraph, new: &Paragraph, del: &Rev, ins: &Rev) -> Paragraph {
    let (ot, nt) = (old.text(), new.text());
    let (ow, nw) = (words(&ot), words(&nt));
    let offs = |w: &[&str]| -> Vec<usize> {
        let mut o = 0;
        w.iter()
            .map(|x| {
                let s = o;
                o += x.len();
                s
            })
            .collect()
    };
    let (oo, no) = (offs(&ow), offs(&nw));
    let mut runs: Vec<Run> = Vec::new();
    for op in diff(&ow, &nw) {
        let run = match op {
            Op::Same(_, j) => Run { text: nw[j].to_string(), fmt: new.fmt_at(no[j] + 1), field: None },
            Op::Ins(j) => {
                let mut fmt = new.fmt_at(no[j] + 1);
                fmt.rev = Some(ins.clone());
                Run { text: nw[j].to_string(), fmt, field: None }
            }
            Op::Del(i) => {
                let mut fmt = old.fmt_at(oo[i] + 1);
                fmt.rev = Some(del.clone());
                Run { text: ow[i].to_string(), fmt, field: None }
            }
        };
        runs.push(run);
    }
    let mut p = Paragraph { style: new.style.clone(), fmt: new.fmt.clone(), runs };
    p.normalize();
    p
}

fn plain(p: &Paragraph) -> bool {
    p.runs.iter().all(|r| r.field.is_none())
}

/// Porównuje treść główną. Wynik: dokument `new` (style, obiekty, ustawienia) z treścią, w której
/// różnice względem `old` są śledzonymi zmianami autora `author`. Zwraca też liczbę zmian.
pub fn compare(old: &Document, new: &Document, author: &str) -> (Document, usize) {
    let date = crate::util::iso_now();
    let del = Rev { kind: RevKind::Delete, author: author.to_string(), date: date.clone() };
    let ins = Rev { kind: RevKind::Insert, author: author.to_string(), date };
    let (a, b) = (&old.body.paras, &new.body.paras);
    let at: Vec<String> = a.iter().map(|p| p.text()).collect();
    let bt: Vec<String> = b.iter().map(|p| p.text()).collect();
    let ops = diff(&at, &bt);

    let mut out = new.clone();
    let mut paras: Vec<Paragraph> = Vec::new();
    let mut changes = 0;
    let mut k = 0;
    while k < ops.len() {
        if let Op::Same(_, j) = ops[k] {
            paras.push(b[j].clone());
            k += 1;
            continue;
        }
        // Blok zmian: usunięte i dopisane akapity; pary na tych samych pozycjach porównujemy słowami.
        let mut dels = Vec::new();
        let mut inss = Vec::new();
        while k < ops.len() && !matches!(ops[k], Op::Same(..)) {
            match ops[k] {
                Op::Del(i) => dels.push(i),
                Op::Ins(j) => inss.push(j),
                Op::Same(..) => {}
            }
            k += 1;
        }
        changes += 1;
        let pairs = dels.len().min(inss.len());
        for n in 0..pairs {
            let (o, w) = (&a[dels[n]], &b[inss[n]]);
            if plain(o) && plain(w) {
                paras.push(word_diff(o, w, &del, &ins));
            } else {
                paras.push(mark(o, &del));
                paras.push(mark(w, &ins));
            }
        }
        paras.extend(dels[pairs..].iter().map(|&i| mark(&a[i], &del)));
        paras.extend(inss[pairs..].iter().map(|&j| mark(&b[j], &ins)));
    }
    if !paras.is_empty() {
        out.body.paras = paras;
    }
    (out, changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Document {
        let mut d = Document::new();
        d.body = Story::plain(text, "normal");
        d
    }

    #[test]
    fn marks_words_and_paragraphs() {
        let (d, n) = compare(&doc("Ala ma kota.\nUsunięty akapit.\nKoniec."), &doc("Ala ma psa.\nKoniec.\nNowy."), "Test");
        assert_eq!(n, 2);
        let mut accepted = d.clone();
        accepted.resolve_revs(true);
        let texts: Vec<String> = accepted.body.paras.iter().map(|p| p.text()).filter(|t| !t.is_empty()).collect();
        assert_eq!(texts, ["Ala ma psa.", "Koniec.", "Nowy."]);
        let mut rejected = d;
        rejected.resolve_revs(false);
        let texts: Vec<String> = rejected.body.paras.iter().map(|p| p.text()).filter(|t| !t.is_empty()).collect();
        assert_eq!(texts, ["Ala ma kota.", "Usunięty akapit.", "Koniec."]);
    }
}
