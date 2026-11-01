//! Widok płótna: położenie stron w pikselach logicznych, renderowanie widocznych stron do obrazów
//! Slint (tylko gdy treść albo powiększenie się zmieniły) i nakładki: kursor, zaznaczenie, ramka
//! obiektu, węzły kształtu.

use slint::ComponentHandle;
use std::rc::Rc;

use opendtp_core::geom::{self, Rect};
use opendtp_core::model::*;
use opendtp_core::paint::{self, PathCmd};
use opendtp_core::raster::Raster;
use slint::{Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};

use crate::editor::Editor;
use crate::{AppWindow, BoxV, NodeV, PageV};

pub const GAP: f32 = 28.0;

pub struct View {
    /// Powiększenie (1.0 = 100%).
    pub zoom: f32,
    pub dpr: f32,
    /// Trwa animacja powiększenia – strony z obrazem tylko skalujemy, bez przerysowania.
    pub animating: bool,
    /// Fizyczne piksele na cal ekranu (0 – nieznane, umowne 96 na skalę Windows).
    pub screen_ppi: f32,
    pub vx: f32,
    pub vy: f32,
    pub vw: f32,
    pub vh: f32,
    pub pages: Vec<(f32, f32, f32, f32)>,
    rendered: Vec<Option<(u64, u32)>>,
    raster: Raster,
    model: Rc<VecModel<PageV>>,
    pub marks: bool,
    pub guides: bool,
    pub spell: bool,
    pub spell_ready: bool,
    /// Obiekt ukryty podczas przeciągania (rysowany tylko jako podgląd) – brak.
    pub content_w: f32,
    pub content_h: f32,
}

impl View {
    pub fn new() -> View {
        View {
            zoom: 1.0,
            dpr: 1.0,
            animating: false,
            screen_ppi: 0.0,
            vx: 0.0,
            vy: 0.0,
            vw: 1200.0,
            vh: 800.0,
            pages: Vec::new(),
            rendered: Vec::new(),
            raster: Raster::new(),
            model: Rc::new(VecModel::default()),
            marks: false,
            guides: true,
            spell: true,
            spell_ready: false,
            content_w: 0.0,
            content_h: 0.0,
        }
    }

    pub fn model(&self) -> ModelRc<PageV> {
        ModelRc::from(self.model.clone())
    }

    /// Piksele logiczne na cal przy 100%: z fizycznej gęstości ekranu, żeby 100% oznaczało
    /// rzeczywisty rozmiar kartki; bez niej – umowne 96 jak w Windows.
    pub fn lppi(&self) -> f32 {
        if self.screen_ppi > 0.0 && self.dpr > 0.0 {
            self.screen_ppi / self.dpr
        } else {
            96.0
        }
    }

    /// Piksele logiczne na punkt.
    pub fn k(&self) -> f32 {
        self.zoom * self.lppi() / 72.0
    }

    /// Przelicza położenia stron; zwraca, czy coś się zmieniło.
    pub fn place(&mut self, ed: &Editor) -> bool {
        let k = self.k();
        let n = ed.layout.pages.len().max(1);
        let sizes: Vec<(f32, f32)> = (0..n).map(|i| ed.layout.page_size(&ed.doc, i)).collect();
        let maxw = sizes.iter().map(|s| s.0).fold(0.0, f32::max) * k;
        let cw = (maxw + 2.0 * GAP).max(self.vw);
        let mut y = GAP;
        let pages: Vec<(f32, f32, f32, f32)> = sizes
            .iter()
            .map(|&(w, h)| {
                let (pw, ph) = (w * k, h * k);
                let r = (((cw - pw) / 2.0).max(GAP), y, pw, ph);
                y += ph + GAP;
                r
            })
            .collect();
        let ch = y;
        let changed = pages != self.pages;
        self.pages = pages;
        self.content_w = cw;
        self.content_h = ch;
        if changed {
            self.rendered = vec![None; n];
            while self.model.row_count() > n {
                self.model.remove(self.model.row_count() - 1);
            }
            for (i, &(x, y, w, h)) in self.pages.iter().enumerate() {
                if i < self.model.row_count() {
                    let mut r = self.model.row_data(i).unwrap();
                    r.x = x;
                    r.y = y;
                    r.w = w;
                    r.h = h;
                    r.n = i as i32;
                    self.model.set_row_data(i, r);
                } else {
                    self.model.push(PageV { x, y, w, h, img: Image::default(), n: i as i32 });
                }
            }
        }
        changed
    }

