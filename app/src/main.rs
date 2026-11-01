//! OpenDTP – edytor tekstu i skład dokumentów dla Windows.
//! Pomysł i wykonanie: Adam Rędzikowski · redzikowski.com

#![windows_subsystem = "windows"]

mod actions;
mod commands;
mod editor;
mod files;
mod input;
mod settings;
mod view;
mod win;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use opendtp_core::model::*;
use opendtp_core::update::Release;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use editor::Editor;
use settings::Prefs;
use view::View;

slint::include_modules!();

pub use opendtp_core::lang::tr;

pub struct State {
    pub ed: Editor,
    pub view: View,
    pub prefs: Prefs,
    pub ui: slint::Weak<AppWindow>,
    /// Skopiowany fragment (z tekstem, którym trafił do schowka systemu).
    pub clip: Option<(String, Vec<Paragraph>)>,
    pub drag: input::Drag,
    pub clicks: input::Clicks,
    pub update: Option<Release>,
    pub fonts: Vec<String>,
    pub doc_theme: String,
    pub styles_key: String,
    pub nav_version: u64,
    pub session: String,
    pub pending_close: bool,
    pub after_confirm: Option<String>,
    pub update_busy: bool,
    pub pen: Vec<(f32, f32, Option<(f32, f32)>)>,
    pub pen_page: usize,
    pub shape_preset: String,
    pub text_shape: bool,
    pub caret_shown: std::time::Instant,
    /// Pobrany instalator nowej wersji – uruchamiany przy zamknięciu programu.
    pub pending_setup: Option<std::path::PathBuf>,
}

pub type Shared = Rc<RefCell<State>>;

thread_local! {
    static STATE: RefCell<Option<Shared>> = const { RefCell::new(None) };
}

pub fn shared() -> Option<Shared> {
    STATE.with(|s| s.borrow().clone())
}

// ---------------------------------------------------------------- jednostki

pub fn unit_k(unit: i32) -> f32 {
    match unit {
        0 => MM,
        1 => MM * 10.0,
        2 => 72.0,
        _ => 1.0,
    }
}

pub fn unit_name(unit: i32) -> &'static str {
    match unit {
        0 => "mm",
        1 => "cm",
        2 => "in",
        _ => "pt",
    }
}

pub fn num(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    let s = if (r - r.round()).abs() < 0.005 { format!("{}", r.round() as i64) } else if (r * 10.0 - (r * 10.0).round()).abs() < 0.005 { format!("{r:.1}") } else { format!("{r:.2}") };
    if opendtp_core::lang::current() == "pl" {
        s.replace('.', ",")
    } else {
        s
    }
}

pub fn parse_num(s: &str) -> Option<f32> {
    s.trim().trim_end_matches(|c: char| c.is_alphabetic() || c == '%').trim().replace(',', ".").parse::<f32>().ok()
}

pub fn len_str(pt: f32, unit: i32) -> String {
    num(pt / unit_k(unit))
}

// ---------------------------------------------------------------- odświeżanie

/// Po każdej zmianie: położenie i obrazy stron, nakładki i stan wstążki.
pub fn refresh(st: &mut State) {
    let Some(ui) = st.ui.upgrade() else { return };
    let app = ui.global::<App>();
    if st.view.place(&st.ed) {
        app.set_content_w(st.view.content_w);
        app.set_content_h(st.view.content_h);
    }
    st.view.render(&st.ed);
    st.view.overlays(&st.ed, &ui);
    sync(st, &ui);
    app.set_caret_blink(true);
    st.caret_shown = std::time::Instant::now();
}

/// Odświeżenie z przewinięciem do kursora.
pub fn refresh_scroll(st: &mut State) {
    refresh(st);
    if let Some((vx, vy)) = st.view.scroll_to_caret(&st.ed) {
        if let Some(ui) = st.ui.upgrade() {
            let app = ui.global::<App>();
            app.set_view_x(-vx);
            app.set_view_y(-vy);
        }
        st.view.vx = vx;
        st.view.vy = vy;
        st.view.render(&st.ed);
    }
}

