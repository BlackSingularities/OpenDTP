//! Etykiety adresowe z CSV (jak „Koperty i etykiety” w WordCraft): arkusz A4 podzielony na
//! etykiety o typowych rozmiarach, w każdej ramka tekstowa z treścią wzoru wypełnioną danymi
//! jednego wiersza. Wzorem jest treść bieżącego dokumentu z polami «Nazwa»; dokument bez pól
//! dostaje wzór ze wszystkich kolumn CSV, każda w osobnym wierszu.

use crate::csv::Table;
use crate::model::*;

const MM: f32 = 72.0 / 25.4;

/// Arkusz etykiet: nazwa, kolumny, wiersze, rozmiar etykiety i marginesy (mm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sheet {
    pub id: &'static str,
    pub cols: usize,
    pub rows: usize,
    pub w: f32,
    pub h: f32,
    pub top: f32,
    pub left: f32,
    pub gap_x: f32,
    pub gap_y: f32,
}

/// Popularne arkusze A4 (wymiary jak u producentów etykiet).
pub const SHEETS: [Sheet; 4] = [
    Sheet { id: "3x8", cols: 3, rows: 8, w: 70.0, h: 37.0, top: 0.5, left: 0.0, gap_x: 0.0, gap_y: 0.0 },
    Sheet { id: "3x7", cols: 3, rows: 7, w: 63.5, h: 38.1, top: 15.15, left: 7.2, gap_x: 2.5, gap_y: 0.0 },
    Sheet { id: "2x7", cols: 2, rows: 7, w: 99.1, h: 38.1, top: 15.15, left: 4.65, gap_x: 2.5, gap_y: 0.0 },
    Sheet { id: "2x4", cols: 2, rows: 4, w: 105.0, h: 74.25, top: 0.0, left: 0.0, gap_x: 0.0, gap_y: 0.0 },
];

pub fn sheet(id: &str) -> Sheet {
    SHEETS.into_iter().find(|s| s.id == id).unwrap_or(SHEETS[0])
}

/// Wzór etykiety: treść dokumentu z polami albo wszystkie kolumny CSV.
fn template(doc: &Document, data: &Table) -> Story {
    if !crate::merge::fields(doc).is_empty() {
        return doc.body.clone();
    }
    let paras: Vec<Paragraph> = data.headers.iter().map(|h| Paragraph::with_text("normal", &format!("«{h}»"))).collect();
    Story { paras: if paras.is_empty() { vec![Paragraph::default()] } else { paras } }
}

/// Dokument z etykietami dla wszystkich wierszy `data`.
pub fn labels(doc: &Document, data: &Table, sheet: Sheet) -> Document {
    let mut out = Document::new();
    out.styles = doc.styles.clone();
    out.settings = doc.settings.clone();
    out.meta.title = doc.meta.title.clone();
    out.page.width = 210.0 * MM;
    out.page.height = 297.0 * MM;
    out.page.margin_top = 0.0;
    out.page.margin_bottom = 0.0;
    out.page.margin_left = 0.0;
    out.page.margin_right = 0.0;
    out.page.columns = 1;
    let tpl = template(doc, data);
    let per_page = (sheet.cols * sheet.rows).max(1);
    for row in 0..data.rows.len() {
        let page = row / per_page;
        let k = row % per_page;
        let (c, r) = (k % sheet.cols, k / sheet.cols);
        let mut story = tpl.clone();
        for p in story.paras.iter_mut() {
            p.fmt.break_before = None;
            for run in p.runs.iter_mut().filter(|r| r.field.is_none()) {
                run.text = crate::merge::fill(&run.text, data, row);
            }
        }
        out.objects.push(Object {
            id: out.next_id,
            name: String::new(),
            kind: ObjKind::Text { story, padding: 4.0 * MM, columns: 1, gap: 12.0, valign: VAlign::Center, next: None },
            page,
            x: (sheet.left + c as f32 * (sheet.w + sheet.gap_x)) * MM,
            y: (sheet.top + r as f32 * (sheet.h + sheet.gap_y)) * MM,
            w: sheet.w * MM,
            h: sheet.h * MM,
            shape: Shape::rect(),
            wrap: Wrap::default(),
            fill: None,
            stroke: None,
            opacity: 1.0,
            corner: 0.0,
            shadow: false,
            locked: false,
            anchor_dy: 4.0,
            cont_of: None,
            repeat: Default::default(),
        });
        out.next_id += 1;
    }
    out.min_pages = data.rows.len().div_ceil(per_page).max(1);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv::parse;

    #[test]
    fn fills_a_sheet_and_spills_to_next_page() {
        let data = parse(&format!("Imię;Miasto\n{}", (0..25).map(|i| format!("Osoba {i};Gdańsk\n")).collect::<String>()));
        let d = labels(&Document::new(), &data, sheet("3x8"));
        assert_eq!(d.objects.len(), 25);
        assert_eq!(d.min_pages, 2);
        assert_eq!(d.objects[24].page, 1);
        let s = d.objects[3].story().unwrap();
        assert_eq!(s.paras[0].text(), "Osoba 3");
        assert_eq!(s.paras[1].text(), "Gdańsk");
    }
}
