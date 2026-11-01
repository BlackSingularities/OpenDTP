//! Silnik składu. Każdy tekst płynie przez ciąg regionów: kolumny kolejnych stron (treść główna),
//! kolumny ramek łańcucha (kształty tekstu) albo pas nagłówka/stopki. Region podaje, które
//! przedziały poziome pasa wiersza są wolne: wnętrze kształtu (dowolny kontur z węzłów), minus
//! obiekty, które tekst ma opływać (kontur albo ramka, z odstępem i trybem opływania). Wiersz może
//! więc składać się z kilku odcinków – tekst przechodzi z jednego do drugiego, jak w składzie.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::geom::{self, Pt};
use crate::model::*;
use crate::text::{self, FieldExtra, FieldValues, GKind, SGlyph, ShapedPara};

#[derive(Clone, Copy, Debug)]
pub struct PGlyph {
    pub g: SGlyph,
    /// Położenie pióra (lewa krawędź glifu bez przesunięcia `dx`).
    pub x: f32,
    /// Linia bazowa glifu (z indeksem i przesunięciem).
    pub y: f32,
    pub adv: f32,
    /// Tabulator z kropkami wiodącymi.
    pub leader: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct LSeg {
    pub x0: f32,
    pub x1: f32,
    pub g0: usize,
    pub g1: usize,
}

#[derive(Clone)]
pub struct Line {
    pub story: StoryRef,
    pub para: usize,
    pub page: usize,
    pub top: f32,
    pub height: f32,
    pub baseline: f32,
    pub start: usize,
    pub end: usize,
    pub glyphs: Vec<PGlyph>,
    pub segs: Vec<LSeg>,
    pub first: bool,
    pub last: bool,
    /// Znacznik listy albo inicjał.
    pub marker: Vec<PGlyph>,
    pub marker_style: Option<CharProps>,
    /// Lewa i prawa krawędź obszaru tekstu (tło i ramka akapitu).
    pub area: (f32, f32),
    pub shaped: Arc<ShapedPara>,
    /// Ramka, w której leży wiersz.
    pub obj: Option<u64>,
    /// Kursor w pustym wierszu.
    pub empty_x: f32,
}

impl Line {
    pub fn bottom(&self) -> f32 {
        self.top + self.height
    }

    /// Położenie poziome kursora przed znakiem `off`.
    pub fn x_at(&self, off: usize) -> f32 {
        let mut last: Option<&PGlyph> = None;
        for g in &self.glyphs {
            if g.g.kind == GKind::Newline {
                if g.g.cluster >= off {
                    return g.x;
                }
                continue;
            }
            if g.g.cluster >= off {
                return g.x;
            }
            last = Some(g);
        }
        match last {
            Some(g) => g.x + g.adv,
            None => self.glyphs.first().map(|g| g.x).unwrap_or(self.empty_x),
        }
    }

    /// Prawa krawędź treści wiersza.
    pub fn content_right(&self) -> f32 {
        self.glyphs.iter().rev().find(|g| g.g.kind != GKind::Newline).map(|g| g.x + g.adv).unwrap_or(self.empty_x)
    }
}

pub struct PathGlyph {
    pub g: SGlyph,
    pub x: f32,
    pub y: f32,
    pub angle: f32,
}

#[derive(Default)]
pub struct PageInfo {
    pub lines: Vec<usize>,
    /// Układ strony (sekcje mogą zmieniać kolumny i orientację).
    pub setup: PageSetup,
    /// Kreska nad przypisami: (y, x lewej krawędzi, szerokość).
    pub note_rule: Option<(f32, f32, f32)>,
}

#[derive(Default)]
pub struct Layout {
    pub pages: Vec<PageInfo>,
    pub lines: Vec<Line>,
    pub stories: HashMap<StoryRef, Vec<usize>>,
    /// Ramki, w których tekst się nie zmieścił (ostatnia ramka łańcucha).
    pub overflow: HashSet<u64>,
    pub path_glyphs: HashMap<u64, Vec<PathGlyph>>,
    /// Strona każdego akapitu treści głównej (spis treści, nawigator).
    pub body_pages: Vec<usize>,
}

impl Layout {
    /// Rozmiar strony w punktach.
    pub fn page_size(&self, doc: &Document, page: usize) -> (f32, f32) {
        match self.pages.get(page) {
            Some(p) => (p.setup.width, p.setup.height),
            None => (doc.page.width, doc.page.height),
        }
    }