    pub fn invalidate(&mut self) {
        for r in self.rendered.iter_mut() {
            *r = None;
        }
    }

    pub fn forget_image(&mut self, id: &str) {
        self.raster.forget_image(id);
    }

    /// Renderuje widoczne strony, których obraz jest nieaktualny.
    pub fn render(&mut self, ed: &Editor) {
        let zkey = (self.zoom * 1000.0) as u32 ^ ((self.dpr * 100.0) as u32) << 20;
        let top = self.vy - self.vh * 0.5;
        let bottom = self.vy + self.vh * 1.5;
        let spell_on = self.spell && self.spell_ready;
        let check = |w: &str, l: &str| opendtp_core::spell::misspelled(w, l);
        let opts = paint::PaintOpts { screen: true, marks: self.marks, guides: self.guides, misspelled: if spell_on { Some(&check) } else { None }, hide_obj: None };
        for i in 0..self.pages.len() {
            let (_, y, _, h) = self.pages[i];
            let visible = y + h >= top && y <= bottom;
            if !visible {
                // Daleko od widoku – zwalniamy pamięć obrazu.
                if self.rendered[i].is_some() && (y + h < top - self.vh * 2.0 || y > bottom + self.vh * 2.0) {
                    self.rendered[i] = None;
                    if let Some(mut r) = self.model.row_data(i) {
                        r.img = Image::default();
                        self.model.set_row_data(i, r);
                    }
                }
                continue;
            }
            if self.rendered[i] == Some((ed.version, zkey)) {
                continue;
            }
            if self.animating && self.model.row_data(i).is_some_and(|r| r.img.size().width > 0) {
                continue;
            }
            let scale = self.k() * self.dpr;
            let (pw0, ph0) = ed.layout.page_size(&ed.doc, i);
            let w = (pw0 * scale).round().max(1.0) as u32;
            let hh = (ph0 * scale).round().max(1.0) as u32;
            let Some(mut pm) = tiny_skia::Pixmap::new(w.min(12000), hh.min(16000)) else { continue };
            let ops = paint::page_ops(&ed.doc, &ed.layout, i, &opts);
            self.raster.draw(&ed.doc, &ops, &mut pm, scale);
            let (pw, ph) = (pm.width(), pm.height());
            let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(pm.data(), pw, ph);
            let img = Image::from_rgba8_premultiplied(buf);
            if let Some(mut r) = self.model.row_data(i) {
                r.img = img;
                self.model.set_row_data(i, r);
            }
            self.rendered[i] = Some((ed.version, zkey));
        }
    }

    /// Strona i punkt (pt) pod położeniem na płótnie.
    pub fn to_page(&self, x: f32, y: f32) -> (usize, f32, f32) {
        let k = self.k();
        let mut best = 0;
        for (i, &(_, py, _, ph)) in self.pages.iter().enumerate() {
            if y >= py - GAP / 2.0 {
                best = i;
            }
            if y < py + ph + GAP / 2.0 {
                break;
            }
        }
        let (px, py, _, _) = self.pages.get(best).copied().unwrap_or((0.0, 0.0, 0.0, 0.0));
        (best, (x - px) / k, (y - py) / k)
    }

    pub fn to_canvas(&self, page: usize, x: f32, y: f32) -> (f32, f32) {
        let k = self.k();
        let (px, py, _, _) = self.pages.get(page).copied().unwrap_or((0.0, 0.0, 0.0, 0.0));
        (px + x * k, py + y * k)
    }

