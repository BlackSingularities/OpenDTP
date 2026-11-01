//! Inspektor dokumentu (jak w WordCraft): co zostaje w pliku poza widoczną treścią – komentarze,
//! niezaakceptowane zmiany, historia wersji, dane autora – i usuwanie tego przed wysłaniem.

use crate::model::*;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub comments: usize,
    pub revisions: usize,
    pub versions: usize,
    /// Autor, temat, słowa kluczowe, daty.
    pub personal: bool,
    /// Autorzy zmian i komentarzy (bez powtórzeń).
    pub people: Vec<String>,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        self.comments == 0 && self.revisions == 0 && self.versions == 0 && !self.personal
    }
}

pub fn inspect(doc: &Document) -> Report {
    let mut people: Vec<String> = Vec::new();
    let mut add = |s: &str| {
        if !s.is_empty() && !people.iter().any(|p| p == s) {
            people.push(s.to_string());
        }
    };
    for c in &doc.comments {
        add(&c.author);
        for r in &c.replies {
            add(&r.author);
        }
    }
    for s in crate::stories(doc) {
        for p in &s.paras {
            for r in &p.runs {
                if let Some(rev) = &r.fmt.rev {
                    add(&rev.author);
                }
            }
        }
    }
    let m = &doc.meta;
    Report {
        comments: doc.comments.len(),
        revisions: doc.rev_count(),
        versions: doc.versions.len(),
        personal: !(m.author.is_empty() && m.subject.is_empty() && m.keywords.is_empty() && m.created.is_empty() && m.modified.is_empty()),
        people,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Clean {
    pub comments: bool,
    /// Zmiany są przyjmowane (zostaje bieżąca treść), nie odrzucane.
    pub revisions: bool,
    pub versions: bool,
    pub personal: bool,
}

impl Clean {
    pub const ALL: Clean = Clean { comments: true, revisions: true, versions: true, personal: true };
}

pub fn clean(doc: &mut Document, what: Clean) {
    if what.comments {
        doc.comments.clear();
        for s in crate::stories_mut(doc) {
            for p in s.paras.iter_mut() {
                let before = p.runs.len();
                p.runs.retain(|r| !matches!(r.field, Some(Field::Comment(_))));
                if p.runs.len() != before {
                    p.normalize();
                }
            }
        }
    }
    if what.revisions {
        doc.resolve_revs(true);
    }
    if what.versions {
        doc.versions.clear();
        doc.version_data.clear();
    }
    if what.personal {
        let m = &mut doc.meta;
        m.author.clear();
        m.subject.clear();
        m.keywords.clear();
        m.created.clear();
        m.modified.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_everything() {
        let mut d = Document::new();
        d.meta.author = "Ktoś".into();
        d.body.paras[0].runs = vec![
            Run { text: "a".into(), ..Default::default() },
            Run { text: FIELD_CHAR.to_string(), field: Some(Field::Comment(7)), ..Default::default() },
        ];
        d.comments.push(Comment { id: 7, author: "Ewa".into(), date: String::new(), text: "?".into(), quote: String::new(), resolved: false, replies: Vec::new() });
        let r = inspect(&d);
        assert_eq!((r.comments, r.personal), (1, true));
        assert_eq!(r.people, ["Ewa"]);
        clean(&mut d, Clean::ALL);
        assert!(inspect(&d).is_clean());
        assert_eq!(d.body.paras[0].text(), "a");
    }
}