fn wrap_index(m: WrapMode) -> i32 {
    match m {
        WrapMode::None => 0,
        WrapMode::Both => 1,
        WrapMode::Left => 2,
        WrapMode::Right => 3,
        WrapMode::Largest => 4,
        WrapMode::TopBottom => 5,
        WrapMode::Behind => 6,
    }
}

pub fn list_id(k: ListKind) -> &'static str {
    match k {
        ListKind::Bullet => "bullet",
        ListKind::Dash => "dash",
        ListKind::Check => "check",
        ListKind::Arrow => "arrow",
        ListKind::Number => "number",
        ListKind::NumberParen => "number-paren",
        ListKind::LowerLetter => "lower-letter",
        ListKind::UpperLetter => "upper-letter",
        ListKind::LowerRoman => "lower-roman",
        ListKind::UpperRoman => "upper-roman",
    }
}

fn slint_color(c: Color) -> slint::Color {
    slint::Color::from_argb_u8(c.a, c.r, c.g, c.b)
}

pub fn page_size_name(ps: &PageSetup) -> String {
    let (w, h) = if ps.width < ps.height { (ps.width, ps.height) } else { (ps.height, ps.width) };
    let near = |a: f32, b: f32| (a - b).abs() < 2.0;
    for (n, sw, sh) in actions::PAGE_SIZES {
        if near(w, sw * MM) && near(h, sh * MM) {
            return n.to_string();
        }
    }
    format!("{}×{}", num(ps.width / MM), num(ps.height / MM))
}

