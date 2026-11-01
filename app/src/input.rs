//! Mysz i klawiatura na płótnie: zaznaczanie tekstu, wybór, przesuwanie i skalowanie obiektów,
//! edycja węzłów, rysowanie piórem, tworzenie ramek oraz pisanie z autokorektą i skrótami.

use slint::ComponentHandle;
use std::time::Instant;

use opendtp_core::edit::At;
use opendtp_core::geom::{self, Rect};
use opendtp_core::model::*;
use slint::platform::Key;

use crate::editor::{Kind, Move};
use crate::view::hit_object;
use crate::{actions, commands, refresh, refresh_scroll, tr, App, State};

pub enum Drag {
    None,
    Select { story: StoryRef },
    Move { id: u64, sx: f32, sy: f32, orig: Vec<(u64, f32, f32)>, moved: bool },
    Resize { id: u64, handle: u8, orig: (f32, f32, f32, f32), sx: f32, sy: f32 },
    Node { id: u64, ci: usize, ni: usize, part: u8, moved: bool },
    Create { page: usize, sx: f32, sy: f32 },
    Col { id: u64, col: usize, sx: f32, orig: Vec<f32> },
    Pen { moved: bool },
}

impl Drag {
    pub fn is_none(&self) -> bool {
        matches!(self, Drag::None)
    }
}

#[derive(Default)]
pub struct Clicks {
    t: Option<Instant>,
    x: f32,
    y: f32,
    n: u32,
}

impl Clicks {
    fn hit(&mut self, x: f32, y: f32) -> u32 {
        let now = Instant::now();
        let again = self.t.is_some_and(|t| now.duration_since(t).as_millis() < 450) && (x - self.x).abs() < 5.0 && (y - self.y).abs() < 5.0;
        self.n = if again { self.n % 3 + 1 } else { 1 };
        self.t = Some(now);
        self.x = x;
        self.y = y;
        self.n
    }
}

fn app(st: &State) -> Option<slint::Weak<crate::AppWindow>> {
    Some(st.ui.clone())
}

fn set_cursor(st: &State, c: &str) {
    if let Some(ui) = st.ui.upgrade() {
        let a = ui.global::<App>();
        if a.get_cursor().as_str() != c {
            a.set_cursor(c.into());
        }
    }
}

fn tool(st: &State) -> i32 {
    st.ui.upgrade().map(|u| u.global::<App>().get_tool()).unwrap_or(0)
}

pub fn set_tool(st: &State, t: i32) {
    if let Some(ui) = st.ui.upgrade() {
        ui.global::<App>().set_tool(t);
        ui.global::<App>().set_cursor(if t == 0 { "text" } else { "cross" }.into());
    }
}

/// Uchwyt zmiany rozmiaru pod wskaźnikiem (0..7: rogi i boki, zgodnie z ruchem wskazówek od lewego górnego).
fn handle_at(st: &State, x: f32, y: f32) -> Option<u8> {
    let o = st.ed.object()?;
    if o.locked || st.ed.node_mode {
        return None;
    }
    let (x0, y0) = st.view.to_canvas(o.page, o.x, o.y);
    let k = st.view.k();
    let (w, h) = (o.w * k, o.h * k);
    let pts = [(x0, y0), (x0 + w / 2.0, y0), (x0 + w, y0), (x0 + w, y0 + h / 2.0), (x0 + w, y0 + h), (x0 + w / 2.0, y0 + h), (x0, y0 + h), (x0, y0 + h / 2.0)];
    pts.iter().position(|&(px, py)| (px - x).abs() <= 7.0 && (py - y).abs() <= 7.0).map(|i| i as u8)
}

/// Węzeł albo uchwyt pod wskaźnikiem: (kontur, węzeł, część 0 – węzeł, 1 – uchwyt przed, 2 – za).
fn node_at(st: &State, x: f32, y: f32) -> Option<(usize, usize, u8)> {
    let o = st.ed.object()?;
    let mut best: Option<(usize, usize, u8, f32)> = None;
    for (ci, c) in o.shape.contours.iter().enumerate() {
        for (ni, n) in c.nodes.iter().enumerate() {
            for (part, p) in [(0u8, Some((n.x, n.y))), (1, n.cin), (2, n.cout)] {
                let Some((px, py)) = p else { continue };
                let (cx, cy) = st.view.to_canvas(o.page, o.x + px * o.w, o.y + py * o.h);
                let d = ((cx - x).powi(2) + (cy - y).powi(2)).sqrt();
                if d <= 8.0 && best.is_none_or(|b| d < b.3 || (part == 0 && b.2 != 0 && d <= b.3 + 2.0)) {
                    best = Some((ci, ni, part, d));
                }
            }
        }
    }
    best.map(|b| (b.0, b.1, b.2))
}

/// Najwyższy obiekt pod punktem (pomijamy „za tekstem”, chyba że `behind`).
fn object_at(st: &State, page: usize, x: f32, y: f32, behind: bool) -> Option<u64> {
    st.ed.doc.objects.iter().rev().find(|o| o.page == page && (o.wrap.mode == WrapMode::Behind) == behind && hit_object(o, x, y)).map(|o| o.id)
}

/// Granica kolumn zaznaczonej tabeli pod wskaźnikiem: (obiekt, kolumna, szerokości kolumn w pt).
fn col_edge_at(st: &State, page: usize, x: f32, y: f32) -> Option<(u64, usize, Vec<f32>)> {
    let o = st.ed.object()?;
    let ObjKind::Table { table } = &o.kind else { return None };
    if o.page != page || y < o.y || y > o.y + o.h {
        return None;
    }
    let r = Rect::new(o.x, o.y, o.w, o.h);
    let edges = opendtp_core::paint::table_col_edges(table, r);
    let tol = 4.0 / st.view.k();
    let col = edges.iter().position(|e| (e - x).abs() <= tol)?;
    let mut widths = Vec::new();
    let mut prev = o.x;
    for e in edges.iter().chain(std::iter::once(&(o.x + o.w))) {
        widths.push(e - prev);
        prev = *e;
    }
    Some((o.id, col, widths))
}

fn near_border(o: &Object, x: f32, y: f32, k: f32) -> bool {
    let tol = 6.0 / k;
    let inside = x > o.x + tol && x < o.x + o.w - tol && y > o.y + tol && y < o.y + o.h - tol;
    !inside
}

#[allow(clippy::too_many_arguments)]
pub fn pointer(st: &mut State, kind: i32, x: f32, y: f32, button: i32, shift: bool, ctrl: bool, alt: bool) {
    let (page, px, py) = st.view.to_page(x, y);
    match kind {
        0 => down(st, page, px, py, x, y, button, shift, ctrl, alt),
        1 => drag_move(st, page, px, py, x, y),
        _ => up(st, page, px, py),
    }
}

