//! Polecenia wstążki, menu, palety i skrótów: `run(id, argument)`.

use slint::ComponentHandle;
use std::time::Duration;

use opendtp_core::edit::At;
use opendtp_core::geom::{self, BoolOp};
use opendtp_core::model::*;
use opendtp_core::{io, styles};
use slint::{ModelRc, VecModel};

use crate::editor::Kind;
use crate::input::set_tool;
use crate::{commands, files, len_str, num, parse_num, refresh, refresh_scroll, sync_lists, toast, tr, unit_k, App, ItemV, State};

mod cmd_file;
mod cmd_text;
mod cmd_insert;
mod cmd_page;
mod cmd_objects;
mod cmd_view;
mod cmd_settings;
mod objects_ops;
mod update;
mod zoom;

pub use objects_ops::*;
pub use update::*;
pub use zoom::*;

/// Wynik obsługi polecenia przez jedną z grup.
enum Flow {
    /// Polecenie nie należy do tej grupy.
    Unhandled,
    /// Wykonane – odśwież widok.
    Done,
    /// Wykonane bez odświeżania (albo przerwane).
    Skip,
}

pub const PAGE_SIZES: [(&str, f32, f32); 9] = [
    ("A4", 210.0, 297.0),
    ("A5", 148.0, 210.0),
    ("A3", 297.0, 420.0),
    ("B5", 176.0, 250.0),
    ("Letter", 215.9, 279.4),
    ("Legal", 215.9, 355.6),
    ("A6", 105.0, 148.0),
    ("DL", 99.0, 210.0),
    ("Square", 210.0, 210.0),
];

fn ui_app(st: &State) -> Option<crate::AppWindow> {
    st.ui.upgrade()
}

pub fn kind_label(o: &Object) -> String {
    match &o.kind {
        ObjKind::Text { .. } if o.shape.is_rect() => tr("Ramka tekstowa").into(),
        ObjKind::Text { .. } => tr("Kształt tekstu").into(),
        ObjKind::Image { .. } => tr("Obraz").into(),
        ObjKind::Shape => tr("Kształt").into(),
        ObjKind::Table { .. } => tr("Tabela").into(),
        ObjKind::Chart { .. } => tr("Wykres").into(),
        ObjKind::PathText { .. } => tr("Tekst na ścieżce").into(),
    }
}

fn color_of(c: slint::Color) -> Color {
    Color { r: c.red(), g: c.green(), b: c.blue(), a: c.alpha() }
}

fn list_kind(id: &str) -> Option<ListKind> {
    Some(match id {
        "bullet" => ListKind::Bullet,
        "dash" => ListKind::Dash,
        "check" => ListKind::Check,
        "arrow" => ListKind::Arrow,
        "number" => ListKind::Number,
        "number-paren" => ListKind::NumberParen,
        "lower-letter" => ListKind::LowerLetter,
        "upper-letter" => ListKind::UpperLetter,
        "lower-roman" => ListKind::LowerRoman,
        "upper-roman" => ListKind::UpperRoman,
        "multi" => ListKind::Multi,
        _ => return None,
    })
}

fn wrap_mode(i: i32) -> WrapMode {
    match i {
        1 => WrapMode::Both,
        2 => WrapMode::Left,
        3 => WrapMode::Right,
        4 => WrapMode::Largest,
        5 => WrapMode::TopBottom,
        6 => WrapMode::Behind,
        _ => WrapMode::None,
    }
}

/// Wartość z pola liczbowego: liczba albo krok (+/−) od bieżącej.
fn value_or_step(arg: &str, cur: f32, step: f32, is_step: bool) -> Option<f32> {
    if is_step {
        let d: f32 = arg.parse().unwrap_or(0.0);
        Some(cur + d * step)
    } else {
        parse_num(arg)
    }
}