    pub fn setup<'a>(&'a self, doc: &'a Document, page: usize) -> &'a PageSetup {
        self.pages.get(page).map(|p| &p.setup).unwrap_or(&doc.page)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Caret {
    pub page: usize,
    pub x: f32,
    pub top: f32,
    pub height: f32,
    pub line: usize,
}

impl Layout {
    pub fn story_lines(&self, s: StoryRef) -> &[usize] {
        self.stories.get(&s).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Wiersz, w którym leży miejsce `(para, off)` – indeks w `lines`.
    pub fn line_for(&self, s: StoryRef, para: usize, off: usize) -> Option<usize> {
        let ls = self.story_lines(s);
        let lo = ls.partition_point(|&i| self.lines[i].para < para);
        let mut best = None;
        for &i in &ls[lo..] {
            let l = &self.lines[i];
            if l.para != para {
                break;
            }
            best = Some(i);
            if off >= l.start && off < l.end {
                return Some(i);
            }
        }
        best
    }

    pub fn caret(&self, s: StoryRef, para: usize, off: usize) -> Option<Caret> {
        let i = self.line_for(s, para, off)?;
        let l = &self.lines[i];
        Some(Caret { page: l.page, x: l.x_at(off), top: l.top, height: l.height, line: i })
    }

    /// Miejsce w tekście najbliższe punktowi na stronie. `only` – tylko w tym tekście.
    pub fn hit(&self, page: usize, x: f32, y: f32, only: Option<StoryRef>) -> Option<(StoryRef, usize, usize)> {
        let p = self.pages.get(page)?;
        let mut best: Option<(f32, f32, usize)> = None;
        for &i in &p.lines {
            let l = &self.lines[i];
            if only.is_some_and(|s| s != l.story) {
                continue;
            }
            let dy = if y < l.top { l.top - y } else if y > l.bottom() { y - l.bottom() } else { 0.0 };
            let left = l.segs.first().map(|s| s.x0).unwrap_or(l.empty_x);
            let right = l.segs.last().map(|s| s.x1).unwrap_or(l.empty_x);
            let dx = if x < left { left - x } else if x > right { x - right } else { 0.0 };
            let key = (dy, dx);
            if best.is_none_or(|b| key < (b.0, b.1)) {
                best = Some((dy, dx, i));
            }
        }
        let (_, _, i) = best?;
        Some((self.lines[i].story, self.lines[i].para, self.offset_in_line(i, x)))
    }

    /// Najbliższa granica znaków w wierszu dla położenia `x`.
    pub fn offset_in_line(&self, i: usize, x: f32) -> usize {
        let l = &self.lines[i];
        let mut cands: Vec<(f32, usize)> = Vec::new();
        for g in &l.glyphs {
            if g.g.kind == GKind::Newline {
                cands.push((g.x, g.g.cluster));
                continue;
            }
            if cands.last().map(|c| c.1) != Some(g.g.cluster) {
                cands.push((g.x, g.g.cluster));
            }
        }
        // Koniec wiersza: po ostatnim znaku; w złamanym wierszu – przed spacją, która go kończy.
        let end = if l.last {
            l.end
        } else {
            match l.glyphs.iter().rev().find(|g| g.g.kind != GKind::Newline) {
                Some(g) if g.g.kind == GKind::Space => g.g.cluster,
                Some(_) => l.end,
                None => l.start,
            }
        };
        if !l.glyphs.iter().any(|g| g.g.kind == GKind::Newline) {
            cands.push((l.content_right(), end.max(l.start)));
        }
        if cands.is_empty() {
            return l.start;
        }
        cands.iter().min_by(|a, b| (a.0 - x).abs().partial_cmp(&(b.0 - x).abs()).unwrap()).map(|c| c.1).unwrap_or(l.start)
    }

    /// Prostokąty zaznaczenia (strona, prostokąt) od `a` do `b` (para, off), a ≤ b.
    pub fn selection(&self, s: StoryRef, a: (usize, usize), b: (usize, usize)) -> Vec<(usize, geom::Rect)> {
        let mut out = Vec::new();
        for &i in self.story_lines(s) {
            let l = &self.lines[i];
            if l.para < a.0 || l.para > b.0 {
                continue;
            }
            let lo = if l.para == a.0 { a.1 } else { 0 };
            let hi = if l.para == b.0 { b.1 } else { usize::MAX };
            if hi <= l.start && !(l.glyphs.is_empty() && lo <= l.start && hi >= l.start && l.para != b.0) {
                if !(l.para < b.0 && l.last && lo <= l.end) {
                    continue;
                }
            }
            for sg in &l.segs {
                let mut x0 = f32::MAX;
                let mut x1 = f32::MIN;
                for g in &l.glyphs[sg.g0..sg.g1] {
                    if g.g.cluster >= lo && g.g.cluster < hi {
                        x0 = x0.min(g.x);
                        x1 = x1.max(g.x + g.adv.max(0.0));
                    }
                }
                if x1 > x0 {
                    out.push((l.page, geom::Rect::new(x0, l.top, x1 - x0, l.height)));
                }
            }
            // Zaznaczenie przechodzi przez koniec akapitu – mały znacznik.
            if l.last && l.para < b.0 && lo <= l.end {
                let x = l.content_right();
                out.push((l.page, geom::Rect::new(x, l.top, 4.0, l.height)));
            }
        }
        out
    }
}

// ---------------------------------------------------------------- regiony

#[derive(Clone)]
struct Excl {
    polys: Vec<Vec<Pt>>,
    dist: f32,
    mode: WrapMode,
}

#[derive(Clone)]
struct Region {
    page: usize,
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    polys: Option<Arc<Vec<Vec<Pt>>>>,
    pad: f32,
    excl: Arc<Vec<Excl>>,
    obj: Option<u64>,
}

const INF: f32 = 1.0e7;

impl Region {
    fn intervals(&self, y0: f32, y1: f32) -> Vec<(f32, f32)> {
        let mut base = vec![(self.x0, self.x1)];
        if let Some(p) = &self.polys {
            let inside: Vec<(f32, f32)> = geom::band_inside(p, y0 - self.pad, y1 + self.pad).into_iter().map(|(a, b)| (a + self.pad, b - self.pad)).filter(|(a, b)| b > a).collect();
            base = geom::intersect(&base, &inside);
        }
        for e in self.excl.iter() {
            let t = geom::band_touch(&e.polys, y0 - e.dist, y1 + e.dist);
            if t.is_empty() {
                continue;
            }
            let t: Vec<(f32, f32)> = t.iter().map(|&(a, b)| (a - e.dist, b + e.dist)).collect();
            let lo = t[0].0;
            let hi = t[t.len() - 1].1;
            let cut = match e.mode {
                WrapMode::Both => t,
                WrapMode::Left => vec![(lo, INF)],
                WrapMode::Right => vec![(-INF, hi)],
                // Nad i pod: tylko w kolumnie, na którą obiekt zachodzi (inne kolumny płyną dalej).
                WrapMode::TopBottom if hi <= self.x0 || lo >= self.x1 => continue,
                WrapMode::TopBottom => vec![(-INF, INF)],
                WrapMode::Largest => {
                    if lo - self.x0 >= self.x1 - hi {
                        vec![(lo, INF)]
                    } else {
                        vec![(-INF, hi)]
                    }
                }
                WrapMode::None | WrapMode::Behind => continue,
            };
            base = geom::subtract(&base, &geom::union(cut));
        }
        base
    }
}

fn obj_rect(o: &Object) -> geom::Rect {
    geom::Rect::new(o.x, o.y, o.w, o.h)
}

/// Kontur opływania obiektu w punktach strony.
fn wrap_polys(o: &Object) -> Vec<Vec<Pt>> {
    let r = obj_rect(o);
    if o.wrap.contour {
        let shape = match &o.kind {
            ObjKind::Image { contour: Some(c), .. } => c,
            _ => &o.shape,
        };
        let p = geom::shape_polys(shape, r);
        if !p.is_empty() {
            return p;
        }
    }
    geom::shape_polys(&Shape::rect(), r)
}

fn page_excl(doc: &Document, page: usize, above: Option<usize>, skip: &[u64]) -> Arc<Vec<Excl>> {
    Arc::new(
        doc.objects
            .iter()
            .enumerate()
            .filter(|(i, o)| o.on_page(page, doc.page.first_number) && o.wrap.mode.excludes() && !skip.contains(&o.id) && above.is_none_or(|a| *i > a))
            .map(|(_, o)| Excl { polys: wrap_polys(o), dist: o.wrap.dist, mode: o.wrap.mode })
            .collect(),
    )
}

fn body_regions(doc: &Document, ps: &PageSetup, page: usize, reserve: f32) -> Vec<Region> {
    let excl = page_excl(doc, page, None, &[]);
    let y1 = (ps.height - ps.margin_bottom - reserve).max(ps.margin_top + 36.0);
    ps.column_spans()
        .into_iter()
        .map(|(x0, x1)| Region { page, x0, x1, y0: ps.margin_top, y1, polys: None, pad: 0.0, excl: excl.clone(), obj: None })
        .collect()
}

fn frame_regions(doc: &Document, o: &Object, idx: usize, chain: &[u64]) -> Vec<Region> {
    let ObjKind::Text { padding, columns, gap, .. } = &o.kind else { return Vec::new() };
    let r = obj_rect(o);
    let mut polys = geom::shape_polys(&o.shape, r);
    if polys.is_empty() {
        polys = geom::shape_polys(&Shape::rect(), r);
    }
    let polys = Arc::new(polys);
    let excl = page_excl(doc, o.page, Some(idx), chain);
    let n = (*columns).max(1) as f32;
    let inner_x0 = r.x + padding;
    let inner_w = (r.w - 2.0 * padding).max(1.0);
    let cw = ((inner_w - gap * (n - 1.0)) / n).max(1.0);
    (0..(*columns).max(1))
        .map(|c| {
            let x0 = inner_x0 + c as f32 * (cw + gap);
            Region { page: o.page, x0, x1: x0 + cw, y0: r.y, y1: r.bottom(), polys: Some(polys.clone()), pad: *padding, excl: excl.clone(), obj: Some(o.id) }
        })
        .collect()
}

// ---------------------------------------------------------------- przepływ

struct Flow<'a> {
    doc: &'a Document,
    story: StoryRef,
    regions: Vec<Region>,
    ri: usize,
    y: f32,
    /// Żaden wiersz nie trafił jeszcze do tego regionu.
    top: bool,
    /// Treść główna dokłada strony.
    grow: bool,
    lines: Vec<Line>,
    overflow: bool,
    /// Inicjał: wiersze nad `y_end` zaczynają się za `x_end` (w regionie `ri`).
    drop: Option<(f32, f32, usize)>,
    grid: f32,
    /// Kontrola wdów: przed tym wierszem akapitu (licząc od 0) przejdź do następnego regionu.
    force_break: Option<usize>,
    /// Bieżący układ strony (sekcja) i układ każdej dotąd utworzonej strony.
    setup: PageSetup,
    setups: Vec<PageSetup>,
    /// Miejsce zarezerwowane u dołu strony na przypisy.
    reserve: Arc<HashMap<usize, f32>>,
}

impl<'a> Flow<'a> {
    fn region(&self) -> &Region {
        &self.regions[self.ri]
    }

    fn next_region(&mut self) -> bool {
        if self.ri + 1 >= self.regions.len() {
            if !self.grow {
                return false;
            }
            let page = self.regions.last().map(|r| r.page + 1).unwrap_or(0);
            if page > 5000 {
                return false;
            }
            let r = self.reserve.get(&page).copied().unwrap_or(0.0);
            let more = body_regions(self.doc, &self.setup, page, r);
            if self.setups.len() <= page {
                self.setups.resize(page + 1, self.setup.clone());
            }
            self.setups[page] = self.setup.clone();
            self.regions.extend(more);
        }
        self.ri += 1;
        self.y = self.regions[self.ri].y0;
        self.top = true;
        true
    }

    /// Początek sekcji: nowy układ od następnej strony albo (ciągła) od bieżącego miejsca.
    fn section(&mut self, s: &Section) {
        let ps = self.doc.page.with_section(s);
        if ps == self.setup {
            return;
        }
        let same_size = ps.width == self.setup.width && ps.height == self.setup.height;
        let page = self.region().page;
        let fresh = self.top && !self.lines.iter().any(|l| l.page == page);
        self.setup = ps;
        let r = self.reserve.get(&page).copied().unwrap_or(0.0);
        if fresh {
            self.regions.truncate(self.ri);
            self.regions.extend(body_regions(self.doc, &self.setup, page, r));
            if self.setups.len() <= page {
                self.setups.resize(page + 1, self.setup.clone());
            }
            self.setups[page] = self.setup.clone();
            self.y = self.regions[self.ri].y0;
            self.top = true;
            self.drop = None;
            return;
        }
        if s.continuous && same_size {
            let y = self.lines.iter().filter(|l| l.page == page).map(|l| l.bottom()).fold(self.setup.margin_top, f32::max) + 8.0;
            let mut more = body_regions(self.doc, &self.setup, page, r);
            if more.first().is_some_and(|m| m.y1 - y > 30.0) {
                for m in more.iter_mut() {
                    m.y0 = y;
                }
                self.regions.truncate(self.ri);
                self.regions.extend(more);
                self.y = y;
                self.top = true;
                self.drop = None;
                return;
            }
        }
        // Nowa strona – kolejne regiony powstaną już z nowym układem.
        self.regions.truncate(self.ri + 1);
        if !self.next_region() {
            self.overflow = true;
        }
    }

    fn next_page(&mut self) -> bool {
        let page = self.region().page;
        while self.region().page == page {
            if !self.next_region() {
                return false;
            }
        }
        true
    }
}

fn line_metrics(sp: &ShapedPara, gs: &[SGlyph]) -> (f32, f32, f32) {
    let mut asc: f32 = 0.0;
    let mut desc: f32 = 0.0;
    let mut gap: f32 = 0.0;
    let mut any = false;
    for g in gs {
        let st = sp.style(g);
        // Wysokość wiersza liczymy od pełnego rozmiaru (indeksy nie zmieniają odstępów).
        let (a, d, gp) = text::vmetrics(g.font, st.size.max(g.size));
        let rise = st.rise();
        asc = asc.max(a + rise.max(0.0));
        desc = desc.max(d - rise.min(0.0));
        gap = gap.max(gp);
        any = true;
    }
    if !any {
        let st = &sp.styles[sp.end_style as usize];
        let pk = crate::fonts::pick(&st.font, st.bold, st.italic);
        return text::vmetrics(pk.id, st.size);
    }
    (asc, desc, gap)
}

/// Wysokość wiersza i położenie linii bazowej od góry wiersza.
fn line_box(m: (f32, f32, f32), ls: LineSpacing) -> (f32, f32) {
    let (asc, desc, gap) = m;
    let nat = asc + desc + gap;
    let h = match ls {
        LineSpacing::Multiple(k) => nat * k.max(0.3),
        LineSpacing::Exact(v) => v.max(1.0),
        LineSpacing::AtLeast(v) => v.max(nat),
    };
    let extra = h - (asc + desc);
    (h, asc + extra * 0.4)
}

struct Filled {
    glyphs: Vec<PGlyph>,
    segs: Vec<LSeg>,
    end: usize,
    hard: bool,
}

/// Szerokość glifów `gs` od położenia `pen` (tabulatory liczone wg `tab`).
fn advance_of(g: &SGlyph, pen: f32, tab: &TabCtx, rest: impl Fn() -> f32) -> (f32, bool) {
    if g.kind != GKind::Tab {
        return (g.adv, false);
    }
    let rel = pen - tab.origin;
    for &t in &tab.stops {
        if t > 0.0 && t > rel + 0.5 {
            return (t - rel, false);
        }
    }
    if tab.right_leader {
        let w = (tab.right - pen - rest()).max(4.0);
        return (w, true);
    }
    let step = 36.0;
    let next = ((rel + 0.5) / step).floor() * step + step;
    (next - rel, false)
}

struct TabCtx {
    origin: f32,
    stops: Vec<f32>,
    right_leader: bool,
    right: f32,
}

fn fill(sp: &ShapedPara, start: usize, ivs: &[(f32, f32)], tabs: &mut TabCtx, full_w: f32) -> Filled {
    let gl = &sp.glyphs;
    let n = gl.len();
    let word_end = |gi: usize| -> usize {
        let k = sp.breaks.partition_point(|&b| b <= gi);
        sp.breaks.get(k).copied().unwrap_or(n)
    };
    let widest = ivs.iter().map(|(a, b)| b - a).fold(0.0, f32::max);
    let mut out: Vec<PGlyph> = Vec::new();
    let mut segs = Vec::new();
    let mut gi = start;
    let mut hard = false;
    let rest_width = |from: usize| -> f32 {
        gl[from..].iter().take_while(|g| g.kind != GKind::Newline && g.kind != GKind::Tab).map(|g| g.adv).sum()
    };
    'outer: for &(x0, x1) in ivs {
        let mut pen = x0;
        let g0 = out.len();
        tabs.right = x1;
        while gi < n {
            if gl[gi].kind == GKind::Newline {
                out.push(PGlyph { g: gl[gi], x: pen, y: 0.0, adv: 0.0, leader: false });
                gi += 1;
                hard = true;
                segs.push(LSeg { x0, x1, g0, g1: out.len() });
                break 'outer;
            }
            let we = word_end(gi).max(gi + 1);
            // Szerokość słowa bez spacji na końcu.
            let mut p = pen;
            let mut trim_end = pen;
            let mut advs = Vec::with_capacity(we - gi);
            for k in gi..we {
                if gl[k].kind == GKind::Newline {
                    break;
                }
                let (a, lead) = advance_of(&gl[k], p, tabs, || rest_width(k + 1));
                advs.push((a, lead));
                p += a;
                if gl[k].kind != GKind::Space {
                    trim_end = p;
                }
            }
            let hyw = if sp.hyph.binary_search(&we).is_ok() { gl[gi].size * 0.35 } else { 0.0 };
            if trim_end + hyw <= x1 + 0.01 {
                let count = advs.len();
                for (k, (a, lead)) in (gi..).zip(advs) {
                    out.push(PGlyph { g: gl[k], x: pen, y: 0.0, adv: a, leader: lead });
                    pen += a;
                }
                // Złamanie wiersza na końcu słowa obsłuży następny obieg pętli.
                gi += count.max(1).min(n - gi);
                continue;
            }
            if pen <= x0 + 0.01 && out.is_empty() && (x1 - x0) >= widest - 0.01 && (x1 - x0) >= full_w * 0.6 {
                // Słowo dłuższe niż cały wiersz – łamiemy je na glifach (co najmniej jeden).
                let mut placed = 0;
                while gi < n && gl[gi].kind != GKind::Newline {
                    let a = gl[gi].adv;
                    if placed > 0 && pen + a > x1 + 0.01 {
                        break;
                    }
                    out.push(PGlyph { g: gl[gi], x: pen, y: 0.0, adv: a, leader: false });
                    pen += a;
                    gi += 1;
                    placed += 1;
                }
                segs.push(LSeg { x0, x1, g0, g1: out.len() });
                break 'outer;
            }
            break;
        }
        segs.push(LSeg { x0, x1, g0, g1: out.len() });
        if gi >= n {
            break;
        }
    }
    Filled { glyphs: out, segs, end: gi, hard }
}

fn align_line(f: &mut Filled, align: Align, last: bool) {
    for s in &f.segs {
        if s.g1 <= s.g0 {
            continue;
        }
        let gs = &mut f.glyphs[s.g0..s.g1];
        let Some(li) = gs.iter().rposition(|g| g.g.kind != GKind::Space && g.g.kind != GKind::Newline) else { continue };
        let end = gs[li].x + gs[li].adv;
        let extra = s.x1 - end;
        if extra <= 0.0 {
            continue;
        }
        match align {
            Align::Left => {}
            Align::Center => gs.iter_mut().for_each(|g| g.x += extra / 2.0),
            Align::Right => gs.iter_mut().for_each(|g| g.x += extra),
            Align::Justify => {
                if last || f.hard {
                    continue;
                }
                // Bez rozpychania po tabulatorze.
                let from = gs[..=li].iter().rposition(|g| g.g.kind == GKind::Tab).map(|i| i + 1).unwrap_or(0);
                let spaces = gs[from..li].iter().filter(|g| g.g.kind == GKind::Space).count();
                if spaces == 0 || extra > (s.x1 - s.x0) * 0.45 {
                    continue;
                }
                let add = extra / spaces as f32;
                let mut shift = 0.0;
                for g in gs[from..].iter_mut() {
                    g.x += shift;
                    if g.g.kind == GKind::Space && shift < extra - 0.001 {
                        g.adv += add;
                        shift += add;
                    }
                }
            }
        }
    }
}

const LIST_INDENT: f32 = 18.0;

impl<'a> Flow<'a> {
    #[allow(clippy::too_many_arguments)]
    fn place_para(&mut self, pi: usize, sp: Arc<ShapedPara>, pp: &ParaProps, marker: Option<String>, next_h: f32) -> bool {
        if self.overflow {
            return false;
        }
        match pp.break_before {
            Break::Page if !(self.top && self.lines.is_empty()) => {
                if !self.next_page() {
                    self.overflow = true;
                    return false;
                }
            }
            Break::Column if !self.top => {
                if !self.next_region() {
                    self.overflow = true;
                    return false;
                }
            }
            _ => {}
        }
        if !self.top {
            self.y += pp.space_before;
        }
        let n = sp.glyphs.len();
        let list_lvl = pp.list.map(|l| l.level as f32 + 1.0).unwrap_or(0.0);
        let left = pp.indent_left + list_lvl * LIST_INDENT;
        let first_m = line_metrics(&sp, &sp.glyphs[..n.min(24)]);
        let (h_first, _) = line_box(first_m, pp.line);
        // Nagłówek trzyma się następnego akapitu.
        if pp.keep_next && !self.top {
            let need = h_first + pp.space_after + next_h;
            if self.y + need > self.region().y1 {
                if !self.next_region() {
                    self.overflow = true;
                    return false;
                }
            }
        }
        let first_style = sp.glyphs.first().map(|g| g.style).unwrap_or(sp.end_style) as usize;
        let mut gi = 0;
        // Inicjał – pierwszy znak w powiększeniu na kilka wierszy.
        let mut drop_glyphs: Vec<SGlyph> = Vec::new();
        let mut drop_props: Option<CharProps> = None;
        if pp.drop_cap >= 2 && n > 1 {
            let c0 = sp.glyphs[0].cluster;
            let end = sp.glyphs.iter().position(|g| g.cluster != c0).unwrap_or(n);
            let ch: String = sp.text[c0..sp.glyphs.get(end).map(|g| g.cluster).unwrap_or(sp.text.len())].to_string();
            let mut props = sp.styles[first_style].clone();
            let k = pp.drop_cap as f32;
            props.size = ((k - 1.0) * h_first + props.size * 0.7) / 0.7;
            props.script = Script::Normal;
            let (gs, _) = text::measure(&ch, &props);
            drop_glyphs = gs.into_iter().map(|mut g| {
                g.cluster = c0;
                g
            }).collect();
            drop_props = Some(props);
            gi = end;
        }
        let mut first = true;
        let mut lines_here = 0usize;
        loop {
            if self.force_break == Some(lines_here) && !self.top {
                self.force_break = None;
                if !self.next_region() {
                    self.overflow = true;
                    return false;
                }
            }
            let m = line_metrics(&sp, &sp.glyphs[gi..n.min(gi + 24)]);
            let (mut h, mut base) = line_box(m, pp.line);
            let mut tries = 0;
            let mut steps = 0;
            let filled = loop {
                let r = self.region().clone();
                if self.y + h > r.y1 + 0.01 && !self.top {
                    if !self.next_region() {
                        self.overflow = true;
                        return false;
                    }
                    continue;
                }
                if self.y >= r.y1 - 0.5 && self.top && r.y1 - r.y0 < h {
                    // Region niższy niż wiersz – kładziemy mimo to (inaczej pętla bez końca).
                }
                // Siatka bazowa (tylko treść główna).
                if self.grid > 0.0 && self.story == StoryRef::Body {
                    let b = self.y + base;
                    let snapped = r.y0 + ((b - r.y0) / self.grid).ceil() * self.grid;
                    self.y += snapped - b;
                }
                let mut ivs = r.intervals(self.y, self.y + h);
                if let Some((x_end, y_end, dri)) = self.drop {
                    if dri == self.ri && self.y < y_end {
                        ivs = geom::subtract(&ivs, &[(-INF, x_end)]);
                    }
                }
                // Wcięcia: lewe w pierwszym odcinku, prawe w ostatnim.
                let ind_l = left + if first && pp.list.is_none() { pp.first_line } else { 0.0 };
                if let Some(f) = ivs.first_mut() {
                    f.0 += ind_l.max(-(f.0 - r.x0));
                }
                if let Some(l) = ivs.last_mut() {
                    l.1 -= pp.indent_right;
                }
                let min_w = (sp.glyphs.get(gi).map(|g| g.size).unwrap_or(11.0) * 0.5).max(6.0);
                ivs.retain(|(a, b)| b - a >= min_w);
                if ivs.is_empty() {
                    steps += 1;
                    self.y += 2.0;
                    if self.y + h > r.y1 || steps > 4000 {
                        if !self.next_region() {
                            self.overflow = true;
                            return false;
                        }
                    }
                    continue;
                }
                let mut tabs = TabCtx { origin: ivs[0].0, stops: pp.tabs.clone(), right_leader: pp.tabs.iter().any(|t| *t < 0.0), right: ivs[0].1 };
                let mut f = fill(&sp, gi, &ivs, &mut tabs, r.x1 - r.x0);
                if f.glyphs.is_empty() && gi < n {
                    steps += 1;
                    self.y += 2.0;
                    if self.y + h > r.y1 || steps > 4000 {
                        if !self.next_region() {
                            self.overflow = true;
                            return false;
                        }
                    }
                    continue;
                }
                let real: Vec<SGlyph> = f.glyphs.iter().map(|g| g.g).collect();
                let m2 = line_metrics(&sp, &real);
                let (h2, b2) = line_box(m2, pp.line);
                if h2 > h + 0.01 && tries < 3 {
                    h = h2;
                    base = b2;
                    tries += 1;
                    continue;
                }
                // Sierota akapitu: pierwszy wiersz sam na dole regionu.
                if first && pp.widows && !self.top && f.end < n && self.y + h + h2 > r.y1 + 0.01 && lines_here == 0 {
                    if self.next_region() {
                        continue;
                    }
                }
                let last = f.end >= n;
                align_line(&mut f, pp.align, last);
                if f.glyphs.is_empty() {
                    // Pusty akapit – kursor tam, gdzie zacząłby się tekst.
                    let (x0, x1) = ivs[0];
                    let ex = match pp.align {
                        Align::Center => (x0 + x1) / 2.0,
                        Align::Right => x1,
                        _ => x0,
                    };
                    f.segs = vec![LSeg { x0, x1, g0: 0, g1: 0 }];
                    break (f, ex, ivs[0].0);
                }
                let x_first = ivs[0].0;
                break (f, x_first, x_first);
            };
            let (mut f, empty_x, text_x) = filled;
            if f.end < n && sp.hyph.binary_search(&f.end).is_ok() {
                if let Some(last) = f.glyphs.last().copied().filter(|g| g.g.kind == GKind::Normal) {
                    if let Some(fl) = crate::fonts::get(last.g.font) {
                        if let Some(gid) = fl.face.glyph_index('-') {
                            let adv = fl.face.glyph_hor_advance(gid).unwrap_or(0) as f32 * last.g.size / fl.upem;
                            let mut g = last.g;
                            g.gid = gid.0;
                            g.adv = adv;
                            g.dx = 0.0;
                            g.dy = 0.0;
                            f.glyphs.push(PGlyph { g, x: last.x + last.adv, y: 0.0, adv, leader: false });
                            if let Some(s) = f.segs.last_mut() {
                                s.g1 = f.glyphs.len();
                            }
                        }
                    }
                }
            }
            let baseline = self.y + base;
            for g in f.glyphs.iter_mut() {
                let st = sp.style(&g.g);
                g.y = baseline - st.rise() - g.g.dy;
            }
            let start = if first { 0 } else { sp.glyphs.get(gi).map(|g| g.cluster).unwrap_or(sp.text.len()) };
            let end_g = f.end;
            let end = sp.glyphs.get(end_g).map(|g| g.cluster).unwrap_or(sp.text.len());
            let r = self.region().clone();
            let mut mk: Vec<PGlyph> = Vec::new();
            let mut mk_style = None;
            if first {
                if let Some(mtext) = &marker {
                    let props = sp.styles[first_style].clone();
                    let (gs, w) = text::measure(mtext, &props);
                    let mx = (text_x - LIST_INDENT).max(r.x0);
                    let mut x = if w > LIST_INDENT - 3.0 { (text_x - w - 4.0).max(r.x0) } else { mx };
                    for g in gs {
                        mk.push(PGlyph { g, x, y: baseline - g.dy, adv: g.adv, leader: false });
                        x += g.adv;
                    }
                    mk_style = Some(props);
                }
                if let Some(props) = drop_props.take() {
                    let k = pp.drop_cap as f32;
                    let dbase = baseline + (k - 1.0) * h;
                    let mut x = text_x;
                    for g in &drop_glyphs {
                        mk.push(PGlyph { g: *g, x, y: dbase - g.dy, adv: g.adv, leader: false });
                        x += g.adv;
                    }
                    self.drop = Some((x + 4.0, self.y + k * h - 0.5, self.ri));
                    mk_style = Some(props);
                    // Pierwszy wiersz też zaczyna się za inicjałem.
                    let shift = (x + 4.0) - text_x;
                    for g in f.glyphs.iter_mut() {
                        g.x += shift;
                    }
                }
            }
            self.lines.push(Line {
                story: self.story,
                para: pi,
                page: r.page,
                top: self.y,
                height: h,
                baseline,
                start,
                end,
                glyphs: f.glyphs,
                segs: f.segs,
                first,
                last: end_g >= n,
                marker: mk,
                marker_style: mk_style,
                area: (r.x0 + pp.indent_left, r.x1 - pp.indent_right),
                shaped: sp.clone(),
                obj: r.obj,
                empty_x,
            });
            self.y += h;
            self.top = false;
            lines_here += 1;
            first = false;
            gi = end_g;
            if gi >= n {
                break;
            }
        }
        if let Some(l) = self.lines.last_mut() {
            l.last = true;
        }
        self.y += pp.space_after;
        true
    }
}

// ---------------------------------------------------------------- silnik

#[derive(Default)]
pub struct Engine {
    cache: HashMap<u64, (u64, Arc<ShapedPara>)>,
    gen: u64,
    /// Strony celów odsyłaczy z poprzedniego składu.
    targets: HashMap<u64, (u32, String)>,
    /// Trwa powtórny skład po zmianie stron celów.
    again: bool,
}

fn key(p: &Paragraph, base: &CharProps, fv: Option<&FieldValues>, orphans: bool) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}{:?}{:?}", p.runs, base, orphans).hash(&mut h);
    if let Some(fv) = fv {
        for f in p.runs.iter().filter_map(|r| r.field) {
            fv.text(f).hash(&mut h);
        }
    }
    h.finish()
}