#[allow(clippy::too_many_arguments)]
fn down(st: &mut State, page: usize, px: f32, py: f32, x: f32, y: f32, button: i32, shift: bool, ctrl: bool, alt: bool) {
    let t = tool(st);
    let n = st.clicks.hit(x, y);
    if button == 1 {
        context(st, page, px, py);
        return;
    }
    match t {
        1 | 2 | 4 => {
            st.drag = Drag::Create { page, sx: px, sy: py };
            return;
        }
        3 => {
            pen_click(st, page, px, py);
            return;
        }
        5 => {
            if let Some(target) = object_at(st, page, px, py, false) {
                actions::link_frames(st, target);
            }
            set_tool(st, 0);
            refresh(st);
            return;
        }
        _ => {}
    }
    // Węzły zaznaczonego kształtu.
    if st.ed.node_mode {
        if let Some((ci, ni, part)) = node_at(st, x, y) {
            let id = st.ed.obj.unwrap();
            st.ed.sel_node = Some((ci, ni));
            if alt && part == 0 {
                st.ed.shape_edit(&|s, sel| {
                    if let Some((ci, ni)) = sel {
                        geom::toggle_smooth(s, ci, ni);
                    }
                    sel
                });
            } else {
                st.ed.begin(Kind::Other);
            }
            st.drag = Drag::Node { id, ci, ni, part, moved: false };
            refresh(st);
            return;
        }
        if let Some(o) = st.ed.object() {
            let r = Rect::new(o.x, o.y, o.w, o.h);
            if let Some((ci, si, tt, d)) = geom::nearest_segment(&o.shape, r, px, py) {
                if d * st.view.k() < 8.0 && (n == 2 || shift) {
                    st.ed.shape_edit(&|s, _| {
                        geom::insert_node(s, ci, si, tt);
                        Some((ci, si + 1))
                    });
                    refresh(st);
                    return;
                }
            }
            if o.page == page && o.contains(px, py) {
                return;
            }
        }
        st.ed.node_mode = false;
    }
    // Uchwyty zmiany rozmiaru.
    if let Some(h) = handle_at(st, x, y) {
        let o = st.ed.object().unwrap();
        let orig = (o.x, o.y, o.w, o.h);
        let id = o.id;
        st.ed.begin(Kind::Other);
        st.drag = Drag::Resize { id, handle: h, orig, sx: px, sy: py };
        return;
    }
    let k = st.view.k();
    // Granica kolumn zaznaczonej tabeli – zmiana szerokości.
    if let Some((id, col, orig)) = col_edge_at(st, page, px, py) {
        st.ed.begin(Kind::Other);
        st.drag = Drag::Col { id, col, sx: px, orig };
        return;
    }
    if let Some(mut id) = object_at(st, page, px, py, false) {
        // Kontynuacja tabeli na następnej stronie – działamy na oryginale.
        if let Some(orig) = st.ed.doc.object(id).and_then(|o| o.cont_of) {
            id = orig;
        }
        let o = st.ed.doc.object(id).unwrap().clone();
        if n == 2 && matches!(o.kind, ObjKind::Table { .. }) && o.cont_of.is_none() {
            if let Some((row, col, _)) = st.ed.doc.object(id).and_then(|oo| actions::cell_at(oo, px, py)) {
                st.ed.obj = Some(id);
                st.ed.cell = Some((id, row, col));
                st.drag = Drag::None;
                refresh(st);
                actions::show_cell(st);
                return;
            }
        }
        let is_text = matches!(o.kind, ObjKind::Text { .. });
        let head = st.ed.doc.chain_head(id);
        // Tekst ramki: kliknięcie w środek ustawia kursor, w krawędź (albo z Ctrl) – wybiera ramkę.
        if is_text && !shift && !ctrl && !near_border(&o, px, py, k) && !(st.ed.obj == Some(id) && st.ed.story != StoryRef::Obj(head) && n == 1 && !o.locked && false) {
            let story = StoryRef::Obj(head);
            st.ed.obj = Some(id);
            st.ed.multi.clear();
            st.ed.node_mode = false;
            if let Some((s, p, off)) = st.ed.layout.hit(page, px, py, Some(story)) {
                text_click(st, s, At::new(p, off), n, shift, ctrl);
            } else {
                st.ed.set_caret(story, At::new(0, 0), false);
            }
            st.drag = Drag::Select { story };
            refresh(st);
            return;
        }
        if shift && st.ed.obj.is_some() && st.ed.obj != Some(id) {
            if let Some(i) = st.ed.multi.iter().position(|m| *m == id) {
                st.ed.multi.remove(i);
            } else {
                st.ed.multi.push(id);
            }
            refresh(st);
            return;
        }
        if n == 2 && !is_text {
            st.ed.obj = Some(id);
            match o.kind {
                ObjKind::Chart { .. } => actions::run(st, "panel", "props"),
                _ => {
                    st.ed.node_mode = true;
                    st.ed.sel_node = None;
                }
            }
            refresh(st);
            return;
        }
        if st.ed.obj != Some(id) {
            st.ed.multi.clear();
        }
        st.ed.obj = Some(id);
        if let StoryRef::Obj(_) = st.ed.story {
            st.ed.story = StoryRef::Body;
            st.ed.fix_selection();
        }
        if !o.locked {
            st.ed.begin(Kind::Other);
            let mut orig = vec![(id, o.x, o.y)];
            for m in &st.ed.multi {
                if let Some(mo) = st.ed.doc.object(*m) {
                    orig.push((mo.id, mo.x, mo.y));
                }
            }
            st.drag = Drag::Move { id, sx: px, sy: py, orig, moved: false };
        }
        actions::object_selected(st);
        refresh(st);
        return;
    }
    // Tekst strony (treść główna, nagłówek, stopka).
    let hit = st.ed.layout.hit(page, px, py, None).filter(|(s, _, _)| !matches!(s, StoryRef::Obj(_)));
    let on_line = hit.and_then(|(s, p, off)| st.ed.layout.line_for(s, p, off).map(|li| (li, s, p, off)));
    let over_text = on_line.is_some_and(|(li, _, _, _)| {
        let l = &st.ed.layout.lines[li];
        l.page == page && py >= l.top - 6.0 && py <= l.bottom() + 6.0
    });
    if !over_text {
        if let Some(id) = object_at(st, page, px, py, true) {
            st.ed.obj = Some(id);
            st.ed.multi.clear();
            let o = st.ed.doc.object(id).unwrap().clone();
            if !o.locked {
                st.ed.begin(Kind::Other);
                st.drag = Drag::Move { id, sx: px, sy: py, orig: vec![(id, o.x, o.y)], moved: false };
            }
            refresh(st);
            return;
        }
    }
    st.ed.obj = None;
    st.ed.multi.clear();
    st.ed.node_mode = false;
    if let Some((s, p, off)) = hit {
        // Nagłówek i stopka tylko dwuklikiem (jak w Wordzie), chyba że już je edytujemy.
        let hf = matches!(s, StoryRef::Header | StoryRef::Footer | StoryRef::HeaderFirst | StoryRef::FooterFirst | StoryRef::HeaderEven | StoryRef::FooterEven);
        if hf && st.ed.story != s && n < 2 {
            if let Some((bs, bp, boff)) = st.ed.layout.hit(page, px, py, Some(StoryRef::Body)) {
                text_click(st, bs, At::new(bp, boff), 1, shift, ctrl);
                st.drag = Drag::Select { story: bs };
            }
            refresh(st);
            return;
        }
        text_click(st, s, At::new(p, off), if hf && st.ed.story != s { 1 } else { n }, shift, ctrl);
        st.drag = Drag::Select { story: s };
    } else if let Some((bs, bp, boff)) = st.ed.layout.hit(page, px, py, Some(StoryRef::Body)) {
        text_click(st, bs, At::new(bp, boff), n, shift, ctrl);
        st.drag = Drag::Select { story: bs };
    }
    refresh(st);
}