pub fn run(st: &mut State, id: &str, arg: &str) {
    // Polecenia z palety i paska szybkiego dostępu: „id:arg”.
    if arg.is_empty() {
        if let Some((a, b)) = id.split_once(':') {
            if !id.starts_with("spell-") {
                let (a, b) = (a.to_string(), b.to_string());
                return run(st, &a, &b);
            }
        }
        if let Some(rest) = id.strip_prefix("text-shape-") {
            let rest = rest.to_string();
            return run(st, "text-shape", &rest);
        }
        if let Some(rest) = id.strip_prefix("shape-") {
            let rest = rest.to_string();
            return run(st, "shape", &rest);
        }
        if let Some(rest) = id.strip_prefix("chart-").filter(|r| !matches!(*r, "data" | "title" | "kind" | "legend" | "values")) {
            let rest = rest.to_string();
            return run(st, "chart", &rest);
        }
        if let Some(rest) = id.strip_prefix("columns-") {
            let rest = rest.to_string();
            return run(st, "columns", &rest);
        }
        if let Some(rest) = id.strip_prefix("export-") {
            let rest = rest.to_string();
            return run(st, "export", &rest);
        }
        if let Some(rest) = id.strip_prefix("bool-") {
            let rest = rest.to_string();
            return run(st, "bool", &rest);
        }
        if id == "field-page" {
            return run(st, "field", "page-number");
        }
        if id == "field-date" {
            return run(st, "field", "date");
        }
    }
    if crate::tools::run(st, id, arg) {
        return;
    }
    let Some(ui) = ui_app(st) else { return };
    let app = ui.global::<App>();
    let unit = st.prefs.unit;
    let uk = unit_k(unit);
    let is_step = id.ends_with("-step");
    let base = id.trim_end_matches("-step");
    let mut scroll = false;
    for handle in [cmd_file::handle, cmd_text::handle, cmd_insert::handle, cmd_page::handle, cmd_objects::handle, cmd_view::handle, cmd_settings::handle] {
        match handle(st, &ui, &app, unit, uk, is_step, base, id, arg, &mut scroll) {
            Flow::Unhandled => continue,
            Flow::Skip => return,
            Flow::Done => {
                if scroll {
                    refresh_scroll(st);
                } else {
                    refresh(st);
                }
                return;
            }
        }
    }
    crate::set_status(st, &opendtp_core::trf!("Nieznane polecenie: {}", id));
}

fn ai_config(st: &State) -> opendtp_core::ai::Config {
    opendtp_core::ai::Config {
        provider: st.prefs.ai_provider.clone(),
        key: crate::settings::unprotect(&st.prefs.ai_key),
        model: st.prefs.ai_model.clone(),
        temperature: st.prefs.ai_temp,
        base: st.prefs.ai_base.clone(),
    }
}