struct StoryPrep {
    shaped: Vec<Arc<ShapedPara>>,
    props: Vec<ParaProps>,
    markers: Vec<Option<String>>,
}

impl Engine {
    pub fn new() -> Engine {
        Engine::default()
    }

    fn shaped(&mut self, doc: &Document, p: &Paragraph, fv: &FieldValues) -> (Arc<ShapedPara>, ParaProps) {
        let (pp, base) = doc.resolve_para(p);
        let hy = pp.hyphenate || doc.settings.hyphenate;
        let k = key(p, &base, Some(fv), doc.settings.orphans) ^ if hy { 0x9e37_79b9_7f4a_7c15 } else { 0 };
        let gen = self.gen;
        if let Some((g, sp)) = self.cache.get_mut(&k) {
            *g = gen;
            return (sp.clone(), pp);
        }
        let sp = Arc::new(text::shape_ex(p, &base, fv, doc.settings.orphans, hy));
        self.cache.insert(k, (gen, sp.clone()));
        (sp, pp)
    }

    fn prep(&mut self, doc: &Document, story: &Story, fv: &FieldValues) -> StoryPrep {
        let mut out = StoryPrep { shaped: Vec::new(), props: Vec::new(), markers: Vec::new() };
        let mut counters = [0u32; 9];
        let mut last_kind: [Option<ListKind>; 9] = [None; 9];
        for p in &story.paras {
            let (sp, pp) = self.shaped(doc, p, fv);
            let marker = match pp.list {
                Some(l) => {
                    let lv = (l.level as usize).min(8);
                    if last_kind[lv] != Some(l.kind) {
                        counters[lv] = 0;
                        last_kind[lv] = Some(l.kind);
                    }
                    if let Some(s) = l.start {
                        counters[lv] = s.saturating_sub(1);
                    }
                    counters[lv] += 1;
                    for d in lv + 1..9 {
                        counters[d] = 0;
                        last_kind[d] = None;
                    }
                    if l.kind == ListKind::Multi {
                        Some(counters[..=lv].iter().map(|c| c.max(&1).to_string()).collect::<Vec<_>>().join(".") + ".")
                    } else {
                        Some(l.kind.marker(counters[lv], l.level))
                    }
                }
                None => {
                    counters = [0; 9];
                    last_kind = [None; 9];
                    None
                }
            };
            out.shaped.push(sp);
            out.props.push(pp);
            out.markers.push(marker);
        }
        out
    }