fn text_click(st: &mut State, s: StoryRef, at: At, n: u32, shift: bool, ctrl: bool) {
    st.ed.set_caret(s, at, shift && st.ed.story == s);
    if ctrl && n == 1 && !shift {
        st.ed.select_sentence(at);
    }
    match n {
        2 => st.ed.select_word(at),
        3 => st.ed.select_para(at.para),
        _ => {}
    }
}

fn snap(st: &State, page: usize, x: f32, y: f32, w: f32, h: f32, id: u64) -> (f32, f32, String) {
    if !st.prefs.snap {
        return (x, y, String::new());
    }
    let ps = &st.ed.doc.page;
    let tol = 5.0 / st.view.k();
    let mut xs = vec![ps.margin_left, ps.width - ps.margin_right, ps.width / 2.0, 0.0, ps.width];
    let mut ys = vec![ps.margin_top, ps.height - ps.margin_bottom, ps.height / 2.0, 0.0, ps.height];
    for o in st.ed.doc.objects.iter().filter(|o| o.page == page && o.id != id && !st.ed.multi.contains(&o.id)) {
        xs.extend([o.x, o.x + o.w, o.x + o.w / 2.0]);
        ys.extend([o.y, o.y + o.h, o.y + o.h / 2.0]);
    }
    let mut nx = x;
    let mut ny = y;
    let mut lines = String::new();
    let best = |vals: &[f32], cands: [f32; 3]| -> Option<(f32, f32)> {
        let mut b: Option<(f32, f32, f32)> = None;
        for &v in vals {
            for (i, c) in cands.iter().enumerate() {
                let d = (v - c).abs();
                if d < tol && b.is_none_or(|bb| d < bb.2) {
                    let off = match i {
                        0 => 0.0,
                        1 => w,
                        _ => w / 2.0,
                    };
                    b = Some((v - off, v, d));
                }
            }
        }
        b.map(|b| (b.0, b.1))
    };
    if let Some((sx, line)) = best(&xs, [x, x + w, x + w / 2.0]) {
        nx = sx;
        let (a, b) = st.view.to_canvas(page, line, 0.0);
        let (_, c) = st.view.to_canvas(page, line, ps.height);
        lines.push_str(&format!("M {a:.1} {b:.1} L {a:.1} {c:.1} "));
    }
    let besty = |vals: &[f32]| -> Option<(f32, f32)> {
        let mut b: Option<(f32, f32, f32)> = None;
        for &v in vals {
            for (i, c) in [y, y + h, y + h / 2.0].iter().enumerate() {
                let d = (v - c).abs();
                if d < tol && b.is_none_or(|bb| d < bb.2) {
                    let off = match i {
                        0 => 0.0,
                        1 => h,
                        _ => h / 2.0,
                    };
                    b = Some((v - off, v, d));
                }
            }
        }
        b.map(|b| (b.0, b.1))
    };
    if let Some((sy, line)) = besty(&ys) {
        ny = sy;
        let (a, b) = st.view.to_canvas(page, 0.0, line);
        let (c, _) = st.view.to_canvas(page, ps.width, line);
        lines.push_str(&format!("M {a:.1} {b:.1} L {c:.1} {b:.1} "));
    }
    (nx, ny, lines)
}