fn sync(st: &mut State, ui: &AppWindow) {
    let app = ui.global::<App>();
    let ed = &st.ed;
    let unit = st.prefs.unit;
    let (cp, pp, style) = ed.caret_props();
    app.set_font(cp.font.clone().into());
    app.set_size(num(cp.size).into());
    app.set_bold(cp.bold);
    app.set_italic(cp.italic);
    app.set_underline(cp.underline != Underline::None);
    app.set_strike(cp.strike);
    app.set_sup(cp.script == Script::Super);
    app.set_sub(cp.script == Script::Sub);
    app.set_caps(match cp.caps {
        Caps::None => 0,
        Caps::All => 1,
        Caps::Small => 2,
    });
    app.set_align(match pp.align {
        Align::Left => 0,
        Align::Center => 1,
        Align::Right => 2,
        Align::Justify => 3,
    });
    app.set_list_kind(pp.list.map(|l| list_id(l.kind)).unwrap_or("").into());
    app.set_line_spacing(match pp.line {
        LineSpacing::Multiple(k) => num(k),
        LineSpacing::Exact(v) => format!("{} pt", num(v)),
        LineSpacing::AtLeast(v) => format!("≥{} pt", num(v)),
    }.into());
    app.set_style_id(style.into());
    app.set_indent_left(len_str(pp.indent_left, unit).into());
    app.set_indent_right(len_str(pp.indent_right, unit).into());
    app.set_first_line(len_str(pp.first_line, unit).into());
    app.set_space_before(num(pp.space_before).into());
    app.set_space_after(num(pp.space_after).into());
    app.set_tracking(num(cp.tracking).into());
    app.set_drop_cap(pp.drop_cap > 0);
    app.set_shading(slint_color(pp.shading));
    app.set_unit(unit_name(unit).into());
    app.set_in_frame(matches!(ed.story, StoryRef::Obj(_)));

    let ps = &ed.doc.page;
    app.set_page_size(page_size_name(ps).into());
    app.set_landscape(ps.landscape());
    app.set_columns(ps.columns as i32);
    app.set_col_gap(len_str(ps.column_gap, unit).into());
    app.set_col_rule(ps.column_rule);
    app.set_m_top(len_str(ps.margin_top, unit).into());
    app.set_m_bottom(len_str(ps.margin_bottom, unit).into());
    app.set_m_left(len_str(ps.margin_left, unit).into());
    app.set_m_right(len_str(ps.margin_right, unit).into());
    app.set_page_color(slint_color(ps.color));
    app.set_has_header(ed.doc.header.is_some());
    app.set_has_footer(ed.doc.footer.is_some());
    app.set_diff_first(ps.different_first);
    app.set_orphans(ed.doc.settings.orphans);
    app.set_baseline_grid(ed.doc.settings.baseline_grid > 0.0);
    app.set_doc_theme(st.doc_theme.clone().into());

    if let Some(o) = ed.object() {
        app.set_wrap_mode(wrap_index(o.wrap.mode));
        app.set_wrap_dist(num(o.wrap.dist).into());
        app.set_wrap_contour(o.wrap.contour);
        match &o.fill {
            Some(Fill::Solid { color }) => {
                app.set_has_fill(true);
                app.set_fill(slint_color(*color));
                app.set_fill_kind(0);
            }
            Some(Fill::Linear { from, to, .. }) => {
                app.set_has_fill(true);
                app.set_fill(slint_color(*from));
                app.set_fill2(slint_color(*to));
                app.set_fill_kind(1);
            }
            Some(Fill::Radial { from, to }) => {
                app.set_has_fill(true);
                app.set_fill(slint_color(*from));
                app.set_fill2(slint_color(*to));
                app.set_fill_kind(2);
            }
            None => app.set_has_fill(false),
        }
        match &o.stroke {
            Some(s) => {
                app.set_has_stroke(s.width > 0.0);
                app.set_stroke(slint_color(s.color));
                app.set_stroke_w(num(s.width).into());
                app.set_dashed(s.dash.len() >= 2);
            }
            None => {
                app.set_has_stroke(false);
                app.set_stroke_w("0".into());
                app.set_dashed(false);
            }
        }
        app.set_shadow(o.shadow);
        app.set_opacity(o.opacity);
        app.set_corner(num(o.corner).into());
        app.set_obj_x(len_str(o.x, unit).into());
        app.set_obj_y(len_str(o.y, unit).into());
        app.set_obj_w(len_str(o.w, unit).into());
        app.set_obj_h(len_str(o.h, unit).into());
        app.set_obj_name(if o.name.is_empty() { actions::kind_label(o) } else { o.name.clone() }.into());
        let linked = ed.doc.chain(ed.doc.chain_head(o.id)).len() > 1;
        app.set_linked(linked);
        match &o.kind {
            ObjKind::Text { padding, columns, valign, .. } => {
                app.set_padding(num(*padding).into());
                app.set_frame_cols(*columns as i32);
                app.set_valign(match valign {
                    VAlign::Top => 0,
                    VAlign::Center => 1,
                    VAlign::Bottom => 2,
                });
            }
            ObjKind::Image { fit, .. } => app.set_image_fit_cover(*fit == ImageFit::Cover),
            ObjKind::Chart { chart } => {
                app.set_chart_kind(match chart.kind {
                    ChartKind::Column => 0,
                    ChartKind::Bar => 1,
                    ChartKind::Line => 2,
                    ChartKind::Area => 3,
                    ChartKind::Pie => 4,
                    ChartKind::Doughnut => 5,
                });
                app.set_chart_legend(chart.legend);
                app.set_chart_values(chart.values);
                if !app.get_chart_data().is_empty() && st.drag.is_none() {
                    // Dane w polu edycji zostawiamy, dopóki użytkownik ich nie zastosuje.
                }
            }
            ObjKind::PathText { story, offset } => {
                app.set_path_offset(*offset);
                let t = story.text();
                if app.get_path_text().as_str() != t {
                    app.set_path_text(t.into());
                }
            }
            ObjKind::Shape => {}
        }
    } else {
        app.set_linked(false);
    }

    // Stan programu.
    let page = ed.caret_page().map(|c| c.0).unwrap_or(0);
    app.set_page_info(opendtp_core::trf!("Strona {} z {}", page + 1, ed.layout.pages.len().max(1)).into());
    let (words, _, _, _, _) = ed.stats();
    app.set_words(opendtp_core::trf!("Wyrazy: {}", words).into());
    app.set_spell_state(if !st.view.spell {
        tr("Pisownia wyłączona").into()
    } else if !st.view.spell_ready {
        tr("Wczytywanie słowników…").into()
    } else if cp.lang.starts_with("en") {
        "English (US)".into()
    } else {
        tr("Polski").into()
    });
    app.set_dirty(ed.dirty);
    let name = files::doc_name(ed);
    app.set_doc_name(name.clone().into());
    app.set_title(format!("{}{} – OpenDTP", name, if ed.dirty { " •" } else { "" }).into());
    app.set_marks(st.view.marks);
    app.set_guides(st.view.guides);
    app.set_spell_live(st.view.spell);
    app.set_zoom((st.view.zoom * 100.0).round() as i32);
    app.set_node_mode(ed.node_mode);

    // Style (tylko po zmianie).
    let key = format!("{:?}", ed.doc.styles);
    if key != st.styles_key {
        st.styles_key = key;
        let items: Vec<StyleV> = ed
            .doc
            .styles
            .iter()
            .filter(|s| s.gallery)
            .map(|s| {
                let p = Paragraph { style: s.id.clone(), fmt: ParaFmt::default(), runs: vec![Run::default()] };
                let (pp, cp) = ed.doc.resolve_para(&p);
                StyleV {
                    id: s.id.clone().into(),
                    name: tr_style(&s.name).into(),
                    font: cp.font.clone().into(),
                    size: cp.size * 1.333,
                    bold: cp.bold,
                    italic: cp.italic,
                    color: slint_color(cp.color),
                    shade: slint_color(pp.shading),
                }
            })
            .collect();
        app.set_styles(ModelRc::new(VecModel::from(items)));
    }
    // Nawigator.
    if st.nav_version != ed.version {
        st.nav_version = ed.version;
        let cur = if ed.story == StoryRef::Body { ed.head.para } else { usize::MAX };
        let mut items = Vec::new();
        let heads: Vec<(usize, u8, String)> = ed
            .doc
            .body
            .paras
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let lvl = ed.doc.outline_level(p).or(if p.style == "title" { Some(0) } else { None })?;
                let t = p.text().replace(FIELD_CHAR, "");
                if t.trim().is_empty() {
                    None
                } else {
                    Some((i, lvl, t))
                }
            })
            .collect();
        for (k, (i, lvl, t)) in heads.iter().enumerate() {
            let next = heads.get(k + 1).map(|h| h.0).unwrap_or(usize::MAX);
            items.push(NavV {
                label: t.trim().into(),
                level: (*lvl as i32 - 1).max(0),
                para: *i as i32,
                page: ed.layout.body_pages.get(*i).copied().unwrap_or(0) as i32,
                current: cur >= *i && cur < next,
            });
        }
        app.set_nav(ModelRc::new(VecModel::from(items)));
    }
}