    /// Tekst przypisu: wcięty, z numerem w miejscu znacznika listy.
    fn note_prep(&mut self, doc: &Document, story: &Story, num: &str, fv: &FieldValues) -> StoryPrep {
        let mut np = self.prep(doc, story, fv);
        for (i, pp) in np.props.iter_mut().enumerate() {
            pp.indent_left += 14.0;
            pp.first_line = 0.0;
            pp.list = None;
            pp.drop_cap = 0;
            np.markers[i] = None;
        }
        if let Some(m) = np.markers.first_mut() {
            *m = Some(num.to_string());
        }
        np
    }

    #[allow(clippy::too_many_arguments)]
    fn note_lines(&mut self, doc: &Document, id: u64, k: u32, page: usize, x0: f32, x1: f32, y0: f32, fv: &FieldValues) -> Vec<Line> {
        let Some(n) = doc.notes.iter().find(|n| n.id == id) else { return Vec::new() };
        let np = self.note_prep(doc, &n.story, &k.to_string(), fv);
        let region = Region { page, x0, x1, y0, y1: INF, polys: None, pad: 0.0, excl: Arc::new(Vec::new()), obj: None };
        let mut f = Flow { doc, story: StoryRef::Note(id), regions: vec![region], ri: 0, y: y0, top: true, grow: false, lines: Vec::new(), overflow: false, drop: None, grid: 0.0, force_break: None, setup: doc.page.clone(), setups: Vec::new(), reserve: Default::default() };
        self.run(&mut f, &np);
        f.lines
    }