    fn bx(&self, page: usize, r: Rect) -> BoxV {
        let (x, y) = self.to_canvas(page, r.x, r.y);
        let k = self.k();
        BoxV { x, y, w: r.w * k, h: r.h * k }
    }

    /// Nakładki: kursor, zaznaczenie, obiekt, węzły.
    pub fn overlays(&self, ed: &Editor, ui: &AppWindow) {
        let app = ui.global::<crate::App>();
        let k = self.k();
        // Kursor i zaznaczenie tekstu.
        let show_caret = ed.obj.is_none() || matches!(ed.story, StoryRef::Obj(h) if ed.obj.is_some_and(|o| ed.doc.chain_head(o) == h));
        if let (true, Some((page, x, top, h))) = (show_caret && !ed.has_sel(), ed.caret_page()) {
            let (cx, cy) = self.to_canvas(page, x, top);
            app.set_caret(BoxV { x: cx, y: cy, w: (1.2 * k).max(1.5), h: h * k });
            app.set_caret_on(true);
        } else {
            app.set_caret_on(false);
        }
        let rects: Vec<BoxV> = if ed.has_sel() {
            let (a, b) = ed.range();
            ed.layout.selection(ed.story, (a.para, a.off), (b.para, b.off)).into_iter().map(|(p, r)| self.bx(p, r)).collect()
        } else {
            Vec::new()
        };
        app.set_sel_rects(ModelRc::new(VecModel::from(rects)));
        // Wyniki wyszukiwania.
        let mut finds = Vec::new();
        if ed.story == StoryRef::Body {
            for &(p, a, b) in ed.find.iter().take(500) {
                for (pg, r) in ed.layout.selection(ed.story, (p, a), (p, b)) {
                    finds.push(self.bx(pg, r));
                }
            }
        }
        app.set_find_rects(ModelRc::new(VecModel::from(finds)));
        // Obiekt.
        match ed.object() {
            Some(o) => {
                let r = Rect::new(o.x, o.y, o.w, o.h);
                app.set_obj_sel(true);
                app.set_obj_box(self.bx(o.page, r));
                app.set_obj_locked(o.locked);
                app.set_obj_kind(o.kind_name().into());
                let p = paint::shape_path(&o.shape, r, o.corner);
                app.set_obj_path(self.svg(o.page, &p).into());
                if ed.node_mode {
                    let mut nodes = Vec::new();
                    let mut lines = String::new();
                    for (ci, c) in o.shape.contours.iter().enumerate() {
                        for (ni, n) in c.nodes.iter().enumerate() {
                            let map = |x: f32, y: f32| self.to_canvas(o.page, o.x + x * o.w, o.y + y * o.h);
                            let (nx, ny) = map(n.x, n.y);
                            let sel = ed.sel_node == Some((ci, ni));
                            for (part, h) in [(1, n.cin), (2, n.cout)] {
                                if let Some((hx, hy)) = h {
                                    let (ax, ay) = map(hx, hy);
                                    lines.push_str(&format!("M {nx:.1} {ny:.1} L {ax:.1} {ay:.1} "));
                                    nodes.push(NodeV { x: ax, y: ay, kind: 2, sel: false, id: format!("{ci}:{ni}:{part}").into() });
                                }
                            }
                            nodes.push(NodeV { x: nx, y: ny, kind: if n.smooth || n.cin.is_some() || n.cout.is_some() { 1 } else { 0 }, sel, id: format!("{ci}:{ni}:0").into() });
                        }
                    }
                    app.set_nodes(ModelRc::new(VecModel::from(nodes)));
                    app.set_node_lines(lines.into());
                } else {
                    app.set_nodes(ModelRc::new(VecModel::from(Vec::<NodeV>::new())));
                    app.set_node_lines("".into());
                }
                app.set_node_mode(ed.node_mode);
            }
            None => {
                app.set_obj_sel(false);
                app.set_obj_path("".into());
                app.set_node_mode(false);
                app.set_nodes(ModelRc::new(VecModel::from(Vec::<NodeV>::new())));
            }
        }
        let multi: Vec<BoxV> = ed.multi.iter().filter_map(|m| ed.doc.object(*m)).map(|o| self.bx(o.page, Rect::new(o.x, o.y, o.w, o.h))).collect();
        app.set_multi_boxes(ModelRc::new(VecModel::from(multi)));
        app.set_multi_count(if ed.obj.is_some() { 1 + ed.multi.iter().filter(|m| Some(**m) != ed.obj).count() as i32 } else { 0 });
    }