/// Nazwy stylów wbudowanych w języku interfejsu.
pub fn tr_style(name: &str) -> String {
    opendtp_core::lang::tr_owned(name)
}

pub fn toast(st: &State, msg: &str) {
    if let Some(ui) = st.ui.upgrade() {
        let app = ui.global::<App>();
        app.set_toast(msg.into());
        app.set_toast_serial(app.get_toast_serial() + 1);
    }
}

pub fn set_status(st: &State, msg: &str) {
    if let Some(ui) = st.ui.upgrade() {
        ui.global::<App>().set_status(msg.into());
    }
}

/// Modele list zależne od ustawień: pasek szybkiego dostępu, Moja karta, ostatnie pliki.
pub fn sync_lists(st: &State) {
    let Some(ui) = st.ui.upgrade() else { return };
    let app = ui.global::<App>();
    let item = |id: &str| -> Option<ItemV> {
        let c = commands::find(id)?;
        Some(ItemV { id: id.into(), label: commands::label(c).into(), hint: c.key.into(), icon: commands::icon(c.icon), checked: false, font: "".into() })
    };
    app.set_qat(ModelRc::new(VecModel::from(st.prefs.qat.iter().filter_map(|i| item(i)).collect::<Vec<_>>())));
    app.set_custom_tab(ModelRc::new(VecModel::from(st.prefs.custom_tab.iter().filter_map(|i| item(i)).collect::<Vec<_>>())));
    let recent: Vec<RecentV> = st
        .prefs
        .recent
        .iter()
        .map(|(p, when)| RecentV {
            name: std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default().into(),
            path: p.clone().into(),
            when: when.clone().into(),
            pinned: false,
        })
        .collect();
    app.set_recent(ModelRc::new(VecModel::from(recent)));
    let rf: Vec<ItemV> = st.prefs.recent_fonts.iter().map(|f| ItemV { id: f.clone().into(), label: f.clone().into(), ..Default::default() }).collect();
    app.set_recent_fonts(ModelRc::new(VecModel::from(rf)));
    let themes: Vec<ItemV> = opendtp_core::styles::THEMES.iter().map(|t| ItemV { id: t.id.into(), label: tr(t.name).into(), font: t.heading.into(), ..Default::default() }).collect();
    app.set_doc_themes(ModelRc::new(VecModel::from(themes)));
}