    fn run(&mut self, flow: &mut Flow, prep: &StoryPrep) {
        for i in 0..prep.shaped.len() {
            if flow.story == StoryRef::Body && flow.grow {
                if let Some(sec) = flow.doc.body.paras.get(i).and_then(|p| p.fmt.section.clone()) {
                    flow.section(&sec);
                    if flow.overflow {
                        break;
                    }
                }
            }
            let next_h = match prep.shaped.get(i + 1) {
                Some(sp) => line_box(line_metrics(sp, &sp.glyphs[..sp.glyphs.len().min(24)]), prep.props[i + 1].line).0,
                None => 0.0,
            };
            let saved = (flow.lines.len(), flow.ri, flow.y, flow.top, flow.drop);
            if !flow.place_para(i, prep.shaped[i].clone(), &prep.props[i], prep.markers[i].clone(), next_h) {
                break;
            }
            // Wdowa: ostatni wiersz akapitu sam w nowym regionie – przenosimy też przedostatni.
            if prep.props[i].widows {
                let mine: Vec<&Line> = flow.lines[saved.0..].iter().collect();
                let n = mine.len();
                if n >= 3 {
                    let last = mine[n - 1];
                    let prev = mine[n - 2];
                    let moved = last.page != prev.page || last.top < prev.top;
                    if moved {
                        let k = n - 2;
                        flow.lines.truncate(saved.0);
                        flow.ri = saved.1;
                        flow.y = saved.2;
                        flow.top = saved.3;
                        flow.drop = saved.4;
                        flow.force_break = Some(k);
                        if !flow.place_para(i, prep.shaped[i].clone(), &prep.props[i], prep.markers[i].clone(), next_h) {
                            break;
                        }
                        flow.force_break = None;
                    }
                }
            }
        }
    }

