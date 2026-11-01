//! Polecenia narzędzi dokumentu (na wzór WordCraft): korespondencja seryjna z CSV, porównanie
//! z inną wersją, inspektor dokumentu, kontrola dostępności, tabele (sortowanie, suma, zamiana
//! tekstu w tabelę i z powrotem), cytowania i bibliografia, spis ilustracji, etykiety adresowe.

use opendtp_core::model::*;
use opendtp_core::{cite, compare, csv, inspect, labels, merge, tables, trf};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::editor::Kind;
use crate::{files, refresh, toast, tr, State};

/// Obsługuje polecenie, jeśli jest jednym z narzędzi; `false` – to nie nasze.
pub fn run(st: &mut State, id: &str, arg: &str) -> bool {
    match id {
        "merge" => merge_csv(st, false),
        "merge-each" => merge_csv(st, true),
        "merge-fields" => {
            let f = merge::fields(&st.ed.doc);
            if f.is_empty() {
                toast(st, tr("Brak pól korespondencji – wpisz w treści np. «Imię» albo {{Imię}}"));
            } else {
                toast(st, &trf!("Pola korespondencji: {}", f.join(", ")));
            }
        }
        "compare" => compare_with(st),
        "inspect" => {
            let r = inspect::inspect(&st.ed.doc);
            if r.is_clean() {
                toast(st, tr("Dokument nie zawiera komentarzy, zmian, historii wersji ani danych osobowych"));
            } else {
                let who = if r.people.is_empty() { String::new() } else { trf!(" · osoby: {}", r.people.join(", ")) };
                toast(st, &trf!("Komentarze: {} · zmiany: {} · wersje: {} · dane autora: {}{}", r.comments, r.revisions, r.versions, if r.personal { tr("tak") } else { tr("nie") }, who));
            }
        }
        "inspect-clean" => {
            if inspect::inspect(&st.ed.doc).is_clean() {
                toast(st, tr("Nie ma czego usuwać"));
                return true;
            }
            st.ed.begin(Kind::Other);
            inspect::clean(&mut st.ed.doc, inspect::Clean::ALL);
            st.ed.changed();
            refresh(st);
            toast(st, tr("Usunięto komentarze, historię wersji i dane autora; zmiany zostały przyjęte"));
        }
        // Wyniki kontroli dostępności są w panelu Preflight, pod błędami druku.
        "a11y" => crate::actions::run(st, "preflight", "keep"),
        "table-sort" | "table-sort-desc" => {
            let col = arg.parse::<usize>().ok().or_else(|| st.ctx_cell.filter(|c| Some(c.0) == st.ed.obj).map(|c| c.2)).unwrap_or(0);
            let desc = id == "table-sort-desc";
            if !with_table(st, &move |t| tables::sort(t, col, desc)) {
                toast(st, tr("Zaznacz tabelę"));
            }
        }
        "table-sum" => {
            let col = st.ctx_cell.filter(|c| Some(c.0) == st.ed.obj).map(|c| c.2).unwrap_or(0);
            let sum = selected_table(st).and_then(|t| tables::column_sum(t, col));
            match sum {
                Some(v) => {
                    let s = format!("{v:.2}").trim_end_matches('0').trim_end_matches('.').replace('.', ",");
                    toast(st, &trf!("Suma kolumny {}: {}", col + 1, s));
                }
                None => toast(st, tr("W tej kolumnie nie ma liczb")),
            }
        }
        "cite" => {
            cite_dialog(st);
            if let Some(ui) = st.ui.upgrade() {
                ui.global::<crate::App>().set_dialog("cite".into());
            }
        }
        "cite-style" => {
            let style = CiteStyle::from_index(arg.parse().unwrap_or(0));
            if st.ed.doc.cite_style != style {
                st.ed.begin(Kind::Other);
                st.ed.doc.cite_style = style;
                cite::update(&mut st.ed.doc);
                st.ed.changed();
                refresh(st);
            }
            cite_dialog(st);
        }
        "cite-insert" => {
            let ids: Vec<u64> = arg.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if ids.is_empty() {
                return true;
            }
            if let Some(ui) = st.ui.upgrade() {
                ui.global::<crate::App>().set_dialog("".into());
            }
            let run = cite::citation_run(&st.ed.doc, &ids);
            st.ed.paste(&[Paragraph { style: String::new(), fmt: ParaFmt::default(), runs: vec![run] }]);
            // Numery IEEE mogły się przesunąć – przeliczamy wszystkie cytowania.
            cite::update(&mut st.ed.doc);
            st.ed.changed();
            refresh(st);
        }
        "cite-add" => match cite::parse_form(arg) {
            Some(src) => {
                st.ed.begin(Kind::Other);
                let n = cite::add_sources(&mut st.ed.doc, vec![src]);
                st.ed.changed();
                if let Some(ui) = st.ui.upgrade() {
                    ui.global::<crate::App>().set_cite_new("".into());
                }
                cite_dialog(st);
                toast(st, if n > 0 { tr("Dodano źródło") } else { tr("To źródło już jest na liście") });
            }
            None => toast(st, tr("Wpisz co najmniej tytuł („Tytuł: …”)")),
        },
        "cite-import" => {
            let Some(p) = rfd::FileDialog::new().set_title(tr("Importuj źródła")).add_filter("BibTeX / RIS", &["bib", "ris", "txt"]).pick_file() else { return true };
            let text = match std::fs::read(&p) {
                Ok(b) => csv::decode(&b),
                Err(e) => {
                    toast(st, &trf!("Nie udało się otworzyć pliku: {}", e));
                    return true;
                }
            };
            let ris = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ris")) || text.contains("\nTY  - ") || text.starts_with("TY  - ");
            let list = if ris { cite::parse_ris(&text) } else { cite::parse_bibtex(&text) };
            st.ed.begin(Kind::Other);
            let n = cite::add_sources(&mut st.ed.doc, list);
            st.ed.changed();
            cite_dialog(st);
            toast(st, &trf!("Zaimportowano źródeł: {}", n));
        }
        "bibliography" => {
            if st.ed.doc.sources.is_empty() {
                toast(st, tr("Najpierw dodaj źródła (Odwołania › Cytowanie)"));
                return true;
            }
            if let Some(ui) = st.ui.upgrade() {
                ui.global::<crate::App>().set_dialog("".into());
            }
            st.ed.begin(Kind::Other);
            if st.ed.doc.body.paras.iter().any(|p| p.style == cite::BIB_STYLE) {
                cite::update(&mut st.ed.doc);
                st.ed.changed();
                toast(st, tr("Bibliografia zaktualizowana"));
            } else {
                let paras = cite::bibliography_block(&mut st.ed.doc);
                st.ed.changed();
                if st.ed.story != StoryRef::Body {
                    st.ed.set_caret(StoryRef::Body, st.ed.head, false);
                }
                st.ed.insert_paragraphs_at_caret(paras);
            }
            refresh(st);
        }
        "tof" | "tof-update" => {
            let kind = SeqKind::from_id(arg).unwrap_or(SeqKind::Figure);
            figures(st, kind, id == "tof-update");
        }
        "labels" => labels(st, if arg.is_empty() { "3x8" } else { arg }),
        "text-to-table" => text_to_table(st),
        "table-to-text" => table_to_text(st),
        _ => return false,
    }
    true
}

