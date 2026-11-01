//! OpenDTP – narzędzia działające na całym dokumencie, na wzór WordCraft (ArtCraft, MIT/Apache-2.0):
//! cytowania i bibliografia, korespondencja seryjna z CSV, porównywanie dokumentów, inspektor dokumentu, kontrola
//! dostępności i operacje na tabelach.

pub mod a11y;
pub mod cite;
pub mod compare;
pub mod csv;
pub mod inspect;
pub mod labels;
pub mod merge;
pub mod tables;

#[allow(unused_imports)]
use opendtp_base::{lang, trf, util};
#[allow(unused_imports)]
use opendtp_doc::model;

/// Wszystkie teksty dokumentu do odczytu (treść, nagłówki, stopki, przypisy, ramki).
pub(crate) fn stories(doc: &model::Document) -> Vec<&model::Story> {
    let mut v: Vec<&model::Story> = vec![&doc.body];
    v.extend([&doc.header, &doc.footer, &doc.header_first, &doc.footer_first, &doc.header_even, &doc.footer_even].into_iter().flatten());
    v.extend(doc.notes.iter().map(|n| &n.story));
    v.extend(doc.objects.iter().filter_map(|o| o.story()));
    v
}

/// Jak [`stories`], do zmiany.
pub(crate) fn stories_mut(doc: &mut model::Document) -> Vec<&mut model::Story> {
    let mut v: Vec<&mut model::Story> = vec![&mut doc.body];
    v.extend([&mut doc.header, &mut doc.footer, &mut doc.header_first, &mut doc.footer_first, &mut doc.header_even, &mut doc.footer_even].into_iter().flatten());
    v.extend(doc.notes.iter_mut().map(|n| &mut n.story));
    v.extend(doc.objects.iter_mut().filter_map(|o| o.story_mut()));
    v
}