    pub fn layout(&mut self, doc: &Document) -> Layout {
        self.gen += 1;
        let mut out = Layout::default();
        let title = doc.meta.title.clone();
        let author = doc.meta.author.clone();

        // Treść główna – kilka przebiegów, aż miejsce na przypisy dolne przestanie się zmieniać.
        let notes = doc.note_numbers();
        let m = &doc.meta;
        let ext = FieldExtra { seqs: doc.seq_numbers(), targets: self.targets.clone(), meta: [m.subject.clone(), m.keywords.clone(), crate::util::short_date(&m.created), crate::util::short_date(&m.modified)] };
        let fv = FieldValues { page: doc.page.first_number, pages: 0, title: &title, author: &author, notes: &notes, ext: Some(&ext), running: "" };
        let prep = self.prep(doc, &doc.body, &fv);
        let mut reserve: HashMap<usize, f32> = HashMap::new();
        let mut heights: HashMap<(u64, u32), f32> = HashMap::new();
        let mut pass = 0;
        let (mut flow, foot) = loop {
            let r0 = reserve.get(&0).copied().unwrap_or(0.0);
            let mut flow = Flow { doc, story: StoryRef::Body, regions: body_regions(doc, &doc.page, 0, r0), ri: 0, y: doc.page.margin_top, top: true, grow: true, lines: Vec::new(), overflow: false, drop: None, grid: doc.settings.baseline_grid, force_break: None, setup: doc.page.clone(), setups: vec![doc.page.clone()], reserve: Arc::new(reserve.clone()) };
            self.run(&mut flow, &prep);
            // Przypisy końcowe – za treścią główną, w tym samym przepływie.
            for &(id, k, end) in notes.iter() {
                if !end {
                    continue;
                }
                let Some(n) = doc.notes.iter().find(|n| n.id == id) else { continue };
                let np = self.note_prep(doc, &n.story, &crate::text::roman(k), &fv);
                flow.story = StoryRef::Note(id);
                self.run(&mut flow, &np);
                flow.story = StoryRef::Body;
            }
            // Przypisy dolne każdej strony.
            let mut foot: Vec<(usize, Vec<(u64, u32)>)> = Vec::new();
            for l in flow.lines.iter().filter(|l| l.story == StoryRef::Body) {
                let Some(p) = doc.body.paras.get(l.para) else { continue };
                let mut off = 0;
                for r in &p.runs {
                    if let Some(Field::Footnote(id)) = r.field {
                        if off >= l.start && (off < l.end || (l.last && off <= l.end)) {
                            if let Some(&(_, k, false)) = notes.iter().find(|n| n.0 == id) {
                                match foot.last_mut() {
                                    Some((pg, v)) if *pg == l.page => v.push((id, k)),
                                    _ => foot.push((l.page, vec![(id, k)])),
                                }
                            }
                        }
                    }
                    off += r.text.len();
                }
            }
            let mut need: HashMap<usize, f32> = HashMap::new();
            for (pg, ids) in &foot {
                let ps = flow.setups.get(*pg).cloned().unwrap_or_else(|| doc.page.clone());
                let w = ps.content_width();
                let mut h = 10.0;
                for &(id, k) in ids {
                    let key = (id, (w * 10.0) as u32);
                    let hh = match heights.get(&key) {
                        Some(v) => *v,
                        None => {
                            let lines = self.note_lines(doc, id, k, *pg, ps.margin_left, ps.margin_left + w, 0.0, &fv);
                            let v = lines.last().map(|l| l.bottom()).unwrap_or(0.0) + 2.0;
                            heights.insert(key, v);
                            v
                        }
                    };
                    h += hh;
                }
                need.insert(*pg, h.min(ps.height * 0.6));
            }
            pass += 1;
            if need == reserve || pass >= 4 {
                break (flow, foot);
            }
            // Rezerwa tylko rośnie – unikamy przeskakiwania odsyłacza tam i z powrotem.
            for (k, v) in need {
                let e = reserve.entry(k).or_insert(0.0);
                *e = e.max(v);
            }
        };
        let body_lines = std::mem::take(&mut flow.lines);
        let body_pages = body_lines.last().map(|l| l.page + 1).unwrap_or(1);
        let pages = body_pages.max(doc.page_count_hint());
        let last_setup = flow.setup.clone();
        out.pages = (0..pages).map(|p| PageInfo { setup: flow.setups.get(p).cloned().unwrap_or_else(|| last_setup.clone()), ..Default::default() }).collect();
        out.body_pages = vec![0; doc.body.paras.len()];
        for l in body_lines.iter().filter(|l| l.story == StoryRef::Body) {
            if l.first {
                if let Some(p) = out.body_pages.get_mut(l.para) {
                    *p = l.page;
                }
            }
        }
        // Cele odsyłaczy: gdy zmieniły się ich strony, składamy jeszcze raz (pola mają inną treść).
        let targets = ref_targets(doc, &out.body_pages);
        if targets != self.targets {
            let has_refs = doc.body.paras.iter().any(|p| p.runs.iter().any(|r| matches!(r.field, Some(Field::Ref(..)))));
            self.targets = targets;
            if has_refs && !self.again {
                self.again = true;
                let l = self.layout(doc);
                self.again = false;
                return l;
            }
        }
        let running = running_heads(doc, &out.body_pages, pages);
        let push = |out: &mut Layout, lines: Vec<Line>| {
            for l in lines {
                let i = out.lines.len();
                if l.page < out.pages.len() {
                    out.pages[l.page].lines.push(i);
                }
                out.stories.entry(l.story).or_default().push(i);
                out.lines.push(l);
            }
        };
        push(&mut out, body_lines);

        // Przypisy dolne: od dołu obszaru tekstu, pod krótką kreską.
        for (pg, ids) in &foot {
            if *pg >= out.pages.len() {
                continue;
            }
            let ps = out.pages[*pg].setup.clone();
            let w = ps.content_width();
            let r = reserve.get(pg).copied().unwrap_or(0.0);
            let mut y = ps.height - ps.margin_bottom - r + 10.0;
            out.pages[*pg].note_rule = Some((y - 5.0, ps.margin_left, (w / 3.0).min(144.0)));
            for &(id, k) in ids {
                let lines = self.note_lines(doc, id, k, *pg, ps.margin_left, ps.margin_left + w, y, &fv);
                y = lines.last().map(|l| l.bottom()).unwrap_or(y) + 2.0;
                push(&mut out, lines);
            }
        }

        // Nagłówki i stopki – osobno na każdej stronie (pola numeru strony).
        for page in 0..pages {
            let ps = &out.pages[page].setup.clone();
            let first = page == 0 && ps.different_first;
            let even = !first && doc.page.odd_even && (page as u32 + ps.first_number) % 2 == 0;
            let empty = [String::new(), String::new(), String::new()];
            let rh = running.get(page).unwrap_or(&empty);
            let mut fvp = FieldValues { page: page as u32 + ps.first_number, pages: pages as u32 + ps.first_number - 1, title: &title, author: &author, notes: &notes, ext: Some(&ext), running: "" };
            let (hs, hr) = if first {
                (doc.header_first.as_ref(), StoryRef::HeaderFirst)
            } else if even && doc.header_even.is_some() {
                (doc.header_even.as_ref(), StoryRef::HeaderEven)
            } else {
                (doc.header.as_ref(), StoryRef::Header)
            };
            let (fs, fr) = if first {
                (doc.footer_first.as_ref(), StoryRef::FooterFirst)
            } else if even && doc.footer_even.is_some() {
                (doc.footer_even.as_ref(), StoryRef::FooterEven)
            } else {
                (doc.footer.as_ref(), StoryRef::Footer)
            };
            for (story, sref, footer) in [(hs, hr, false), (fs, fr, true)] {
                let Some(story) = story else { continue };
                // Żywa pagina: poziom z pierwszego pola `Running` w tym tekście.
                let lvl = story.paras.iter().flat_map(|p| &p.runs).find_map(|r| match r.field {
                    Some(Field::Running(n)) => Some(n.clamp(1, 3) as usize - 1),
                    _ => None,
                });
                fvp.running = lvl.map(|i| rh[i].as_str()).unwrap_or("");
                let prep = self.prep(doc, story, &fvp);
                let region = Region {
                    page,
                    x0: ps.margin_left,
                    x1: ps.width - ps.margin_right,
                    y0: if footer { 0.0 } else { ps.header_dist },
                    y1: INF,
                    polys: None,
                    pad: 0.0,
                    excl: Arc::new(Vec::new()),
                    obj: None,
                };
                let mut f = Flow { doc, story: sref, regions: vec![region], ri: 0, y: 0.0, top: true, grow: false, lines: Vec::new(), overflow: false, drop: None, grid: 0.0, force_break: None, setup: doc.page.clone(), setups: Vec::new(), reserve: Default::default() };
                f.y = f.regions[0].y0;
                self.run(&mut f, &prep);
                let mut lines = f.lines;
                if footer {
                    let bottom = lines.last().map(|l| l.bottom()).unwrap_or(0.0);
                    let shift = ps.height - ps.footer_dist - bottom;
                    for l in lines.iter_mut() {
                        l.top += shift;
                        l.baseline += shift;
                        for g in l.glyphs.iter_mut().chain(l.marker.iter_mut()) {
                            g.y += shift;
                        }
                    }
                }
                push(&mut out, lines);
            }
        }

        // Ramki tekstowe (łańcuchy) i tekst na ścieżce.
        let ps = &doc.page;
        let fvo = FieldValues { page: 1, pages: pages as u32, title: &title, author: &author, notes: &notes, ext: Some(&ext), running: "" };
        let heads: Vec<u64> = doc.objects.iter().filter(|o| matches!(o.kind, ObjKind::Text { .. }) && doc.chain_head(o.id) == o.id).map(|o| o.id).collect();
        for head in heads {
            let chain = doc.chain(head);
            let Some(ho) = doc.object(head) else { continue };
            let Some(story) = ho.story() else { continue };
            let mut regions = Vec::new();
            for id in &chain {
                if let Some((idx, o)) = doc.objects.iter().enumerate().find(|(_, o)| o.id == *id) {
                    regions.extend(frame_regions(doc, o, idx, &chain));
                }
            }
            if regions.is_empty() {
                continue;
            }
            let fvh = FieldValues { page: ho.page as u32 + ps.first_number, ..fvo };
            let prep = self.prep(doc, story, &fvh);
            let y0 = regions[0].y0;
            let mut f = Flow { doc, story: StoryRef::Obj(head), regions, ri: 0, y: y0, top: true, grow: false, lines: Vec::new(), overflow: false, drop: None, grid: 0.0, force_break: None, setup: doc.page.clone(), setups: Vec::new(), reserve: Default::default() };
            // Górny margines wewnętrzny ramki.
            f.y += f.regions[0].pad;
            self.run(&mut f, &prep);
            if f.overflow {
                out.overflow.insert(*chain.last().unwrap());
            }
            let mut lines = f.lines;
            // Wyrównanie w pionie – tylko pojedyncza ramka prostokątna.
            if chain.len() == 1 && ho.shape.is_rect() {
                if let ObjKind::Text { valign, padding, .. } = &ho.kind {
                    if *valign != VAlign::Top && !lines.is_empty() {
                        let bottom = lines.iter().map(|l| l.bottom()).fold(0.0, f32::max);
                        let free = ho.y + ho.h - padding - bottom;
                        let shift = if *valign == VAlign::Center { free / 2.0 } else { free };
                        if shift > 0.0 {
                            for l in lines.iter_mut() {
                                l.top += shift;
                                l.baseline += shift;
                                for g in l.glyphs.iter_mut().chain(l.marker.iter_mut()) {
                                    g.y += shift;
                                }
                            }
                        }
                    }
                }
            }
            push(&mut out, lines);
        }
        for o in &doc.objects {
            if let ObjKind::PathText { story, offset } = &o.kind {
                let Some(c) = o.shape.contours.first() else { continue };
                let poly = {
                    let mut p = geom::flatten(c, obj_rect(o), 0.8);
                    if c.closed {
                        if let Some(f) = p.first().copied() {
                            p.push(f);
                        }
                    }
                    p
                };
                let lens = geom::polyline_lengths(&poly);
                let total = *lens.last().unwrap_or(&0.0);
                let mut glyphs = Vec::new();
                if let Some(p) = story.paras.first() {
                    let (sp, pp) = self.shaped(doc, p, &fvo);
                    let width: f32 = sp.glyphs.iter().map(|g| g.adv).sum();
                    let mut s = match pp.align {
                        Align::Center => (total - width) / 2.0,
                        Align::Right => total - width,
                        _ => 0.0,
                    } + offset * total;
                    for g in &sp.glyphs {
                        let mid = s + g.adv / 2.0;
                        if let Some((q, ang)) = geom::along(&poly, &lens, mid) {
                            let st = sp.style(g);
                            let (sin, cos) = ang.sin_cos();
                            let half = g.adv / 2.0;
                            let rise = st.rise() + g.dy;
                            glyphs.push(PathGlyph { g: *g, x: q.x - cos * half + sin * rise, y: q.y - sin * half - cos * rise, angle: ang });
                        }
                        s += g.adv;
                    }
                }
                out.path_glyphs.insert(o.id, glyphs);
            }
        }

        // Porządki w pamięci kształtowania: wyrzucamy to, czego ten skład nie użył.
        let gen = self.gen;
        if self.cache.len() > 4096 {
            self.cache.retain(|_, (g, _)| *g + 2 >= gen);
        }
        out
    }
}