pub fn apply_theme_prefs(st: &State, ui: &AppWindow) {
    let th = ui.global::<Theme>();
    th.set_mode(st.prefs.theme);
    if let Some(c) = Color::parse(&st.prefs.accent) {
        th.set_accent(slint::Color::from_rgb_u8(c.r, c.g, c.b));
    }
    th.set_corners(st.prefs.corners);
    th.set_scale(st.prefs.scale.clamp(0.8, 1.5));
    th.set_dense(st.prefs.dense);
    let app = ui.global::<App>();
    app.set_accent_hex(st.prefs.accent.clone().into());
    app.set_s_unit(st.prefs.unit);
    app.set_s_autosave(st.prefs.autosave);
    app.set_s_update_check(st.prefs.update_check);
    app.set_s_update_auto(st.prefs.update_auto);
    app.set_s_smart_quotes(st.prefs.smart_quotes);
    app.set_s_autocorrect(st.prefs.autocorrect);
    app.set_s_capitalize(st.prefs.capitalize);
    app.set_s_autolist(st.prefs.autolist);
    app.set_s_restore(st.prefs.restore);
    app.set_s_snap(st.prefs.snap);
    app.set_s_author(st.prefs.author.clone().into());
    app.set_s_default_font(st.prefs.default_font.clone().into());
    app.set_s_default_size(num(st.prefs.default_size).into());
    app.set_s_lang(match st.prefs.lang.as_str() {
        "pl" => 1,
        "en" => 2,
        _ => 0,
    });
    app.set_ribbon_collapsed(st.prefs.ribbon_collapsed);
    app.set_exact_shapes(st.prefs.exact_shapes);
    win::dark_title(ui, st.prefs.theme != 2);
    sync_ai(st, ui);
}

/// Ustawienia AI w interfejsie (dostawca, model, temperatura, adres).
pub fn sync_ai(st: &State, ui: &AppWindow) {
    let app = ui.global::<App>();
    let provs: Vec<ItemV> = opendtp_core::ai::PROVIDERS.iter().map(|p| ItemV { id: p.id.into(), label: tr(p.name).into(), ..Default::default() }).collect();
    app.set_ai_providers(ModelRc::new(VecModel::from(provs)));
    let idx = opendtp_core::ai::PROVIDERS.iter().position(|p| p.id == st.prefs.ai_provider).unwrap_or(0);
    app.set_ai_provider(idx as i32);
    let p = &opendtp_core::ai::PROVIDERS[idx];
    app.set_ai_has_key(!st.prefs.ai_key.is_empty());
    app.set_ai_model(st.prefs.ai_model.clone().into());
    app.set_ai_temp(st.prefs.ai_temp);
    app.set_ai_base(if st.prefs.ai_base.is_empty() { p.base.to_string() } else { st.prefs.ai_base.clone() }.into());
    app.set_ai_needs_base(p.id == "custom" || p.id == "ollama");
    if app.get_ai_models().row_count() == 0 {
        let m: Vec<ItemV> = p.fallback.iter().map(|m| ItemV { id: (*m).into(), label: (*m).into(), ..Default::default() }).collect();
        app.set_ai_models(ModelRc::new(VecModel::from(m)));
    }
    if app.get_ai_status().is_empty() {
        app.set_ai_status(if st.prefs.ai_model.is_empty() { tr("Wybierz dostawcę, wklej klucz i wybierz model").into() } else { opendtp_core::trf!("Model: {}", st.prefs.ai_model).into() });
    }
}