    /// Ścieżka w punktach strony jako polecenia SVG we współrzędnych płótna.
    pub fn svg(&self, page: usize, p: &paint::Path) -> String {
        let mut s = String::new();
        for c in &p.0 {
            match *c {
                PathCmd::M(x, y) => {
                    let (a, b) = self.to_canvas(page, x, y);
                    s.push_str(&format!("M {a:.1} {b:.1} "));
                }
                PathCmd::L(x, y) => {
                    let (a, b) = self.to_canvas(page, x, y);
                    s.push_str(&format!("L {a:.1} {b:.1} "));
                }
                PathCmd::C(x1, y1, x2, y2, x, y) => {
                    let (a, b) = self.to_canvas(page, x1, y1);
                    let (c2, d) = self.to_canvas(page, x2, y2);
                    let (e, f) = self.to_canvas(page, x, y);
                    s.push_str(&format!("C {a:.1} {b:.1} {c2:.1} {d:.1} {e:.1} {f:.1} "));
                }
                PathCmd::Z => s.push_str("Z "),
            }
        }
        s
    }

    pub fn shape_svg(&self, page: usize, s: &Shape, r: Rect) -> String {
        self.svg(page, &paint::shape_path(s, r, 0.0))
    }

    /// Przewija tak, by kursor był widoczny. Zwraca nowe przesunięcie, jeśli trzeba.
    pub fn scroll_to_caret(&self, ed: &Editor) -> Option<(f32, f32)> {
        let (page, x, top, h) = ed.caret_page()?;
        let (cx, cy) = self.to_canvas(page, x, top);
        let ch = h * self.k();
        let mut vy = self.vy;
        let mut vx = self.vx;
        if cy < self.vy + 20.0 {
            vy = (cy - 60.0).max(0.0);
        } else if cy + ch > self.vy + self.vh - 20.0 {
            vy = cy + ch - self.vh + 60.0;
        }
        if cx < self.vx + 10.0 {
            vx = (cx - 40.0).max(0.0);
        } else if cx > self.vx + self.vw - 10.0 {
            vx = cx - self.vw + 40.0;
        }
        if (vy - self.vy).abs() > 0.5 || (vx - self.vx).abs() > 0.5 {
            Some((vx, vy))
        } else {
            None
        }
    }

    pub fn fit_width(&self, ed: &Editor) -> f32 {
        ((self.vw - 2.0 * GAP) / (ed.doc.page.width * self.lppi() / 72.0)).clamp(0.25, 4.0)
    }

    pub fn fit_page(&self, ed: &Editor) -> f32 {
        let a = (self.vw - 2.0 * GAP) / (ed.doc.page.width * self.lppi() / 72.0);
        let b = (self.vh - 2.0 * GAP) / (ed.doc.page.height * self.lppi() / 72.0);
        a.min(b).clamp(0.25, 4.0)
    }
}

/// Kształt obiektu w punktach strony jako wielokąty (trafienia myszą).
pub fn hit_object(o: &Object, x: f32, y: f32) -> bool {
    if !o.contains(x, y) {
        return false;
    }
    let r = Rect::new(o.x, o.y, o.w, o.h);
    let polys = geom::shape_polys(&o.shape, r);
    if polys.is_empty() {
        // Linia / ścieżka otwarta – blisko odcinka.
        return geom::nearest_segment(&o.shape, r, x, y).is_some_and(|s| s.3 < 6.0);
    }
    geom::point_in(&polys, x, y) || matches!(o.kind, ObjKind::Text { .. } | ObjKind::Image { .. } | ObjKind::Chart { .. }) && o.contains(x, y) && o.shape.is_rect()
}