fn drag_move(st: &mut State, page: usize, px: f32, py: f32, x: f32, y: f32) {
    let k = st.view.k();
    match std::mem::replace(&mut st.drag, Drag::None) {
        Drag::Select { story } => {
            st.drag = Drag::Select { story };
            if let Some((s, p, off)) = st.ed.layout.hit(page, px, py, Some(story)) {
                if s == st.ed.story {
                    st.ed.head = At::new(p, off);
                    crate::refresh_scroll(st);
                }
            }
        }
        Drag::Move { id, sx, sy, orig, moved } => {
            let (dx, dy) = (px - sx, py - sy);
            let moved = moved || (dx.abs() + dy.abs()) * k > 3.0;
            if moved {
                let (w, h, cur_page) = st.ed.doc.object(id).map(|o| (o.w, o.h, o.page)).unwrap_or((0.0, 0.0, 0));
                let base = orig[0];
                // Przeciągnięcie na inną stronę przenosi obiekt.
                let target_page = if orig.len() == 1 { page } else { cur_page };
                let page_shift = if target_page != cur_page && orig.len() == 1 {
                    let (_, py0) = (0.0, 0.0);
                    let _ = py0;
                    true
                } else {
                    false
                };
                let (nx, ny, lines) = snap(st, target_page, base.1 + dx, base.2 + dy, w, h, id);
                let (fx, fy) = (nx - base.1, ny - base.2);
                for &(oid, ox, oy) in &orig {
                    let tp = target_page;
                    st.ed.drag_obj(oid, &|o| {
                        o.x = ox + fx;
                        o.y = if page_shift { py - (sy - oy) } else { oy + fy };
                        if page_shift {
                            o.page = tp;
                        }
                    });
                }
                if let Some(ui) = st.ui.upgrade() {
                    ui.global::<App>().set_snap_lines(lines.into());
                }
                refresh(st);
            }
            st.drag = Drag::Move { id, sx, sy, orig, moved };
        }
        Drag::Resize { id, handle, orig, sx, sy } => {
            let (ox, oy, ow, oh) = orig;
            let (dx, dy) = (px - sx, py - sy);
            let (mut x0, mut y0, mut x1, mut y1) = (ox, oy, ox + ow, oy + oh);
            match handle {
                0 => {
                    x0 += dx;
                    y0 += dy;
                }
                1 => y0 += dy,
                2 => {
                    x1 += dx;
                    y0 += dy;
                }
                3 => x1 += dx,
                4 => {
                    x1 += dx;
                    y1 += dy;
                }
                5 => y1 += dy,
                6 => {
                    x0 += dx;
                    y1 += dy;
                }
                _ => x0 += dx,
            }
            let keep = st.ui.upgrade().is_some();
            let _ = keep;
            let (nx, ny, nw, nh) = (x0.min(x1), y0.min(y1), (x1 - x0).abs().max(4.0), (y1 - y0).abs().max(4.0));
            st.ed.drag_obj(id, &|o| {
                o.x = nx;
                o.y = ny;
                o.w = nw;
                o.h = nh;
            });
            refresh(st);
            st.drag = Drag::Resize { id, handle, orig, sx, sy };
        }
        Drag::Node { id, ci, ni, part, .. } => {
            if let Some(o) = st.ed.doc.object(id) {
                let (nx, ny) = ((px - o.x) / o.w.max(1.0), (py - o.y) / o.h.max(1.0));
                st.ed.drag_obj(id, &|o| {
                    let Some(n) = o.shape.contours.get_mut(ci).and_then(|c| c.nodes.get_mut(ni)) else { return };
                    match part {
                        0 => {
                            let (ddx, ddy) = (nx - n.x, ny - n.y);
                            n.x = nx;
                            n.y = ny;
                            n.cin = n.cin.map(|(a, b)| (a + ddx, b + ddy));
                            n.cout = n.cout.map(|(a, b)| (a + ddx, b + ddy));
                        }
                        1 | 2 => {
                            if part == 1 {
                                n.cin = Some((nx, ny));
                            } else {
                                n.cout = Some((nx, ny));
                            }
                            if n.smooth {
                                // Drugi uchwyt po przeciwnej stronie (zachowuje swoją długość).
                                let other = if part == 1 { n.cout } else { n.cin };
                                let len = other.map(|(a, b)| ((a - n.x).powi(2) + (b - n.y).powi(2)).sqrt()).unwrap_or(0.0);
                                let (vx, vy) = (n.x - nx, n.y - ny);
                                let vl = (vx * vx + vy * vy).sqrt().max(1e-6);
                                let p = (n.x + vx / vl * len, n.y + vy / vl * len);
                                if len > 0.0 {
                                    if part == 1 {
                                        n.cout = Some(p);
                                    } else {
                                        n.cin = Some(p);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                });
                st.ed.corner_reset(id);
                refresh(st);
            }
            st.drag = Drag::Node { id, ci, ni, part, moved: true };
        }
        Drag::Col { id, col, sx, orig } => {
            let d = px - sx;
            let min = 16.0;
            if col + 1 < orig.len() {
                let a = (orig[col] + d).clamp(min, orig[col] + orig[col + 1] - min);
                let b = orig[col] + orig[col + 1] - a;
                let mut w = orig.clone();
                w[col] = a;
                w[col + 1] = b;
                st.ed.drag_obj(id, &move |o| {
                    if let ObjKind::Table { table } = &mut o.kind {
                        table.widths = w.clone();
                    }
                });
                refresh(st);
            }
            st.drag = Drag::Col { id, col, sx, orig };
        }
        Drag::Create { page: p0, sx, sy } => {
            let (a, b) = st.view.to_canvas(p0, sx.min(px), sy.min(py));
            let (w, h) = ((px - sx).abs() * k, (py - sy).abs() * k);
            if let Some(ui) = st.ui.upgrade() {
                let app = ui.global::<App>();
                app.set_drag_box(crate::BoxV { x: a, y: b, w, h });
                app.set_drag_box_on(true);
            }
            st.drag = Drag::Create { page: p0, sx, sy };
        }
        Drag::Pen { moved } => {
            // Przeciąganie po postawieniu węzła – uchwyty krzywej.
            if let Some(last) = st.pen.last_mut() {
                let (lx, ly) = (last.0, last.1);
                if ((px - lx).abs() + (py - ly).abs()) * k > 3.0 {
                    last.2 = Some((px, py));
                }
            }
            pen_preview(st, Some((px, py)));
            st.drag = Drag::Pen { moved: moved || true };
            let _ = (x, y);
        }
        Drag::None => {}
    }
}

fn up(st: &mut State, page: usize, px: f32, py: f32) {
    if let Some(ui) = st.ui.upgrade() {
        let app = ui.global::<App>();
        app.set_snap_lines("".into());
        app.set_drag_box_on(false);
    }
    match std::mem::replace(&mut st.drag, Drag::None) {
        Drag::Select { .. } => {
            if tool(st) == 6 {
                actions::apply_painter(st);
            }
            actions::object_selected(st);
        }
        Drag::Move { moved, id, .. } => {
            if !moved {
                // Kliknięcie bez ruchu – nic do cofania.
                st.ed.break_group();
            } else if st.ed.anchored(id) {
                st.ed.update_anchor_offset(id);
            }
            st.ed.changed();
            refresh(st);
        }
        Drag::Col { .. } => {
            st.ed.changed();
            refresh(st);
        }
        Drag::Resize { .. } => {
            st.ed.changed();
            refresh(st);
        }
        Drag::Node { id, .. } => {
            st.ed.normalize_shape(id);
            st.ed.changed();
            refresh(st);
        }
        Drag::Create { page: p0, sx, sy } => {
            let _ = page;
            let (x0, y0) = (sx.min(px), sy.min(py));
            let (w, h) = ((px - sx).abs(), (py - sy).abs());
            actions::create_from_drag(st, p0, x0, y0, w, h);
        }
        Drag::Pen { .. } => {}
        Drag::None => {}
    }
}

pub fn hover(st: &mut State, x: f32, y: f32) {
    if !st.drag.is_none() {
        return;
    }
    let t = tool(st);
    if t == 3 {
        let (_, px, py) = st.view.to_page(x, y);
        if !st.pen.is_empty() {
            pen_preview(st, Some((px, py)));
        }
        set_cursor(st, "cross");
        return;
    }
    if t != 0 && t != 6 {
        set_cursor(st, if t == 5 { "pointer" } else { "cross" });
        return;
    }
    if t == 6 {
        set_cursor(st, "copy");
        return;
    }
    if st.ed.node_mode {
        set_cursor(st, if node_at(st, x, y).is_some() { "pointer" } else { "arrow" });
        return;
    }
    {
        let (page, px, py) = st.view.to_page(x, y);
        if col_edge_at(st, page, px, py).is_some() {
            set_cursor(st, "ew");
            return;
        }
    }
    if let Some(h) = handle_at(st, x, y) {
        set_cursor(st, match h {
            0 | 4 => "nwse",
            2 | 6 => "nesw",
            1 | 5 => "ns",
            _ => "ew",
        });
        return;
    }
    let (page, px, py) = st.view.to_page(x, y);
    if let Some(id) = object_at(st, page, px, py, false) {
        let o = st.ed.doc.object(id).unwrap();
        let c = match o.kind {
            ObjKind::Text { .. } if !near_border(o, px, py, st.view.k()) => "text",
            _ => "move",
        };
        set_cursor(st, c);
        return;
    }
    set_cursor(st, "text");
}

// ---------------------------------------------------------------- pióro

fn pen_click(st: &mut State, page: usize, px: f32, py: f32) {
    if st.pen.is_empty() {
        st.pen_page = page;
    }
    let (_, ppx, ppy) = if page != st.pen_page {
        let (cx, cy) = st.view.to_canvas(page, px, py);
        let (_, a, b) = st.view.to_page(cx, cy);
        (st.pen_page, a, b)
    } else {
        (page, px, py)
    };
    // Kliknięcie w pierwszy węzeł zamyka kształt.
    if st.pen.len() >= 3 {
        let (fx, fy, _) = st.pen[0];
        if ((fx - ppx).abs() + (fy - ppy).abs()) * st.view.k() < 12.0 {
            actions::finish_pen(st, true);
            return;
        }
    }
    st.pen.push((ppx, ppy, None));
    st.drag = Drag::Pen { moved: false };
    pen_preview(st, None);
}

pub fn pen_preview(st: &State, cursor: Option<(f32, f32)>) {
    let Some(ui) = st.ui.upgrade() else { return };
    let mut s = String::new();
    for (i, &(x, y, h)) in st.pen.iter().enumerate() {
        let (cx, cy) = st.view.to_canvas(st.pen_page, x, y);
        if i == 0 {
            s.push_str(&format!("M {cx:.1} {cy:.1} "));
        } else {
            let prev = st.pen[i - 1];
            let c1 = prev.2.unwrap_or((prev.0, prev.1));
            let c2 = h.map(|(hx, hy)| (2.0 * x - hx, 2.0 * y - hy)).unwrap_or((x, y));
            let (a, b) = st.view.to_canvas(st.pen_page, c1.0, c1.1);
            let (c, d) = st.view.to_canvas(st.pen_page, c2.0, c2.1);
            s.push_str(&format!("C {a:.1} {b:.1} {c:.1} {d:.1} {cx:.1} {cy:.1} "));
        }
        // Znacznik węzła.
        s.push_str(&format!("M {:.1} {:.1} L {:.1} {:.1} L {:.1} {:.1} L {:.1} {:.1} Z M {cx:.1} {cy:.1} ", cx - 3.0, cy - 3.0, cx + 3.0, cy - 3.0, cx + 3.0, cy + 3.0, cx - 3.0, cy + 3.0));
    }
    if let (Some((mx, my)), Some(&(lx, ly, lh))) = (cursor, st.pen.last()) {
        let (a, b) = st.view.to_canvas(st.pen_page, lx, ly);
        let (c, d) = st.view.to_canvas(st.pen_page, mx, my);
        if let Some((hx, hy)) = lh {
            let (e, f) = st.view.to_canvas(st.pen_page, hx, hy);
            s.push_str(&format!("M {a:.1} {b:.1} C {e:.1} {f:.1} {c:.1} {d:.1} {c:.1} {d:.1} "));
        } else {
            s.push_str(&format!("M {a:.1} {b:.1} L {c:.1} {d:.1} "));
        }
    }
    ui.global::<App>().set_preview_path(s.into());
}

// ---------------------------------------------------------------- menu kontekstowe

/// Menu kontekstowe: zwykłe pozycje (ikona, skrót, znacznik), separatory, nagłówki i rzędy ikon
/// z podpisem (np. opływanie, kolejność) – zwarte, a każda ikona ma podpowiedź.
struct Menu(Vec<crate::CtxV>);

/// Przycisk rzędu ikon: polecenie (puste – wyszarzony), podpowiedź, ikona, stan włączenia.
type Btn<'a> = (&'a str, &'a str, &'a str, bool);

impl Menu {
    fn add(&mut self, id: &str, label: &str, icon: &str, hint: &str) -> &mut Self {
        self.0.push(crate::CtxV { kind: 0, id: id.into(), label: label.into(), hint: hint.into(), icon: commands::icon(icon), ..Default::default() });
        self
    }
    /// Pozycja dostępna tylko, gdy `on` – inaczej wyszarzona.
    fn add_if(&mut self, on: bool, id: &str, label: &str, icon: &str, hint: &str) -> &mut Self {
        self.add(if on { id } else { "" }, label, icon, hint)
    }
    /// Pozycja-przełącznik ze znacznikiem stanu.
    fn check(&mut self, on: bool, id: &str, label: &str, icon: &str, hint: &str) -> &mut Self {
        self.add(id, label, icon, hint);
        self.0.last_mut().unwrap().checked = on;
        self
    }
    fn row(&mut self, label: &str, btns: &[Btn]) -> &mut Self {
        let row: Vec<crate::ItemV> = btns.iter().map(|&(id, tip, icon, on)| crate::ItemV { id: id.into(), label: tip.into(), icon: commands::icon(icon), checked: on, ..Default::default() }).collect();
        self.0.push(crate::CtxV { kind: 3, label: label.into(), row: slint::ModelRc::new(slint::VecModel::from(row)), ..Default::default() });
        self
    }
    fn head(&mut self, label: &str) -> &mut Self {
        self.sep();
        self.0.push(crate::CtxV { kind: 2, label: label.into(), ..Default::default() });
        self
    }
    fn sep(&mut self) -> &mut Self {
        if self.0.last().is_some_and(|i| i.kind != 1) {
            self.0.push(crate::CtxV { kind: 1, ..Default::default() });
        }
        self
    }
}

fn context(st: &mut State, page: usize, px: f32, py: f32) {
    let mut m = Menu(Vec::new());
    let on_obj = object_at(st, page, px, py, false).or_else(|| object_at(st, page, px, py, true));
    let in_frame_text = on_obj.and_then(|id| st.ed.doc.object(id)).is_some_and(|o| matches!(o.kind, ObjKind::Text { .. }) && !near_border(o, px, py, st.view.k()));
    if on_obj.is_some() && !in_frame_text {
        st.ed.obj = on_obj;
        refresh(st);
        let linked = ui_flag(st, |a| a.get_linked());
        let o = st.ed.object().unwrap();
        st.ctx_cell = None;
        let anchored = st.ed.anchored(o.id);
        let wrap = crate::wrap_index(o.wrap.mode);
        let (is_table, is_image, is_chart, is_text) = (matches!(o.kind, ObjKind::Table { .. }), matches!(o.kind, ObjKind::Image { .. }), matches!(o.kind, ObjKind::Chart { .. }), matches!(o.kind, ObjKind::Text { .. }));
        let locked = o.locked;
        let cell = if is_table { actions::cell_at(o, px, py) } else { None };
        let id = o.id;

        // Polecenia zależne od rodzaju obiektu – na górze, bo najczęściej po nie sięgamy.
        if let Some((row, col, _)) = cell {
            st.ctx_cell = Some((id, row, col));
            m.row(
                tr("Wstaw"),
                &[
                    ("tbl-row-above", tr("Wstaw wiersz powyżej"), "arrow-up-from-line", false),
                    ("tbl-row-below", tr("Wstaw wiersz poniżej"), "arrow-down-to-line", false),
                    ("tbl-col-left", tr("Wstaw kolumnę z lewej"), "arrow-left-to-line", false),
                    ("tbl-col-right", tr("Wstaw kolumnę z prawej"), "arrow-right-to-line", false),
                ],
            )
            .row(
                tr("Komórki"),
                &[
                    ("tbl-merge-right", tr("Scal z komórką z prawej"), "squares-unite", false),
                    ("tbl-split", tr("Rozdziel komórkę"), "square-split-vertical", false),
                    ("tbl-row-del", tr("Usuń wiersz"), "fold-vertical", false),
                    ("tbl-col-del", tr("Usuń kolumnę"), "unfold-horizontal", false),
                ],
            )
            .row(
                tr("Dane"),
                &[
                    ("table-sort", tr("Sortuj rosnąco według tej kolumny"), "arrow-down-to-line", false),
                    ("table-sort-desc", tr("Sortuj malejąco według tej kolumny"), "arrow-up-from-line", false),
                    ("table-sum", tr("Suma kolumny"), "sigma", false),
                    ("table-to-text", tr("Zamień w tekst"), "rows-3", false),
                ],
            )
            .add("panel:props", tr("Dane i wygląd tabeli…"), "layout-template", "")
            .sep();
        }
        if is_image {
            m.add("replace-image", tr("Zmień obraz…"), "image", "").add("detect-contour", tr("Wykryj kontur do opływania"), "scan", "").add("caption", tr("Dodaj podpis"), "captions", "").sep();
        }
        if is_chart {
            m.add("panel:props", tr("Dane i typ wykresu…"), "chart-column", "").sep();
        }
        if is_text {
            m.add("link-frame", tr("Połącz z następną ramką"), "link-2", "").add_if(linked, "unlink-frame", tr("Rozłącz ramki"), "unlink", "").sep();
        }

        m.add("cut", tr("Wytnij"), "scissors", "Ctrl+X")
            .add("copy", tr("Kopiuj"), "copy", "Ctrl+C")
            .add("paste", tr("Wklej"), "clipboard-paste", "Ctrl+V")
            .add("duplicate", tr("Duplikuj"), "files", "Ctrl+D")
            .add("delete-obj", tr("Usuń"), "trash-2", "Delete")
            .sep();

        if !is_chart && !is_table {
            m.row(
                tr("Kształt"),
                &[
                    ("node-mode", tr("Edytuj węzły kształtu"), "spline-pointer", false),
                    ("flip-h", tr("Odbij w poziomie"), "flip-horizontal", false),
                    ("flip-v", tr("Odbij w pionie"), "flip-vertical", false),
                ],
            );
        }
        m.row(
            tr("Opływanie"),
            &[
                ("wrap:1", tr("Opływanie z obu stron"), "wrap-text", wrap == 1),
                ("wrap:4", tr("Opływanie po większej stronie"), "move-horizontal", wrap == 4),
                ("wrap:5", tr("Opływanie: nad i pod"), "between-horizontal-start", wrap == 5),
                ("wrap:0", tr("Przed tekstem – bez opływania"), "x", wrap == 0),
                ("wrap:6", tr("Za tekstem"), "layers", wrap == 6),
            ],
        )
        .row(
            tr("Kolejność"),
            &[
                ("z-front", tr("Na wierzch"), "bring-to-front", false),
                ("z-forward", tr("O warstwę wyżej"), "arrow-up-from-line", false),
                ("z-backward", tr("O warstwę niżej"), "arrow-down-to-line", false),
                ("z-back", tr("Na spód"), "send-to-back", false),
            ],
        )
        .row(
            tr("Położenie"),
            &[
                ("anchor-toggle", tr("Przesuwaj z tekstem (zakotwiczenie w akapicie)"), "pin", anchored),
                ("lock", if locked { tr("Odblokuj położenie") } else { tr("Zablokuj położenie") }, "lock", locked),
            ],
        )
        .sep()
        .add("panel:props", tr("Właściwości obiektu…"), "sliders-horizontal", "");
    } else {
        // Tekst: kursor w miejsce kliknięcia, chyba że klik w zaznaczenie.
        let only = if in_frame_text { on_obj.map(|id| StoryRef::Obj(st.ed.doc.chain_head(id))) } else { None };
        if let Some((s, p, off)) = st.ed.layout.hit(page, px, py, only) {
            let at = At::new(p, off);
            let (a, b) = st.ed.range();
            let inside = s == st.ed.story && at >= a && at <= b && st.ed.has_sel();
            if !inside {
                if in_frame_text {
                    st.ed.obj = on_obj;
                } else {
                    st.ed.obj = None;
                }
                st.ed.set_caret(s, at, false);
            }
        }
        let sel = st.ed.has_sel();
        let (cp, pp, _) = st.ed.caret_props();
        let story = st.ed.story;

        // Pisownia – podpowiedzi na samej górze.
        let h = st.ed.head;
        let t = st.ed.para_text(h.para);
        let (wa, wb) = opendtp_core::edit::word_at(&t, h.off);
        let word = t.get(wa..wb).unwrap_or("").to_string();
        if !sel && word.chars().any(|c| c.is_alphabetic()) && opendtp_core::spell::misspelled(&word, &cp.lang) {
            let sugg = opendtp_core::spell::suggest(&word, &cp.lang);
            if sugg.is_empty() {
                m.add("", tr("(brak podpowiedzi)"), "spell-check", "");
            }
            for s in sugg.into_iter().take(6) {
                m.0.push(crate::CtxV { kind: 0, id: format!("spell-fix:{}:{}:{}", wa, wb, s).into(), label: s.into(), ..Default::default() });
            }
            m.row(
                tr("Pisownia"),
                &[
                    (&format!("spell-add:{word}"), tr("Dodaj do słownika"), "book-a", false),
                    (&format!("spell-ignore:{word}"), tr("Ignoruj wszystkie wystąpienia"), "eye", false),
                    ("spell-next", tr("Następny błąd (F7)"), "spell-check", false),
                ],
            )
            .sep();
        }

        // Łącze pod kursorem.
        if let Some(url) = cp.link.clone().filter(|u| !u.is_empty() && !internal_link(u)) {
            m.add(&format!("open-url:{url}"), tr("Otwórz łącze"), "external-link", "")
                .add("link", tr("Edytuj łącze…"), "link", "Ctrl+K")
                .sep();
        }

        m.add_if(sel, "cut", tr("Wytnij"), "scissors", "Ctrl+X")
            .add_if(sel, "copy", tr("Kopiuj"), "copy", "Ctrl+C")
            .add("paste", tr("Wklej"), "clipboard-paste", "Ctrl+V")
            .add("paste-plain", tr("Wklej jako tekst"), "clipboard-paste", "Ctrl+Shift+V")
            .add("select-all", tr("Zaznacz wszystko"), "text-select", "Ctrl+A")
            .sep();

        m.row(
            tr("Znaki"),
            &[
                ("bold", tr("Pogrubienie (Ctrl+B)"), "bold", cp.bold),
                ("italic", tr("Kursywa (Ctrl+I)"), "italic", cp.italic),
                ("underline", tr("Podkreślenie (Ctrl+U)"), "underline", cp.underline != Underline::None),
                ("strike", tr("Przekreślenie"), "strikethrough", cp.strike),
                ("format-painter", tr("Malarz formatów – przenieś formatowanie na inny tekst"), "paintbrush", false),
                ("clear-format", tr("Wyczyść formatowanie (Ctrl+Spacja)"), "eraser", false),
            ],
        )
        .row(
            tr("Akapit"),
            &[
                ("align:0", tr("Do lewej (Ctrl+L)"), "align-left", matches!(pp.align, Align::Left)),
                ("align:1", tr("Wyśrodkuj (Ctrl+E)"), "align-center", matches!(pp.align, Align::Center)),
                ("align:3", tr("Wyjustuj (Ctrl+J)"), "align-justify", matches!(pp.align, Align::Justify)),
                ("list:bullet", tr("Lista punktowana"), "list", pp.list.is_some()),
                ("outdent", tr("Zmniejsz wcięcie (Shift+Tab)"), "list-indent-decrease", false),
                ("indent", tr("Zwiększ wcięcie (Tab)"), "list-indent-increase", false),
            ],
        )
        .add("font-dialog", tr("Czcionka…"), "a-large-small", "")
        .add("para-dialog", tr("Akapit…"), "sliders-vertical", "")
        .add("panel:styles", tr("Style…"), "letter-text", "");
        if sel {
            m.add("style-new", tr("Nowy styl z zaznaczenia"), "plus", "");
        }
        m.sep();

        let body = story == StoryRef::Body;
        m.row(
            tr("Wstaw"),
            &[
                ("link", tr("Hiperłącze (Ctrl+K)"), "link", false),
                (if body { "footnote" } else { "" }, tr("Przypis dolny"), "superscript", false),
                (if body { "page-break" } else { "" }, tr("Podział strony (Ctrl+Enter)"), "separator-horizontal", false),
                (if body { "column-break" } else { "" }, tr("Podział kolumny (Ctrl+Shift+Enter)"), "columns-2", false),
                ("field:page-number", tr("Numer strony"), "hash", false),
                ("field:date", tr("Data"), "calendar", false),
            ],
        );
        if sel {
            m.add("find", tr("Znajdź zaznaczony tekst"), "search", "Ctrl+F");
        }
        if in_frame_text {
            m.sep().add("panel:props", tr("Właściwości ramki…"), "sliders-horizontal", "");
        }
    }
    refresh(st);
    let Some(ui) = st.ui.upgrade() else { return };
    let app = ui.global::<App>();
    while m.0.last().is_some_and(|i| i.kind == 1) {
        m.0.pop();
    }
    app.set_ctx_items(slint::ModelRc::new(slint::VecModel::from(m.0)));
    app.set_ctx_open(true);
}

fn ui_flag(st: &State, f: impl Fn(&App) -> bool) -> bool {
    st.ui.upgrade().is_some_and(|ui| f(&ui.global::<App>()))
}

// ---------------------------------------------------------------- klawiatura

fn is(text: &str, k: Key) -> bool {
    let c: char = k.into();
    text.chars().next() == Some(c) && text.chars().count() == 1
}

fn printable(text: &str) -> bool {
    text.chars().all(|c| !c.is_control() && !('\u{F700}'..='\u{F8FF}').contains(&c)) && !text.is_empty()
}

pub fn key(st: &mut State, text: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
    let Some(ui) = app(st).and_then(|w| w.upgrade()) else { return false };
    let a = ui.global::<App>();
    // Okna i nakładki.
    if is(text, Key::Escape) {
        if st.ed.cell.is_some() {
            st.ed.cell = None;
            actions::show_cell(st);
        } else if a.get_ctx_open() {
            a.set_ctx_open(false);
        } else if a.get_palette_open() {
            a.set_palette_open(false);
        } else if a.get_dialog() != "" {
            a.set_dialog("".into());
        } else if a.get_backstage() {
            a.set_backstage(false);
        } else if a.get_fullscreen() {
            actions::run(st, "fullscreen", "");
        } else if !st.pen.is_empty() {
            actions::finish_pen(st, false);
        } else if a.get_tool() != 0 {
            set_tool(st, 0);
            st.ed.painter = None;
        } else if st.ed.node_mode {
            st.ed.node_mode = false;
        } else if matches!(st.ed.story, StoryRef::Obj(_)) && st.ed.obj.is_some() {
            // Z tekstu ramki do zaznaczenia ramki.
            st.ed.story = StoryRef::Body;
            st.ed.fix_selection();
        } else if st.ed.obj.is_some() {
            st.ed.obj = None;
            st.ed.multi.clear();
        } else if matches!(st.ed.story, StoryRef::Note(_)) {
            crate::actions::run(st, "note-back", "");
        } else if matches!(st.ed.story, StoryRef::Header | StoryRef::Footer | StoryRef::HeaderFirst | StoryRef::FooterFirst | StoryRef::HeaderEven | StoryRef::FooterEven) {
            st.ed.set_caret(StoryRef::Body, At::new(0, 0), false);
        } else if a.get_right_panel() != "" {
            a.set_right_panel("".into());
        } else if a.get_left_panel() != "" {
            a.set_left_panel("".into());
        }
        refresh(st);
        return true;
    }
    if a.get_backstage() || a.get_dialog() != "" || a.get_palette_open() {
        return false;
    }
    // Skróty globalne.
    let lower = text.to_lowercase();
    if ctrl && !alt {
        let cmd: Option<(&str, &str)> = match lower.as_str() {
            "z" if shift => Some(("redo", "")),
            "z" => Some(("undo", "")),
            "y" => Some(("redo", "")),
            "s" if shift => Some(("save-as", "")),
            "s" => Some(("save", "")),
            "o" => Some(("open", "")),
            "n" if shift => Some(("style", "normal")),
            "n" => Some(("new", "blank")),
            "p" => Some(("print", "")),
            "f" => Some(("find", "")),
            "h" => Some(("replace", "")),
            "k" if shift => Some(("link", "")),
            "k" => Some(("palette", "")),
            "x" => Some(("cut", "")),
            "c" if shift => Some(("copy-format", "")),
            "c" => Some(("copy", "")),
            "v" if shift => Some(("paste-plain", "")),
            "v" => Some(("paste", "")),
            "a" => Some(("select-all", "")),
            "b" => Some(("bold", "")),
            "i" => Some(("italic", "")),
            "u" => Some(("underline", "")),
            "e" => Some(("align", "1")),
            "l" => Some(("align", "0")),
            "r" => Some(("align", "2")),
            "j" => Some(("align", "3")),
            "d" => Some(("duplicate", "")),
            "]" => Some(("grow", "")),
            "[" => Some(("shrink", "")),
            "=" if shift => Some(("sup", "")),
            "+" => Some(("sup", "")),
            "=" => Some(("sub", "")),
            "*" | "8" if shift || lower == "*" => Some(("marks", "")),
            "-" => Some(("zoom-out", "")),
            _ => None,
        };
        if let Some((id, arg)) = cmd {
            actions::run(st, id, arg);
            return true;
        }
        if is(text, Key::F1) {
            actions::run(st, "ribbon-collapse", "");
            return true;
        }
        if lower == "0" {
            actions::run(st, "zoom", "100");
            return true;
        }
    }
    if ctrl && alt && !shift {
        match lower.as_str() {
            "1" => {
                actions::run(st, "style", "heading1");
                return true;
            }
            "2" => {
                actions::run(st, "style", "heading2");
                return true;
            }
            "3" => {
                actions::run(st, "style", "heading3");
                return true;
            }
            _ => {}
        }
    }
    if is(text, Key::F7) {
        actions::run(st, "spell-next", "");
        return true;
    }
    if is(text, Key::F11) {
        actions::run(st, "fullscreen", "");
        return true;
    }
    // Pióro: Enter zamyka kształt.
    if !st.pen.is_empty() && is(text, Key::Return) {
        actions::finish_pen(st, true);
        return true;
    }
    // Zaznaczony obiekt (bez edycji jego tekstu).
    let editing_frame = matches!(st.ed.story, StoryRef::Obj(h) if st.ed.obj.is_some_and(|o| st.ed.doc.chain_head(o) == h));
    if st.ed.obj.is_some() && !editing_frame {
        if is(text, Key::Delete) || is(text, Key::Backspace) {
            if st.ed.node_mode {
                actions::run(st, "node-delete", "");
            } else {
                actions::run(st, "delete-obj", "");
            }
            return true;
        }
        let step = if shift { 10.0 } else if ctrl { 0.25 } else { 1.0 };
        let mv = if is(text, Key::LeftArrow) {
            Some((-step, 0.0))
        } else if is(text, Key::RightArrow) {
            Some((step, 0.0))
        } else if is(text, Key::UpArrow) {
            Some((0.0, -step))
        } else if is(text, Key::DownArrow) {
            Some((0.0, step))
        } else {
            None
        };
        if let Some((dx, dy)) = mv {
            if st.ed.node_mode {
                if let (Some((ci, ni)), Some(o)) = (st.ed.sel_node, st.ed.object()) {
                    let (kx, ky) = (dx / o.w.max(1.0), dy / o.h.max(1.0));
                    st.ed.shape_edit(&move |s, sel| {
                        if let Some(n) = s.contours.get_mut(ci).and_then(|c| c.nodes.get_mut(ni)) {
                            n.x += kx;
                            n.y += ky;
                            n.cin = n.cin.map(|(a, b)| (a + kx, b + ky));
                            n.cout = n.cout.map(|(a, b)| (a + kx, b + ky));
                        }
                        sel
                    });
                }
            } else {
                st.ed.edit_obj(&move |o| {
                    if !o.locked {
                        o.x += dx;
                        o.y += dy;
                    }
                });
            }
            refresh(st);
            return true;
        }
        if is(text, Key::Return) || is(text, Key::F2) {
            let o = st.ed.object().unwrap().clone();
            if matches!(o.kind, ObjKind::Text { .. }) {
                let head = st.ed.doc.chain_head(o.id);
                st.ed.set_caret(StoryRef::Obj(head), At::new(0, 0), false);
                st.ed.obj = Some(o.id);
                refresh(st);
            }
            return true;
        }
        if is(text, Key::Tab) {
            let objs: Vec<u64> = st.ed.doc.objects.iter().map(|o| o.id).collect();
            if let Some(i) = objs.iter().position(|o| Some(*o) == st.ed.obj) {
                let n = objs.len();
                let j = if shift { (i + n - 1) % n } else { (i + 1) % n };
                st.ed.obj = Some(objs[j]);
                refresh(st);
            }
            return true;
        }
        if printable(text) && !ctrl {
            // Pisanie przy zaznaczonej ramce tekstowej – zaczyna edycję jej tekstu.
            let o = st.ed.object().unwrap().clone();
            if matches!(o.kind, ObjKind::Text { .. }) {
                let head = st.ed.doc.chain_head(o.id);
                let s = st.ed.doc.story(StoryRef::Obj(head)).cloned().unwrap_or_default();
                let n = s.paras.len();
                st.ed.set_caret(StoryRef::Obj(head), At::new(n - 1, s.paras[n - 1].len()), false);
                st.ed.obj = Some(o.id);
            } else {
                return true;
            }
        } else {
            return false;
        }
    }
    // Tekst.
    let mv = |m: Move| -> Option<Move> { Some(m) };
    let m = if is(text, Key::LeftArrow) {
        mv(if ctrl { Move::WordLeft } else { Move::Left })
    } else if is(text, Key::RightArrow) {
        mv(if ctrl { Move::WordRight } else { Move::Right })
    } else if is(text, Key::UpArrow) {
        mv(if ctrl { Move::ParaUp } else { Move::Up })
    } else if is(text, Key::DownArrow) {
        mv(if ctrl { Move::ParaDown } else { Move::Down })
    } else if is(text, Key::Home) {
        mv(if ctrl { Move::DocStart } else { Move::LineStart })
    } else if is(text, Key::End) {
        mv(if ctrl { Move::DocEnd } else { Move::LineEnd })
    } else if is(text, Key::PageUp) {
        mv(Move::PageUp)
    } else if is(text, Key::PageDown) {
        mv(Move::PageDown)
    } else {
        None
    };
    if let Some(m) = m {
        st.ed.move_caret(m, shift);
        refresh_scroll(st);
        return true;
    }
    if is(text, Key::Backspace) {
        st.ed.backspace(ctrl);
        refresh_scroll(st);
        return true;
    }
    if is(text, Key::Delete) {
        st.ed.delete_forward(ctrl);
        refresh_scroll(st);
        return true;
    }
    if is(text, Key::Return) {
        if ctrl {
            st.ed.page_break(shift);
        } else if shift {
            st.ed.insert("\u{2028}");
        } else {
            st.ed.enter();
        }
        refresh_scroll(st);
        return true;
    }
    if is(text, Key::Tab) {
        let (_, pp, _) = st.ed.caret_props();
        let at_start = st.ed.head.off == 0 || st.ed.has_sel();
        if pp.list.is_some() && at_start {
            st.ed.indent(if shift { -1 } else { 1 });
        } else if shift {
            st.ed.indent(-1);
        } else {
            st.ed.insert("\t");
        }
        refresh_scroll(st);
        return true;
    }
    if ctrl && shift && text == " " {
        st.ed.insert("\u{00A0}");
        refresh_scroll(st);
        return true;
    }
    if ctrl && !alt && text == " " {
        actions::run(st, "clear-format", "");
        return true;
    }
    if printable(text) && (!ctrl || alt) {
        type_text(st, text);
        refresh_scroll(st);
        return true;
    }
    false
}

/// Pisanie z autokorektą (cudzysłowy, myślniki, wielokropek, symbole, wielka litera, listy).
fn type_text(st: &mut State, text: &str) {
    let p = st.prefs.clone();
    let before: String = {
        let t = st.ed.para_text(st.ed.head.para);
        t[..st.ed.head.off.min(t.len())].to_string()
    };
    let prev = before.chars().last();
    let mut t = text.to_string();
    if p.smart_quotes && !st.ed.has_sel() {
        let opening = prev.is_none_or(|c| c.is_whitespace() || "([{„«—–-/".contains(c));
        if t == "\"" {
            t = if opening { "„".into() } else { "”".into() };
        } else if t == "'" {
            t = if opening { "‚".into() } else { "’".into() };
        }
    }
    // Wielka litera na początku zdania (przy pierwszej literze wyrazu).
    if p.capitalize && t.chars().count() == 1 && t.chars().all(|c| c.is_lowercase()) {
        let trimmed = before.trim_end();
        let starts = trimmed.is_empty() && before.is_empty() || (before.ends_with(' ') && (trimmed.ends_with('.') || trimmed.ends_with('!') || trimmed.ends_with('?')) && !trimmed.ends_with("..") && !abbrev(trimmed));
        let in_frame_or_body = true;
        if starts && in_frame_or_body && !st.ed.has_sel() {
            t = t.to_uppercase();
        }
    }
    st.ed.insert(&t);
    if p.autocorrect {
        let now: String = {
            let tt = st.ed.para_text(st.ed.head.para);
            tt[..st.ed.head.off.min(tt.len())].to_string()
        };
        let rules: [(&str, &str); 9] = [("...", "…"), ("(c)", "©"), ("(C)", "©"), ("(r)", "®"), ("(R)", "®"), ("(tm)", "™"), ("->", "→"), ("<-", "←"), ("=>", "⇒")];
        for (from, to) in rules {
            if now.ends_with(from) {
                st.ed.replace_before(from.len(), to);
                return;
            }
        }
        // „ -- ” i „ - ” między wyrazami → półpauza.
        if text == " " {
            for from in [" -- ", " - "] {
                if now.ends_with(from) && now.len() > from.len() {
                    st.ed.replace_before(from.len(), " – ");
                    return;
                }
            }
        }
    }
    if p.autolist && text == " " {
        let now = st.ed.para_text(st.ed.head.para);
        let h = st.ed.head.off;
        if h == now.len() || h <= 4 {
            let pre = &now[..h.min(now.len())];
            let kind = match pre {
                "- " | "* " | "• " => Some(ListKind::Bullet),
                "– " => Some(ListKind::Dash),
                "1. " => Some(ListKind::Number),
                "1) " => Some(ListKind::NumberParen),
                "a) " => Some(ListKind::LowerLetter),
                "> " => Some(ListKind::Arrow),
                _ => None,
            };
            if let Some(k) = kind {
                let (_, pp, _) = st.ed.caret_props();
                if pp.list.is_none() {
                    st.ed.replace_before(pre.len(), "");
                    st.ed.set_list(Some(k));
                }
            }
        }
    }
}

fn abbrev(s: &str) -> bool {
    let last = s.rsplit(' ').next().unwrap_or("");
    let l = last.trim_end_matches('.').to_lowercase();
    ["np", "itd", "itp", "tj", "tzn", "m.in", "ok", "ul", "nr", "str", "godz", "dr", "prof", "mgr", "inż", "św", "r", "w", "tys", "mln", "zł", "e.g", "i.e", "etc", "mr", "mrs", "vs"].contains(&l.as_str()) || l.len() == 1
}