fn main() {
    let prefs = Prefs::load();
    let code = opendtp_core::lang::init(&prefs.lang);
    let ui = AppWindow::new().expect("okno");
    let slint_code = if code == opendtp_core::lang::SOURCE { "" } else { code.as_str() };
    let _ = slint::select_bundled_translation(slint_code);

    // Czcionki i słowniki w tle – okno pojawia się od razu.
    std::thread::spawn(opendtp_core::fonts::warm_up);
    std::thread::spawn(opendtp_core::spell::load);

    let args: Vec<String> = std::env::args().skip(1).collect();
    let session = format!("{}", std::process::id());
    let mut st = State {
        ed: Editor::new(files::blank(&prefs)),
        view: View::new(),
        prefs,
        ui: ui.as_weak(),
        clip: None,
        drag: input::Drag::None,
        clicks: input::Clicks::default(),
        update: None,
        fonts: Vec::new(),
        doc_theme: "classic".into(),
        styles_key: String::new(),
        nav_version: 0,
        session,
        pending_close: false,
        after_confirm: None,
        update_busy: false,
        pen: Vec::new(),
        pen_page: 0,
        shape_preset: String::new(),
        text_shape: false,
        caret_shown: std::time::Instant::now(),
        pending_setup: None,
    };
    st.view.zoom = (st.prefs.zoom as f32 / 100.0).clamp(0.25, 4.0);
    st.view.marks = st.prefs.marks;
    st.view.guides = st.prefs.guides;
    st.view.spell = st.prefs.spell_live;
    st.view.dpr = ui.window().scale_factor();
    {
        let app = ui.global::<App>();
        app.set_version(opendtp_core::version().into());
        app.set_author_line(opendtp_core::AUTHOR_LINE.into());
        app.set_portable(opendtp_core::paths::portable());
        app.set_pages(st.view.model());
        app.set_tab(st.prefs.tab.clamp(0, 8));
        app.set_update_status(opendtp_core::trf!("Wersja {} · jeszcze nie sprawdzano", opendtp_core::version()).into());
    }
    apply_theme_prefs(&st, &ui);
    sync_lists(&st);

    let shared: Shared = Rc::new(RefCell::new(st));
    STATE.with(|s| *s.borrow_mut() = Some(shared.clone()));

    // Dokument startowy: plik z wiersza poleceń, kopia odzyskana, ostatni dokument albo przewodnik.
    {
        let mut st = shared.borrow_mut();
        let opened = args.iter().find(|a| !a.starts_with("--") && !a.ends_with(".png") && a.parse::<i32>().is_err()).map(|p| files::open_path(&mut st, p)).unwrap_or(false);
        if !opened && !files::recover(&mut st) {
            let last = st.prefs.last_doc.clone();
            let ok = st.prefs.restore && !last.is_empty() && std::path::Path::new(&last).exists() && files::open_path(&mut st, &last);
            if !ok {
                let first_run = st.prefs.recent.is_empty();
                if first_run {
                    st.ed = Editor::new(opendtp_core::sample::template("welcome"));
                }
            }
        }
    }

    wire(&ui, &shared);

    // Mruganie kursora.
    let blink = slint::Timer::default();
    {
        let w = ui.as_weak();
        let sh = shared.clone();
        blink.start(slint::TimerMode::Repeated, Duration::from_millis(530), move || {
            let Some(ui) = w.upgrade() else { return };
            let Ok(st) = sh.try_borrow() else { return };
            if st.caret_shown.elapsed() < Duration::from_millis(500) {
                return;
            }
            let app = ui.global::<App>();
            app.set_caret_blink(!app.get_caret_blink());
        });
    }
    // Słowniki gotowe – przerysowanie z podkreśleniami.
    let spell_poll = slint::Timer::default();
    {
        let sh = shared.clone();
        spell_poll.start(slint::TimerMode::Repeated, Duration::from_millis(400), move || {
            let Ok(mut st) = sh.try_borrow_mut() else { return };
            if !st.view.spell_ready && opendtp_core::spell::ready() {
                st.view.spell_ready = true;
                st.view.invalidate();
                refresh(&mut st);
            }
        });
    }
    // Lista czcionek, gdy baza się wczyta.
    let font_poll = slint::Timer::default();
    {
        let sh = shared.clone();
        font_poll.start(slint::TimerMode::SingleShot, Duration::from_millis(300), move || {
            let mut st = sh.borrow_mut();
            st.fonts = opendtp_core::fonts::families();
            actions::font_query(&st, "");
            st.ed.relayout();
            st.view.invalidate();
            refresh(&mut st);
        });
    }
    // Autozapis kopii do odzyskania.
    let autosave = slint::Timer::default();
    {
        let sh = shared.clone();
        autosave.start(slint::TimerMode::Repeated, Duration::from_secs(30), move || {
            let Ok(mut st) = sh.try_borrow_mut() else { return };
            files::autosave_tick(&mut st);
        });
    }
    // Aktualizacje: minutę po starcie, potem co 6 godzin.
    let upd_first = slint::Timer::default();
    let upd = slint::Timer::default();
    upd_first.start(slint::TimerMode::SingleShot, Duration::from_secs(60), || actions::check_update(false));
    upd.start(slint::TimerMode::Repeated, Duration::from_secs(6 * 3600), || actions::check_update(false));

    {
        let mut st = shared.borrow_mut();
        refresh(&mut st);
    }
    {
        let sh = shared.clone();
        ui.window().on_close_requested(move || {
            let Ok(mut st) = sh.try_borrow_mut() else { return slint::CloseRequestResponse::KeepWindowShown };
            if st.ed.dirty && !st.pending_close {
                files::confirm(&mut st, "exit");
                return slint::CloseRequestResponse::KeepWindowShown;
            }
            files::on_exit(&mut st);
            slint::CloseRequestResponse::HideWindow
        });
    }
    ui.show().expect("okno");
    {
        let sh = shared.clone();
        let w = ui.as_weak();
        slint::Timer::single_shot(Duration::from_millis(50), move || {
            if let Some(ui) = w.upgrade() {
                win::dark_title(&ui, sh.borrow().prefs.theme != 2);
                let mut st = sh.borrow_mut();
                st.view.dpr = ui.window().scale_factor();
                st.view.invalidate();
                refresh(&mut st);
                ui.global::<App>().set_focus_serial(1);
            }
        });
    }
    snapshot_mode(&ui, &args);
    slint::run_event_loop().expect("pętla zdarzeń");
}