/// Obiekty zakotwiczone w treści głównej: stają pod poprzednim akapitem (z odstępem `anchor_dy`),
/// a tabela, która nie mieści się na stronie, dzieli się – reszta wierszy (z powtórzonym nagłówkiem)
/// trafia do kontynuacji na następnej stronie. Zwraca, czy coś się zmieniło (wtedy trzeba złożyć ponownie).
pub fn apply_anchors(doc: &mut Document, lay: &Layout) -> bool {
    let before = doc.objects.clone();
    let min_before = doc.min_pages;
    doc.objects.retain(|o| o.cont_of.is_none());
    let mut anchors: Vec<(u64, usize)> = Vec::new();
    for (pi, p) in doc.body.paras.iter().enumerate() {
        for r in &p.runs {
            if let Some(Field::Anchor(id)) = r.field {
                if !anchors.iter().any(|a| a.0 == id) {
                    anchors.push((id, pi));
                }
            }
        }
    }
    let ps = doc.page.clone();
    let bottom = ps.height - ps.margin_bottom;
    let body = lay.story_lines(StoryRef::Body);
    let mut conts = Vec::new();
    for (id, pi) in anchors {
        // Punkt zakotwiczenia: koniec ostatniego wiersza przed akapitem kotwicy.
        let prev = body.iter().map(|&i| &lay.lines[i]).filter(|l| l.para < pi).last();
        let (page, refy) = match prev {
            Some(l) => (l.page, l.bottom()),
            None => (0, ps.margin_top),
        };
        // Kolumna, w której kończy się poprzedni akapit – zakotwiczona tabela dopasowuje się do niej.
        let col = prev.map(|l| l.segs.first().map(|s| s.x0).unwrap_or(l.empty_x)).and_then(|x| ps.column_spans().into_iter().find(|(a, b)| x >= *a - 1.0 && x <= *b + 1.0));
        let Some(o) = doc.objects.iter_mut().find(|o| o.id == id) else { continue };
        if let (Some((x0, x1)), ObjKind::Table { .. }) = (col, &o.kind) {
            if o.w > x1 - x0 + 0.5 || o.x < x0 - 0.5 || o.x + o.w > x1 + 0.5 {
                o.x = x0;
                o.w = o.w.min(x1 - x0);
            }
        }
        let mut page = page;
        // Odstęp co najmniej taki jak margines opływania – inaczej obiekt spycha wiersz, za którym stoi.
        let mut y = refy + o.anchor_dy.max(if o.wrap.mode.excludes() { o.wrap.dist + 1.0 } else { 0.0 });
        let snapshot = o.clone();
        if let ObjKind::Table { table } = &mut o.kind {
            table.take = None;
            let full = crate::paint::table_height(table, o.w);
            if y + full > bottom + 0.5 {
                let heights = crate::paint::table_row_heights(table, o.w);
                let mut acc = 0.0;
                let mut k = 0;
                for h in &heights {
                    if y + acc + h > bottom {
                        break;
                    }
                    acc += h;
                    k += 1;
                }
                let head = if table.header { 1 } else { 0 };
                if k <= head {
                    // Nawet pierwszy wiersz nie wchodzi – cała tabela od nowej strony.
                    page += 1;
                    y = ps.margin_top;
                } else if k < table.rows.len() {
                    table.take = Some(k);
                    let mut rest: Vec<Vec<String>> = if table.header { vec![table.rows[0].clone()] } else { Vec::new() };
                    rest.extend(table.rows[k..].iter().cloned());
                    let mut ct = table.clone();
                    ct.rows = rest;
                    ct.take = None;
                    let h = crate::paint::table_height(&ct, snapshot.w);
                    let mut c = snapshot.clone();
                    c.id = id + 1_000_000_000;
                    c.kind = ObjKind::Table { table: ct };
                    c.page = page + 1;
                    c.y = ps.margin_top;
                    c.h = h;
                    c.cont_of = Some(id);
                    c.locked = true;
                    conts.push(c);
                }
            }
            o.h = crate::paint::table_height(table, o.w);
        } else if y + o.h > bottom + 0.5 && o.h < bottom - ps.margin_top {
            page += 1;
            y = ps.margin_top;
        }
        o.page = page;
        o.y = y;
        doc.min_pages = doc.min_pages.max(page + 1);
    }
    for c in &conts {
        doc.min_pages = doc.min_pages.max(c.page + 1);
    }
    doc.objects.extend(conts);
    doc.objects != before || doc.min_pages != min_before
}