thread_local! {
    static OBJ_CLIP: std::cell::RefCell<Option<Object>> = const { std::cell::RefCell::new(None) };
}
/// Lista elementów biblioteki do panelu.
fn library_list(app: &App) {
    let items: Vec<ItemV> = opendtp_core::library::list()
        .into_iter()
        .map(|i| ItemV {
            id: i.path.to_string_lossy().to_string().into(),
            label: i.name.into(),
            hint: if i.objects { tr("obiekty") } else { tr("tekst") }.into(),
            icon: commands::icon(if i.objects { "shapes" } else { "file-text" }),
            ..Default::default()
        })
        .collect();
    app.set_library(ModelRc::new(VecModel::from(items)));
}
/// Opcje wyszukiwania z panelu Znajdź i zamień.
fn find_opts(app: &App) -> opendtp_core::edit::FindOpts {
    opendtp_core::edit::FindOpts { case: app.get_find_case(), whole: app.get_find_whole(), regex: app.get_find_regex(), style: app.get_find_style().to_string() }
}
fn find_info(st: &State) {
    if let Some(ui) = st.ui.upgrade() {
        let app = ui.global::<App>();
        let n = st.ed.find.len();
        let opts = find_opts(&app);
        let bad = if opts.regex { opts.pattern(&app.get_find_text()).err() } else { None };
        let txt = if app.get_find_text().is_empty() && opts.style.is_empty() {
            String::new()
        } else if let Some(e) = bad {
            opendtp_core::trf!("Błędny wzorzec: {}", e.lines().last().unwrap_or(""))
        } else if n == 0 {
            tr("Brak wyników").to_string()
        } else {
            opendtp_core::trf!("Wynik {} z {}", (st.ed.find_i % n) + 1, n)
        };
        app.set_find_info(txt.into());
    }
}
fn change_case(st: &mut State, how: &str) {
    if !st.ed.has_sel() {
        let t = st.ed.para_text(st.ed.head.para);
        let (a, b) = opendtp_core::edit::word_at(&t, st.ed.head.off);
        let p = st.ed.head.para;
        st.ed.anchor = At::new(p, a);
        st.ed.head = At::new(p, b);
    }
    let frag = st.ed.fragment();
    let conv = |s: &str| -> String {
        match how {
            "upper" => s.to_uppercase(),
            "lower" => s.to_lowercase(),
            "title" => s.split(' ').map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                    None => String::new(),
                }
            }).collect::<Vec<_>>().join(" "),
            _ => {
                let lower = s.to_lowercase();
                let mut c = lower.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            }
        }
    };
    let new: Vec<Paragraph> = frag.into_iter().map(|mut p| {
        for r in p.runs.iter_mut() {
            if r.field.is_none() {
                r.text = conv(&r.text);
            }
        }
        p
    }).collect();
    let (a, _) = st.ed.range();
    st.ed.paste(&new);
    let end = st.ed.head;
    st.ed.anchor = a;
    st.ed.head = end;
}
fn spell_next(st: &mut State) -> bool {
    use unicode_segmentation::UnicodeSegmentation;
    let start = st.ed.range().1;
    let story = st.ed.story;
    let paras: Vec<(usize, String, String)> = st.ed.story_ref().paras.iter().enumerate().map(|(i, p)| (i, p.text(), st.ed.doc.resolve_para(p).1.lang)).collect();
    let n = paras.len();
    for k in 0..=n {
        let (pi, text, lang) = &paras[(start.para + k) % n];
        for (i, w) in text.split_word_bound_indices() {
            if k == 0 && i < start.off {
                continue;
            }
            if w.chars().any(|c| c.is_alphabetic()) && opendtp_core::spell::misspelled(w, lang) {
                st.ed.set_caret(story, At::new(*pi, i), false);
                st.ed.head = At::new(*pi, i + w.len());
                return true;
            }
        }
    }
    false
}
pub fn palette_query(st: &State, q: &str) {
    let Some(ui) = st.ui.upgrade() else { return };
    let items: Vec<ItemV> = commands::ALL
        .iter()
        .filter(|c| q.trim().is_empty() || commands::matches(c, q))
        .take(60)
        .map(|c| ItemV { id: c.id.into(), label: commands::label(c).into(), hint: c.key.into(), icon: commands::icon(c.icon), checked: false, font: "".into() })
        .collect();
    ui.global::<App>().set_palette_items(ModelRc::new(VecModel::from(items)));
}
pub fn font_query(st: &State, q: &str) {
    let Some(ui) = st.ui.upgrade() else { return };
    let ql = q.to_lowercase();
    let items: Vec<ItemV> = st.fonts.iter().filter(|f| ql.is_empty() || f.to_lowercase().contains(&ql)).take(800).map(|f| ItemV { id: f.clone().into(), label: f.clone().into(), ..Default::default() }).collect();
    ui.global::<App>().set_fonts(ModelRc::new(VecModel::from(items)));
}
fn customize_list(st: &State, which: &str) {
    let Some(ui) = st.ui.upgrade() else { return };
    let tab = st.prefs.tabs().get(ui.global::<App>().get_custom_edit().max(0) as usize);
    let list: &[String] = if which == "qat" { &st.prefs.qat } else { tab.map(|t| t.items.as_slice()).unwrap_or(&[]) };
    let items: Vec<ItemV> = commands::ALL.iter().map(|c| ItemV { id: c.id.into(), label: commands::label(c).into(), hint: c.key.into(), icon: Default::default(), checked: list.iter().any(|x| x == c.id), font: "".into() }).collect();
    ui.global::<App>().set_all_commands(ModelRc::new(VecModel::from(items)));
}
pub fn object_selected(st: &mut State) {
    let Some(ui) = st.ui.upgrade() else { return };
    let app = ui.global::<App>();
    if let Some(o) = st.ed.object() {
        if let ObjKind::Chart { chart } = &o.kind {
            app.set_chart_data(chart_text(chart).into());
            app.set_chart_title(chart.title.clone().into());
        }
        if let ObjKind::PathText { story, .. } = &o.kind {
            app.set_path_text(story.text().into());
        }
        if let ObjKind::Table { table } = &o.kind {
            app.set_table_data(table.rows.iter().map(|r| r.join(";")).collect::<Vec<_>>().join("\n").into());
        }
    }
}
fn chart_text(ch: &Chart) -> String {
    let mut s = String::from(";");
    s.push_str(&ch.series.iter().map(|x| x.name.clone()).collect::<Vec<_>>().join(";"));
    for (i, l) in ch.labels.iter().enumerate() {
        s.push('\n');
        s.push_str(l);
        for se in &ch.series {
            s.push(';');
            s.push_str(&se.values.get(i).map(|v| num(*v)).unwrap_or_default());
        }
    }
    s
}
fn parse_chart(t: &str) -> (Vec<String>, Vec<Series>) {
    let rows: Vec<Vec<String>> = t.lines().filter(|l| !l.trim().is_empty()).map(|l| l.split([';', '\t']).map(|c| c.trim().to_string()).collect()).collect();
    if rows.len() < 2 {
        return (Vec::new(), Vec::new());
    }
    let names: Vec<String> = rows[0].iter().skip(1).cloned().collect();
    let mut series: Vec<Series> = names.iter().map(|n| Series { name: n.clone(), values: Vec::new(), color: None }).collect();
    let mut labels = Vec::new();
    for r in &rows[1..] {
        labels.push(r.first().cloned().unwrap_or_default());
        for (i, s) in series.iter_mut().enumerate() {
            s.values.push(r.get(i + 1).and_then(|v| parse_num(v)).unwrap_or(0.0));
        }
    }
    (labels, series)
}
/// Sekcja, w której stoi kursor (tylko treść główna).
fn section_of(st: &mut State) -> Option<&mut Section> {
    if st.ed.story != StoryRef::Body {
        return None;
    }
    let i = st.ed.doc.section_start(st.ed.head.para)?;
    st.ed.doc.body.paras[i].fmt.section.as_mut()
}