fn wire(ui: &AppWindow, shared: &Shared) {
    let app = ui.global::<App>();
    {
        // Od razu (nie przez kolejkę poleceń) – przeciąganie okna musi ruszyć przy wciśniętym przycisku.
        let w = ui.as_weak();
        app.on_win(move |a| {
            if let Some(ui) = w.upgrade() {
                win::action(&ui, a);
            }
        });
    }
    {
        let sh = shared.clone();
        app.on_cmd(move |id, arg| {
            let id = id.to_string();
            let arg = arg.to_string();
            let sh = sh.clone();
            // Polecenia mogą otwierać okna systemowe – wykonujemy je poza bieżącym wywołaniem Slint.
            slint::Timer::single_shot(Duration::ZERO, move || {
                let mut st = sh.borrow_mut();
                actions::run(&mut st, &id, &arg);
            });
        });
    }
    {
        let sh = shared.clone();
        app.on_pointer(move |kind, x, y, button, shift, ctrl, alt| {
            let Ok(mut st) = sh.try_borrow_mut() else { return };
            input::pointer(&mut st, kind, x, y, button, shift, ctrl, alt);
        });
    }
    {
        let sh = shared.clone();
        app.on_hover(move |x, y| {
            let Ok(mut st) = sh.try_borrow_mut() else { return };
            input::hover(&mut st, x, y);
        });
    }
    {
        let sh = shared.clone();
        app.on_key(move |text, ctrl, shift, alt| {
            let Ok(mut st) = sh.try_borrow_mut() else { return false };
            input::key(&mut st, text.as_str(), ctrl, shift, alt)
        });
    }
    {
        let sh = shared.clone();
        app.on_wheel_zoom(move |delta, x, y| {
            let Ok(mut st) = sh.try_borrow_mut() else { return };
            let z = st.view.zoom * if delta > 0.0 { 1.1 } else { 1.0 / 1.1 };
            actions::set_zoom(&mut st, z, Some((x, y)));
        });
    }
    {
        let sh = shared.clone();
        app.on_viewport(move |x, y, w, h| {
            let sh2 = sh.clone();
            let apply = move || {
                let Ok(mut st) = sh2.try_borrow_mut() else { return };
                let resized = (st.view.vw - w).abs() > 0.5;
                if let Some(ui) = st.ui.upgrade() {
                    let m = ui.window().is_maximized();
                    if ui.global::<App>().get_maximized() != m {
                        ui.global::<App>().set_maximized(m);
                    }
                }
                st.view.vx = x;
                st.view.vy = y;
                st.view.vw = w;
                st.view.vh = h;
                if resized {
                    refresh(&mut st);
                } else {
                    let st = &mut *st;
                    st.view.render(&st.ed);
                }
            };
            slint::Timer::single_shot(Duration::ZERO, apply);
        });
    }
    {
        let sh = shared.clone();
        app.on_palette_query(move |q| {
            let st = sh.borrow();
            actions::palette_query(&st, q.as_str());
        });
    }
    {
        let sh = shared.clone();
        app.on_font_query(move |q| {
            let st = sh.borrow();
            actions::font_query(&st, q.as_str());
        });
    }
}