/// Punkt zakotwiczenia obiektu (strona, y) – do przeliczenia `anchor_dy` po przeciągnięciu.
pub fn anchor_ref(doc: &Document, lay: &Layout, id: u64) -> Option<(usize, f32)> {
    let pi = doc.body.paras.iter().position(|p| p.runs.iter().any(|r| r.field == Some(Field::Anchor(id))))?;
    let prev = lay.story_lines(StoryRef::Body).iter().map(|&i| &lay.lines[i]).filter(|l| l.para < pi).last();
    Some(match prev {
        Some(l) => (l.page, l.bottom()),
        None => (0, doc.page.margin_top),
    })
}

/// Cele odsyłaczy (zakładki i podpisy w treści głównej): id → (strona, tekst akapitu).
fn ref_targets(doc: &Document, body_pages: &[usize]) -> HashMap<u64, (u32, String)> {
    let mut out = HashMap::new();
    for (i, p) in doc.body.paras.iter().enumerate() {
        for r in &p.runs {
            if let Some(Field::Bookmark(id) | Field::Seq(id, _)) = r.field {
                let page = body_pages.get(i).copied().unwrap_or(0) as u32 + doc.page.first_number;
                out.insert(id, (page, para_plain(p)));
            }
        }
    }
    out
}

/// Tekst akapitu bez pól (do żywej paginy, odsyłaczy i spisów).
fn para_plain(p: &Paragraph) -> String {
    p.runs.iter().filter(|r| r.field.is_none()).map(|r| r.text.as_str()).collect::<String>().trim().to_string()
}

/// Żywa pagina każdej strony dla poziomów 1–3: pierwszy nagłówek zaczynający się na stronie,
/// a gdy go nie ma – ostatni nagłówek z wcześniejszych stron (jak w słownikach i encyklopediach).
fn running_heads(doc: &Document, body_pages: &[usize], pages: usize) -> Vec<[String; 3]> {
    let mut heads: Vec<(usize, usize, String)> = Vec::new();
    for (i, p) in doc.body.paras.iter().enumerate() {
        if p.style.starts_with("toc") || p.style.starts_with("index") {
            continue;
        }
        let Some(level) = doc.outline_level(p) else { continue };
        let text = para_plain(p);
        if level == 0 || level > 3 || text.is_empty() {
            continue;
        }
        heads.push((body_pages.get(i).copied().unwrap_or(0), level as usize, text));
    }
    let mut out = vec![[String::new(), String::new(), String::new()]; pages];
    let mut last = [String::new(), String::new(), String::new()];
    let mut k = 0;
    for (page, slot) in out.iter_mut().enumerate() {
        let mut first: [Option<String>; 3] = [None, None, None];
        while k < heads.len() && heads[k].0 <= page {
            let (pg, level, text) = &heads[k];
            // Nagłówek wyższego rzędu zastępuje też paginę niższych poziomów.
            for l in level - 1..3 {
                if *pg == page && first[l].is_none() {
                    first[l] = Some(text.clone());
                }
                last[l] = text.clone();
            }
            k += 1;
        }
        for l in 0..3 {
            slot[l] = first[l].clone().unwrap_or_else(|| last[l].clone());
        }
    }
    out
}

/// Skorowidz z haseł indeksu: litery działowe, hasła i podhasła z numerami stron.
pub fn index(doc: &Document, layout: &Layout) -> Vec<Paragraph> {
    use std::collections::BTreeMap;
    // (hasło, podhasło) → strony
    let mut map: BTreeMap<(String, String), (String, String, Vec<u32>)> = BTreeMap::new();
    for (i, p) in doc.body.paras.iter().enumerate() {
        if p.style.starts_with("index") {
            continue;
        }
        for r in &p.runs {
            let Some(Field::IndexMark(id)) = r.field else { continue };
            let Some(e) = doc.index.iter().find(|e| e.id == id) else { continue };
            if e.term.trim().is_empty() {
                continue;
            }
            let page = layout.body_pages.get(i).copied().unwrap_or(0) as u32 + doc.page.first_number;
            let key = (sort_key(&e.term), sort_key(&e.sub));
            let v = map.entry(key).or_insert_with(|| (e.term.trim().to_string(), e.sub.trim().to_string(), Vec::new()));
            if !v.2.contains(&page) {
                v.2.push(page);
            }
        }
    }
    let mut out = Vec::new();
    let mut letter = String::new();
    let mut main = String::new();
    for ((tk, _), (term, sub, mut pages)) in map {
        pages.sort_unstable();
        let first: String = term.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
        if first != letter {
            letter = first.clone();
            out.push(Paragraph { style: "index-letter".into(), fmt: ParaFmt::default(), runs: vec![Run { text: first, ..Default::default() }] });
        }
        if sub.is_empty() {
            main = tk;
            out.push(Paragraph { style: "index1".into(), fmt: ParaFmt::default(), runs: vec![Run { text: format!("{term}\t{}", page_ranges(&pages)), ..Default::default() }] });
        } else {
            if main != tk {
                main = tk;
                out.push(Paragraph { style: "index1".into(), fmt: ParaFmt::default(), runs: vec![Run { text: term.clone(), ..Default::default() }] });
            }
            out.push(Paragraph { style: "index2".into(), fmt: ParaFmt::default(), runs: vec![Run { text: format!("{sub}\t{}", page_ranges(&pages)), ..Default::default() }] });
        }
    }
    out
}

/// Klucz sortowania z polskim porządkiem liter (ą po a, ł po l…).
fn sort_key(s: &str) -> String {
    const ORDER: &str = "0123456789aąbcćdeęfghijklłmnńoópqrsśtuvwxyzźż";
    let mut k = String::new();
    for c in s.trim().to_lowercase().chars() {
        match ORDER.chars().position(|o| o == c) {
            Some(i) => k.push(char::from(b'0' + i as u8)),
            None if c.is_alphanumeric() => k.push(c),
            None => {}
        }
    }
    k
}

/// „3, 4, 5, 9” → „3–5, 9”.
fn page_ranges(p: &[u32]) -> String {
    let mut parts = Vec::new();
    let mut i = 0;
    while i < p.len() {
        let mut j = i;
        while j + 1 < p.len() && p[j + 1] == p[j] + 1 {
            j += 1;
        }
        parts.push(if j >= i + 2 { format!("{}–{}", p[i], p[j]) } else if j == i + 1 { format!("{}, {}", p[i], p[j]) } else { p[i].to_string() });
        i = j + 1;
    }
    parts.join(", ")
}

/// Spis treści z nagłówków (z numerami stron bieżącego składu).
pub fn toc(doc: &Document, layout: &Layout) -> Vec<Paragraph> {
    let mut out = Vec::new();
    for (i, p) in doc.body.paras.iter().enumerate() {
        if p.style.starts_with("toc") {
            continue;
        }
        let Some(level) = doc.outline_level(p) else { continue };
        if level > 3 {
            continue;
        }
        let text = p.text().replace(FIELD_CHAR, "");
        if text.trim().is_empty() {
            continue;
        }
        let page = layout.body_pages.get(i).copied().unwrap_or(0) as u32 + doc.page.first_number;
        out.push(Paragraph { style: format!("toc{level}"), fmt: ParaFmt::default(), runs: vec![Run { text: format!("{}\t{}", text.trim(), page), ..Default::default() }] });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_page_ranges() {
        assert_eq!(page_ranges(&[3, 4, 5, 9]), "3–5, 9");
        assert_eq!(page_ranges(&[1, 2]), "1, 2");
        assert_eq!(page_ranges(&[7]), "7");
    }

    #[test]
    fn index_polish_order() {
        let mut w = vec!["łódź", "lato", "ząb", "żaba", "źródło", "zebra"];
        w.sort_by_key(|s| sort_key(s));
        assert_eq!(w, ["lato", "łódź", "ząb", "zebra", "źródło", "żaba"]);
    }
}