fn selected_table(st: &State) -> Option<&Table> {
    match &st.ed.doc.object(st.ed.obj?)?.kind {
        ObjKind::Table { table } => Some(table),
        _ => None,
    }
}

fn with_table(st: &mut State, f: &dyn Fn(&mut Table)) -> bool {
    if selected_table(st).is_none() {
        return false;
    }
    st.ed.edit_table(f);
    refresh(st);
    true
}

fn author(st: &State) -> String {
    if !st.prefs.author.is_empty() {
        st.prefs.author.clone()
    } else {
        std::env::var("USERNAME").unwrap_or_default()
    }
}

fn merge_csv(st: &mut State, each: bool) {
    if merge::fields(&st.ed.doc).is_empty() {
        toast(st, tr("Brak pól korespondencji – wpisz w treści np. «Imię» albo {{Imię}}"));
        return;
    }
    let Some(p) = rfd::FileDialog::new().set_title(tr("Dane do korespondencji (CSV)")).add_filter("CSV", &["csv", "txt"]).pick_file() else { return };
    let data = match std::fs::read(&p) {
        Ok(b) => csv::parse(&csv::decode(&b)),
        Err(e) => {
            toast(st, &trf!("Nie udało się otworzyć pliku: {}", e));
            return;
        }
    };
    if data.rows.is_empty() {
        toast(st, tr("Plik CSV nie zawiera danych (pierwszy wiersz to nazwy kolumn)"));
        return;
    }
    let missing: Vec<String> = merge::fields(&st.ed.doc).into_iter().filter(|f| !data.headers.iter().any(|h| h.to_lowercase() == f.to_lowercase())).collect();
    if each {
        let Some(dir) = rfd::FileDialog::new().set_title(tr("Folder na dokumenty")).pick_folder() else { return };
        let base = files::doc_name(&st.ed);
        let mut ok = 0;
        for (i, d) in merge::merge_each(&st.ed.doc, &data).iter().enumerate() {
            let label: String = data.rows[i].first().map(|s| s.chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-').collect()).unwrap_or_default();
            let name = format!("{base} {:03} {}.odtp", i + 1, label.trim());
            if opendtp_core::io::odtp::save(d, &dir.join(name.trim())).is_ok() {
                ok += 1;
            }
        }
        toast(st, &trf!("Zapisano dokumentów: {} z {}", ok, data.rows.len()));
        let _ = open::that(&dir);
    } else {
        let doc = merge::merge(&st.ed.doc, &data);
        files::set_doc(st, doc, None);
        st.ed.dirty = true;
        toast(st, &trf!("Scalono {} rekordów – każdy od nowej strony", data.rows.len()));
    }
    if !missing.is_empty() {
        toast(st, &trf!("Brak kolumn w CSV: {} – te pola zostały bez zmian", missing.join(", ")));
    }
}

fn compare_with(st: &mut State) {
    let Some(p) = rfd::FileDialog::new()
        .set_title(tr("Porównaj z wcześniejszą wersją"))
        .add_filter(tr("Dokumenty"), &["odtp", "docx", "odt", "rtf", "txt", "md"])
        .pick_file()
    else {
        return;
    };
    let old = match files::load_any(&p) {
        Ok(d) => d,
        Err(e) => {
            toast(st, &trf!("Nie udało się otworzyć pliku: {}", e));
            return;
        }
    };
    let (doc, n) = compare::compare(&old, &st.ed.doc, &author(st));
    if n == 0 {
        toast(st, tr("Dokumenty mają tę samą treść"));
        return;
    }
    files::set_doc(st, doc, None);
    st.ed.dirty = true;
    toast(st, &trf!("Różnice: {} – przejrzyj je na karcie Recenzja (przyjmij albo odrzuć)", n));
}

fn text_to_table(st: &mut State) {
    if st.ed.story != StoryRef::Body {
        toast(st, tr("Zamiana w tabelę działa w treści głównej"));
        return;
    }
    let lines = st.ed.take_selected_paras();
    let rows = tables::text_to_rows(&lines);
    if rows.is_empty() {
        toast(st, tr("Zaznacz akapity z kolumnami oddzielonymi tabulatorem, średnikiem albo przecinkiem"));
        return;
    }
    let page = st.ed.caret_page().map(|c| c.0).unwrap_or(0);
    let ps = st.ed.doc.page.clone();
    let y = st.ed.caret_page().map(|c| c.2).unwrap_or(ps.margin_top);
    let mut t = Table::default();
    t.rows = rows;
    if let Some(c) = st.ed.doc.style("heading1").and_then(|s| s.chars.color) {
        t.accent = c;
    }
    let h = t.rows.len() as f32 * 22.0;
    let o = crate::actions::new_obj(page, ps.margin_left, y, ps.content_width(), h, Shape::rect(), ObjKind::Table { table: t }, WrapMode::TopBottom);
    let id = st.ed.add_object(o);
    crate::actions::anchor_new(st, id);
    crate::actions::object_selected(st);
    refresh(st);
}

fn table_to_text(st: &mut State) {
    let Some(id) = st.ed.obj else {
        toast(st, tr("Zaznacz tabelę"));
        return;
    };
    let Some(lines) = selected_table(st).map(tables::rows_to_text) else {
        toast(st, tr("Zaznacz tabelę"));
        return;
    };
    // Tekst trafia za akapit z kotwicą tabeli (albo na koniec treści).
    let at = st.ed.doc.body.paras.iter().position(|p| p.runs.iter().any(|r| r.field == Some(Field::Anchor(id)))).map(|i| i + 1).unwrap_or(st.ed.doc.body.paras.len());
    st.ed.begin(Kind::Other);
    let paras: Vec<Paragraph> = lines.iter().map(|l| Paragraph::with_text("normal", l)).collect();
    st.ed.doc.body.paras.splice(at..at, paras);
    st.ed.changed();
    st.ed.delete_object();
    refresh(st);
}

/// Lista źródeł i styl w oknie cytowań.
fn cite_dialog(st: &State) {
    let Some(ui) = st.ui.upgrade() else { return };
    let app = ui.global::<crate::App>();
    let items: Vec<crate::ItemV> = st
        .ed
        .doc
        .sources
        .iter()
        .map(|s| crate::ItemV { id: s.id.to_string().into(), label: cite::label(s).into(), icon: crate::commands::icon("book-open"), ..Default::default() })
        .collect();
    app.set_cite_sources(ModelRc::new(VecModel::from(items)));
    app.set_cite_style(st.ed.doc.cite_style.index());
}

/// Wstawia albo odświeża spis ilustracji (tabel, wzorów) – ciąg akapitów stylu `tof-…`.
fn figures(st: &mut State, kind: SeqKind, update: bool) {
    let style = opendtp_core::layout::figures_style(kind);
    if st.ed.doc.style(&style).is_none() {
        // Wygląd jak pierwszy poziom spisu treści (kropki do numeru strony).
        if let Some(mut s) = st.ed.doc.style("toc1").cloned() {
            s.id = style.clone();
            s.name = format!("{} – {}", tr("Spis"), kind.label());
            s.gallery = false;
            st.ed.doc.styles.push(s);
        }
    }
    let block = |doc: &Document| -> Option<(usize, usize)> {
        let a = doc.body.paras.iter().position(|p| p.style == style)?;
        let mut b = a;
        while b + 1 < doc.body.paras.len() && doc.body.paras[b + 1].style == style {
            b += 1;
        }
        Some((a, b))
    };
    let paras = opendtp_core::layout::figures(&st.ed.doc, &st.ed.layout, kind);
    if let Some((a, b)) = block(&st.ed.doc) {
        st.ed.begin(Kind::Other);
        st.ed.doc.body.paras.splice(a..=b, paras);
        st.ed.changed();
        // Druga runda – numery stron po zmianie długości spisu.
        let again = opendtp_core::layout::figures(&st.ed.doc, &st.ed.layout, kind);
        if let Some((a, b)) = block(&st.ed.doc) {
            st.ed.doc.body.paras.splice(a..=b, again);
            st.ed.changed();
        }
        if !update {
            toast(st, tr("Spis zaktualizowany"));
        }
        return;
    }
    if update {
        return;
    }
    if paras.is_empty() {
        toast(st, &trf!("Brak podpisów „{}” – wstaw je poleceniem Podpis", kind.label()));
        return;
    }
    if st.ed.story != StoryRef::Body {
        st.ed.set_caret(StoryRef::Body, st.ed.head, false);
    }
    st.ed.insert_paragraphs_at_caret(paras);
    refresh(st);
}

/// Odświeża cytowania, bibliografię i spisy ilustracji (część „Aktualizuj wszystko”).
pub fn refresh_refs(st: &mut State) {
    if !st.ed.doc.sources.is_empty() {
        st.ed.begin(Kind::Other);
        cite::update(&mut st.ed.doc);
        st.ed.changed();
    }
    for kind in SeqKind::ALL {
        let style = opendtp_core::layout::figures_style(kind);
        if st.ed.doc.body.paras.iter().any(|p| p.style == style) {
            figures(st, kind, true);
        }
    }
}

fn labels(st: &mut State, sheet: &str) {
    let Some(p) = rfd::FileDialog::new().set_title(tr("Adresy do etykiet (CSV)")).add_filter("CSV", &["csv", "txt"]).pick_file() else { return };
    let data = match std::fs::read(&p) {
        Ok(b) => csv::parse(&csv::decode(&b)),
        Err(e) => {
            toast(st, &trf!("Nie udało się otworzyć pliku: {}", e));
            return;
        }
    };
    if data.rows.is_empty() {
        toast(st, tr("Plik CSV nie zawiera danych (pierwszy wiersz to nazwy kolumn)"));
        return;
    }
    let sh = labels::sheet(sheet);
    let doc = labels::labels(&st.ed.doc, &data, sh);
    files::set_doc(st, doc, None);
    st.ed.dirty = true;
    toast(st, &trf!("Etykiety: {} na arkuszach A4: {} ({}×{})", data.rows.len(), st.ed.doc.min_pages, sh.cols, sh.rows));
}