pub fn items(list: Vec<(String, String, String)>) -> ModelRc<ItemV> {
    ModelRc::new(VecModel::from(
        list.into_iter()
            .map(|(id, label, hint)| ItemV { id: SharedString::from(id), label: label.into(), hint: hint.into(), icon: Default::default(), checked: false, font: "".into() })
            .collect::<Vec<_>>(),
    ))
}

#[allow(dead_code)]
fn model_len<T: Clone + 'static>(m: &ModelRc<T>) -> usize {
    m.row_count()
}

/// Tryb diagnostyczny: `--snapshot plik.png [--tab N] [--backstage N] [--dialog nazwa] [--doc szablon]`
/// – po starcie zapisuje obraz okna i kończy program (sprawdzanie wyglądu bez interakcji).
fn snapshot_mode(ui: &AppWindow, args: &[String]) {
    let Some(i) = args.iter().position(|a| a == "--snapshot") else { return };
    let Some(out) = args.get(i + 1).cloned() else { return };
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|j| args.get(j + 1)).cloned();
    let app = ui.global::<App>();
    if let Some(t) = get("--tab").and_then(|v| v.parse().ok()) {
        app.set_tab(t);
    }
    if let Some(p) = get("--backstage").and_then(|v| v.parse().ok()) {
        app.set_backstage_page(p);
        app.set_backstage(true);
    }
    if let Some(d) = get("--dialog") {
        app.set_dialog(d.into());
    }
    if let Some(t) = get("--doc") {
        if let Some(sh) = shared() {
            let mut st = sh.borrow_mut();
            files::set_doc(&mut st, opendtp_core::sample::template(&t), None);
        }
    }
    if let Some(t) = get("--theme").and_then(|v| v.parse().ok()) {
        ui.global::<Theme>().set_mode(t);
    }
    let w = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(3500), move || {
        if let Some(ui) = w.upgrade() {
            if let Ok(buf) = ui.window().take_snapshot() {
                if let Some(img) = image::RgbaImage::from_raw(buf.width(), buf.height(), buf.as_bytes().to_vec()) {
                    let _ = img.save(&out);
                }
            }
        }
        let _ = slint::quit_event_loop();
    });
}
