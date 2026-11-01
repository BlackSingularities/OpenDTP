//! Geometria kształtów: krzywe Béziera, spłaszczanie do wielokątów, przedziały wierszy w kształcie,
//! operacje logiczne, gotowe kształty i kontur obrazu z przezroczystości.

use crate::model::{Contour, Node, Shape};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

pub fn pt(x: f32, y: f32) -> Pt {
    Pt { x, y }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }
}

/// Segment ścieżki w współrzędnych bezwzględnych.
#[derive(Clone, Copy, Debug)]
pub enum Seg {
    Line(Pt, Pt),
    Cubic(Pt, Pt, Pt, Pt),
}

/// Kontur w punktach strony: węzły przeliczone z 0..1 na prostokąt `r`.
pub fn contour_segments(c: &Contour, r: Rect) -> Vec<Seg> {
    let map = |x: f32, y: f32| pt(r.x + x * r.w, r.y + y * r.h);
    let n = c.nodes.len();
    let mut out = Vec::new();
    if n < 2 {
        return out;
    }
    let count = if c.closed { n } else { n - 1 };
    for i in 0..count {
        let a = &c.nodes[i];
        let b = &c.nodes[(i + 1) % n];
        let p0 = map(a.x, a.y);
        let p3 = map(b.x, b.y);
        if a.cout.is_none() && b.cin.is_none() {
            out.push(Seg::Line(p0, p3));
        } else {
            let p1 = a.cout.map(|(x, y)| map(x, y)).unwrap_or(p0);
            let p2 = b.cin.map(|(x, y)| map(x, y)).unwrap_or(p3);
            out.push(Seg::Cubic(p0, p1, p2, p3));
        }
    }
    out
}

pub fn cubic_at(p0: Pt, p1: Pt, p2: Pt, p3: Pt, t: f32) -> Pt {
    let u = 1.0 - t;
    let a = u * u * u;
    let b = 3.0 * u * u * t;
    let c = 3.0 * u * t * t;
    let d = t * t * t;
    pt(a * p0.x + b * p1.x + c * p2.x + d * p3.x, a * p0.y + b * p1.y + c * p2.y + d * p3.y)
}

/// Spłaszcza kontur do wielokąta (krzywe na odcinki o długości ok. `tol`).
pub fn flatten(c: &Contour, r: Rect, tol: f32) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::new();
    for s in contour_segments(c, r) {
        match s {
            Seg::Line(a, b) => {
                if out.is_empty() {
                    out.push(a);
                }
                out.push(b);
            }
            Seg::Cubic(p0, p1, p2, p3) => {
                if out.is_empty() {
                    out.push(p0);
                }
                let len = dist(p0, p1) + dist(p1, p2) + dist(p2, p3);
                let steps = ((len / tol.max(0.25)).ceil() as usize).clamp(4, 400);
                for i in 1..=steps {
                    out.push(cubic_at(p0, p1, p2, p3, i as f32 / steps as f32));
                }
            }
        }
    }
    if c.closed && out.len() > 1 && dist(out[0], *out.last().unwrap()) < 1e-4 {
        out.pop();
    }
    out
}

pub fn dist(a: Pt, b: Pt) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Wszystkie kontury kształtu jako wielokąty w punktach strony.
pub fn shape_polys(s: &Shape, r: Rect) -> Vec<Vec<Pt>> {
    s.contours.iter().filter(|c| c.closed && c.nodes.len() >= 3).map(|c| flatten(c, r, 1.5)).collect()
}

/// Przedziały poziome wnętrza wielokątów (reguła parzystości) na wysokości `y`.
pub fn scan(polys: &[Vec<Pt>], y: f32) -> Vec<(f32, f32)> {
    let mut xs = Vec::new();
    for p in polys {
        let n = p.len();
        for i in 0..n {
            let a = p[i];
            let b = p[(i + 1) % n];
            if (a.y <= y && b.y > y) || (b.y <= y && a.y > y) {
                let t = (y - a.y) / (b.y - a.y);
                xs.push(a.x + t * (b.x - a.x));
            }
        }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.chunks(2).filter(|c| c.len() == 2 && c[1] > c[0]).map(|c| (c[0], c[1])).collect()
}

/// Część wspólna dwóch zbiorów przedziałów (posortowanych, rozłącznych).
pub fn intersect(a: &[(f32, f32)], b: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        let lo = a[i].0.max(b[j].0);
        let hi = a[i].1.min(b[j].1);
        if hi > lo {
            out.push((lo, hi));
        }
        if a[i].1 < b[j].1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// Suma przedziałów.
pub fn union(mut v: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut out: Vec<(f32, f32)> = Vec::new();
    for (a, b) in v {
        if let Some(l) = out.last_mut() {
            if a <= l.1 {
                l.1 = l.1.max(b);
                continue;
            }
        }
        out.push((a, b));
    }
    out
}

/// `a` bez `b`.
pub fn subtract(a: &[(f32, f32)], b: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    for &(lo, hi) in a {
        let mut cur = lo;
        for &(bl, bh) in b {
            if bh <= cur || bl >= hi {
                continue;
            }
            if bl > cur {
                out.push((cur, bl));
            }
            cur = cur.max(bh);
        }
        if hi > cur {
            out.push((cur, hi));
        }
    }
    out
}

/// Wysokości, na których trzeba próbkować pas `[y0, y1]`: brzegi, wierzchołki w środku i co `step`.
fn samples(polys: &[Vec<Pt>], y0: f32, y1: f32, step: f32) -> Vec<f32> {
    let mut ys = vec![y0 + 0.01, y1 - 0.01];
    let mut y = y0 + step;
    while y < y1 {
        ys.push(y);
        y += step;
    }
    for p in polys {
        for v in p {
            if v.y > y0 && v.y < y1 {
                ys.push(v.y - 0.01);
                ys.push(v.y + 0.01);
            }
        }
    }
    ys
}

/// Przedziały, które na całej wysokości pasa leżą wewnątrz kształtu.
pub fn band_inside(polys: &[Vec<Pt>], y0: f32, y1: f32) -> Vec<(f32, f32)> {
    let mut acc: Option<Vec<(f32, f32)>> = None;
    for y in samples(polys, y0, y1, 2.0) {
        let s = scan(polys, y);
        acc = Some(match acc {
            None => s,
            Some(a) => intersect(&a, &s),
        });
        if acc.as_ref().is_some_and(|a| a.is_empty()) {
            break;
        }
    }
    acc.unwrap_or_default()
}

/// Przedziały, które gdziekolwiek w pasie dotykają kształtu (do wycięcia przy opływaniu).
pub fn band_touch(polys: &[Vec<Pt>], y0: f32, y1: f32) -> Vec<(f32, f32)> {
    let mut all = Vec::new();
    for y in samples(polys, y0, y1, 2.0) {
        all.extend(scan(polys, y));
    }
    union(all)
}

pub fn bounds(polys: &[Vec<Pt>]) -> Option<Rect> {
    let mut it = polys.iter().flatten();
    let first = it.next()?;
    let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x, first.y);
    for p in it {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
}

pub fn point_in(polys: &[Vec<Pt>], x: f32, y: f32) -> bool {
    scan(polys, y).iter().any(|&(a, b)| x >= a && x <= b)
}

/// Długości kolejnych odcinków łamanej (do tekstu na ścieżce).
pub fn polyline_lengths(p: &[Pt]) -> Vec<f32> {
    let mut acc = vec![0.0];
    for w in p.windows(2) {
        let l = acc.last().unwrap() + dist(w[0], w[1]);
        acc.push(l);
    }
    acc
}

/// Punkt i kąt (radiany) na łamanej w odległości `s` od początku.
pub fn along(p: &[Pt], lens: &[f32], s: f32) -> Option<(Pt, f32)> {
    if p.len() < 2 {
        return None;
    }
    let total = *lens.last()?;
    if s < 0.0 || s > total {
        return None;
    }
    let i = match lens.binary_search_by(|v| v.partial_cmp(&s).unwrap()) {
        Ok(i) => i.min(p.len() - 2),
        Err(i) => i.saturating_sub(1).min(p.len() - 2),
    };
    let seg = (lens[i + 1] - lens[i]).max(1e-6);
    let t = (s - lens[i]) / seg;
    let a = p[i];
    let b = p[i + 1];
    Some((pt(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t), (b.y - a.y).atan2(b.x - a.x)))
}

// ---------------------------------------------------------------- gotowe kształty

fn kappa() -> f32 {
    0.552_284_8
}

pub fn ellipse() -> Shape {
    let k = kappa() * 0.5;
    let n = |x: f32, y: f32, cin: (f32, f32), cout: (f32, f32)| Node { x, y, cin: Some(cin), cout: Some(cout), smooth: true };
    Shape {
        contours: vec![Contour {
            nodes: vec![
                n(0.5, 0.0, (0.5 - k, 0.0), (0.5 + k, 0.0)),
                n(1.0, 0.5, (1.0, 0.5 - k), (1.0, 0.5 + k)),
                n(0.5, 1.0, (0.5 + k, 1.0), (0.5 - k, 1.0)),
                n(0.0, 0.5, (0.0, 0.5 + k), (0.0, 0.5 - k)),
            ],
            closed: true,
        }],
    }
}

pub fn regular(sides: usize, rot: f32) -> Shape {
    let pts: Vec<(f32, f32)> = (0..sides)
        .map(|i| {
            let a = rot + i as f32 * std::f32::consts::TAU / sides as f32 - std::f32::consts::FRAC_PI_2;
            (0.5 + 0.5 * a.cos(), 0.5 + 0.5 * a.sin())
        })
        .collect();
    normalize(Shape::polygon(&pts))
}

pub fn star(points: usize, inner: f32) -> Shape {
    let pts: Vec<(f32, f32)> = (0..points * 2)
        .map(|i| {
            let r = if i % 2 == 0 { 0.5 } else { 0.5 * inner };
            let a = i as f32 * std::f32::consts::PI / points as f32 - std::f32::consts::FRAC_PI_2;
            (0.5 + r * a.cos(), 0.5 + r * a.sin())
        })
        .collect();
    normalize(Shape::polygon(&pts))
}

pub fn heart() -> Shape {
    let c = |x: f32, y: f32, cin: Option<(f32, f32)>, cout: Option<(f32, f32)>| Node { x, y, cin, cout, smooth: false };
    Shape {
        contours: vec![Contour {
            nodes: vec![
                c(0.5, 0.25, Some((0.42, 0.05)), Some((0.58, 0.05))),
                c(1.0, 0.32, Some((1.0, 0.05)), Some((1.0, 0.6))),
                c(0.5, 1.0, Some((0.8, 0.8)), Some((0.2, 0.8))),
                c(0.0, 0.32, Some((0.0, 0.6)), Some((0.0, 0.05))),
            ],
            closed: true,
        }],
    }
}

/// Fala: górna i dolna krawędź falista.
pub fn wave() -> Shape {
    let s = |x: f32, y: f32, cin: (f32, f32), cout: (f32, f32)| Node { x, y, cin: Some(cin), cout: Some(cout), smooth: true };
    Shape {
        contours: vec![Contour {
            nodes: vec![
                Node { x: 0.0, y: 0.1, cin: None, cout: Some((0.17, -0.05)), smooth: false },
                s(0.5, 0.1, (0.33, 0.25), (0.67, -0.05)),
                Node { x: 1.0, y: 0.1, cin: Some((0.83, 0.25)), cout: None, smooth: false },
                Node { x: 1.0, y: 0.9, cin: None, cout: Some((0.83, 1.05)), smooth: false },
                s(0.5, 0.9, (0.67, 0.75), (0.33, 1.05)),
                Node { x: 0.0, y: 0.9, cin: Some((0.17, 0.75)), cout: None, smooth: false },
            ],
            closed: true,
        }],
    }
}

/// Łuk (połowa pierścienia).
pub fn arch() -> Shape {
    let k = kappa();
    let n = |x: f32, y: f32, cin: Option<(f32, f32)>, cout: Option<(f32, f32)>| Node { x, y, cin, cout, smooth: false };
    Shape {
        contours: vec![Contour {
            nodes: vec![
                n(0.0, 1.0, None, Some((0.0, 1.0 - k))),
                n(0.5, 0.0, Some((0.5 - 0.5 * k, 0.0)), Some((0.5 + 0.5 * k, 0.0))),
                n(1.0, 1.0, Some((1.0, 1.0 - k)), None),
                n(0.75, 1.0, None, Some((0.75, 1.0 - 0.5 * k))),
                n(0.5, 0.5, Some((0.5 + 0.25 * k, 0.5)), Some((0.5 - 0.25 * k, 0.5))),
                n(0.25, 1.0, Some((0.25, 1.0 - 0.5 * k)), None),
            ],
            closed: true,
        }],
    }
}

pub fn rounded_rect(r: f32) -> Shape {
    let k = r * (1.0 - kappa());
    let n = |x: f32, y: f32, cin: Option<(f32, f32)>, cout: Option<(f32, f32)>| Node { x, y, cin, cout, smooth: false };
    Shape {
        contours: vec![Contour {
            nodes: vec![
                n(r, 0.0, Some((k, 0.0)), None),
                n(1.0 - r, 0.0, None, Some((1.0 - k, 0.0))),
                n(1.0, r, Some((1.0, k)), None),
                n(1.0, 1.0 - r, None, Some((1.0, 1.0 - k))),
                n(1.0 - r, 1.0, Some((1.0 - k, 1.0)), None),
                n(r, 1.0, None, Some((k, 1.0))),
                n(0.0, 1.0 - r, Some((0.0, 1.0 - k)), None),
                n(0.0, r, None, Some((0.0, k))),
            ],
            closed: true,
        }],
    }
}

pub fn line() -> Shape {
    Shape { contours: vec![Contour { nodes: vec![Node::corner(0.0, 0.5), Node::corner(1.0, 0.5)], closed: false }] }
}

pub fn arc_path() -> Shape {
    let k = kappa();
    Shape {
        contours: vec![Contour {
            nodes: vec![
                Node { x: 0.0, y: 1.0, cin: None, cout: Some((0.0, 1.0 - k)), smooth: false },
                Node { x: 0.5, y: 0.0, cin: Some((0.5 - 0.5 * k, 0.0)), cout: Some((0.5 + 0.5 * k, 0.0)), smooth: true },
                Node { x: 1.0, y: 1.0, cin: Some((1.0, 1.0 - k)), cout: None, smooth: false },
            ],
            closed: false,
        }],
    }
}

/// Gotowe kształty: identyfikator, nazwa i budowa.
pub fn preset(id: &str) -> Option<Shape> {
    Some(match id {
        "rect" => Shape::rect(),
        "rounded" => rounded_rect(0.12),
        "ellipse" => ellipse(),
        "triangle" => Shape::polygon(&[(0.5, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        "diamond" => Shape::polygon(&[(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)]),
        "pentagon" => regular(5, 0.0),
        "hexagon" => regular(6, std::f32::consts::FRAC_PI_6),
        "octagon" => regular(8, std::f32::consts::PI / 8.0),
        "star" => star(5, 0.45),
        "star8" => star(8, 0.7),
        "heart" => heart(),
        "wave" => wave(),
        "arch" => arch(),
        "parallelogram" => Shape::polygon(&[(0.2, 0.0), (1.0, 0.0), (0.8, 1.0), (0.0, 1.0)]),
        "trapezoid" => Shape::polygon(&[(0.2, 0.0), (0.8, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        "arrow" => Shape::polygon(&[(0.0, 0.3), (0.6, 0.3), (0.6, 0.0), (1.0, 0.5), (0.6, 1.0), (0.6, 0.7), (0.0, 0.7)]),
        "chevron" => Shape::polygon(&[(0.0, 0.0), (0.7, 0.0), (1.0, 0.5), (0.7, 1.0), (0.0, 1.0), (0.3, 0.5)]),
        "cross" => Shape::polygon(&[(0.33, 0.0), (0.67, 0.0), (0.67, 0.33), (1.0, 0.33), (1.0, 0.67), (0.67, 0.67), (0.67, 1.0), (0.33, 1.0), (0.33, 0.67), (0.0, 0.67), (0.0, 0.33), (0.33, 0.33)]),
        "speech" => Shape::polygon(&[(0.0, 0.0), (1.0, 0.0), (1.0, 0.75), (0.45, 0.75), (0.25, 1.0), (0.28, 0.75), (0.0, 0.75)]),
        "ring" => {
            let mut s = ellipse();
            let mut inner = ellipse().contours.remove(0);
            for n in inner.nodes.iter_mut() {
                let f = |v: f32| 0.5 + (v - 0.5) * 0.55;
                n.x = f(n.x);
                n.y = f(n.y);
                n.cin = n.cin.map(|(x, y)| (f(x), f(y)));
                n.cout = n.cout.map(|(x, y)| (f(x), f(y)));
            }
            s.contours.push(inner);
            s
        }
        "line" => line(),
        "arc" => arc_path(),
        _ => return None,
    })
}

pub const PRESETS: &[&str] = &[
    "rect", "rounded", "ellipse", "triangle", "diamond", "pentagon", "hexagon", "octagon", "star", "star8", "heart", "wave", "arch", "parallelogram", "trapezoid", "arrow", "chevron", "cross", "speech", "ring",
];

/// Dopasowuje kształt do pola 0..1 (po obrotach wielokąty foremne nie wypełniają ramki).
pub fn normalize(mut s: Shape) -> Shape {
    let mut x0 = f32::MAX;
    let mut y0 = f32::MAX;
    let mut x1 = f32::MIN;
    let mut y1 = f32::MIN;
    for c in &s.contours {
        for n in &c.nodes {
            x0 = x0.min(n.x);
            y0 = y0.min(n.y);
            x1 = x1.max(n.x);
            y1 = y1.max(n.y);
        }
    }
    let w = (x1 - x0).max(1e-6);
    let h = (y1 - y0).max(1e-6);
    let f = |(x, y): (f32, f32)| ((x - x0) / w, (y - y0) / h);
    for c in s.contours.iter_mut() {
        for n in c.nodes.iter_mut() {
            let (x, y) = f((n.x, n.y));
            n.x = x;
            n.y = y;
            n.cin = n.cin.map(f);
            n.cout = n.cout.map(f);
        }
    }
    s
}

// ---------------------------------------------------------------- operacje logiczne

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Difference,
    Intersect,
    Xor,
}

/// Operacja logiczna na dwóch kształtach w punktach strony; wynik – kształt w 0..1 nowej ramki
/// i ta ramka. Krzywe są spłaszczane (wynik ma węzły narożne).
pub fn boolean(a: &Shape, ra: Rect, b: &Shape, rb: Rect, op: BoolOp) -> Option<(Shape, Rect)> {
    use i_overlay::core::fill_rule::FillRule;
    use i_overlay::core::overlay_rule::OverlayRule;
    use i_overlay::float::single::SingleFloatOverlay;
    let conv = |polys: Vec<Vec<Pt>>| -> Vec<Vec<[f64; 2]>> { polys.into_iter().map(|p| p.into_iter().map(|q| [q.x as f64, q.y as f64]).collect()).collect() };
    let pa = conv(shape_polys(a, ra));
    let pb = conv(shape_polys(b, rb));
    let rule = match op {
        BoolOp::Union => OverlayRule::Union,
        BoolOp::Difference => OverlayRule::Difference,
        BoolOp::Intersect => OverlayRule::Intersect,
        BoolOp::Xor => OverlayRule::Xor,
    };
    let res = pa.overlay(&pb, rule, FillRule::EvenOdd);
    let polys: Vec<Vec<Pt>> = res.into_iter().flatten().map(|c| c.into_iter().map(|p| pt(p[0] as f32, p[1] as f32)).collect()).collect();
    let r = bounds(&polys)?;
    if r.w < 1.0 || r.h < 1.0 {
        return None;
    }
    let contours = polys
        .into_iter()
        .map(|p| Contour { nodes: p.into_iter().map(|q| Node::corner((q.x - r.x) / r.w, (q.y - r.y) / r.h)).collect(), closed: true })
        .collect();
    Some((Shape { contours }, r))
}

// ---------------------------------------------------------------- kontur obrazu

/// Kontur obrazu wykryty z przezroczystości (albo z jasnego tła, gdy obraz jest nieprzezroczysty):
/// dla każdego wiersza skrajne nieprzezroczyste piksele, potem uproszczenie. Wynik w 0..1.
pub fn image_contour(rgba: &[u8], w: u32, h: u32) -> Option<Shape> {
    if w == 0 || h == 0 {
        return None;
    }
    let px = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        (rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3])
    };
    let has_alpha = (0..h).step_by((h / 64).max(1) as usize).any(|y| (0..w).step_by((w / 64).max(1) as usize).any(|x| px(x, y).3 < 250));
    // Kolor tła z narożników (dla obrazów bez przezroczystości).
    let corners = [px(0, 0), px(w - 1, 0), px(0, h - 1), px(w - 1, h - 1)];
    let bg = corners[0];
    let solid_bg = corners.iter().all(|c| (c.0 as i32 - bg.0 as i32).abs() < 16 && (c.1 as i32 - bg.1 as i32).abs() < 16 && (c.2 as i32 - bg.2 as i32).abs() < 16);
    if !has_alpha && !solid_bg {
        return None;
    }
    let solid = |x: u32, y: u32| {
        let c = px(x, y);
        if has_alpha {
            c.3 > 40
        } else {
            (c.0 as i32 - bg.0 as i32).abs() + (c.1 as i32 - bg.1 as i32).abs() + (c.2 as i32 - bg.2 as i32).abs() > 48
        }
    };
    let rows = 96.min(h);
    let mut left = Vec::new();
    let mut right = Vec::new();
    for r in 0..rows {
        let y0 = r * h / rows;
        let y1 = ((r + 1) * h / rows).max(y0 + 1).min(h);
        let mut lo = u32::MAX;
        let mut hi = 0u32;
        for y in y0..y1 {
            let step = (w / 400).max(1);
            let mut x = 0;
            while x < w {
                if solid(x, y) {
                    lo = lo.min(x);
                    break;
                }
                x += step;
            }
            let mut x = w as i64 - 1;
            while x >= 0 {
                if solid(x as u32, y) {
                    hi = hi.max(x as u32);
                    break;
                }
                x -= step as i64;
            }
        }
        if lo <= hi && lo != u32::MAX {
            let fy0 = y0 as f32 / h as f32;
            let fy1 = y1 as f32 / h as f32;
            left.push((lo as f32 / w as f32, fy0));
            left.push((lo as f32 / w as f32, fy1));
            right.push(((hi + 1) as f32 / w as f32, fy0));
            right.push(((hi + 1) as f32 / w as f32, fy1));
        }
    }
    if left.len() < 2 {
        return None;
    }
    let mut pts: Vec<(f32, f32)> = right.clone();
    pts.extend(left.iter().rev());
    let pts = simplify(&pts, 0.006);
    if pts.len() < 3 {
        return None;
    }
    Some(Shape::polygon(&pts))
}

/// Upraszczanie łamanej zamkniętej (Douglas–Peucker).
pub fn simplify(p: &[(f32, f32)], eps: f32) -> Vec<(f32, f32)> {
    fn rec(p: &[(f32, f32)], eps: f32, out: &mut Vec<(f32, f32)>) {
        if p.len() < 3 {
            out.extend_from_slice(&p[..p.len().saturating_sub(1)]);
            return;
        }
        let (a, b) = (p[0], p[p.len() - 1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-9);
        let (mut idx, mut best) = (0, 0.0);
        for (i, q) in p.iter().enumerate().skip(1).take(p.len() - 2) {
            let d = ((q.0 - a.0) * dy - (q.1 - a.1) * dx).abs() / len;
            if d > best {
                best = d;
                idx = i;
            }
        }
        if best > eps {
            rec(&p[..=idx], eps, out);
            rec(&p[idx..], eps, out);
        } else {
            out.push(a);
        }
    }
    let mut out = Vec::new();
    let mut closed = p.to_vec();
    if let Some(f) = p.first() {
        closed.push(*f);
    }
    rec(&closed, eps, &mut out);
    out
}

/// Węzeł najbliżej punktu (indeks konturu, indeks węzła, odległość).
pub fn nearest_node(s: &Shape, r: Rect, x: f32, y: f32) -> Option<(usize, usize, f32)> {
    let mut best: Option<(usize, usize, f32)> = None;
    for (ci, c) in s.contours.iter().enumerate() {
        for (ni, n) in c.nodes.iter().enumerate() {
            let d = dist(pt(r.x + n.x * r.w, r.y + n.y * r.h), pt(x, y));
            if best.is_none_or(|b| d < b.2) {
                best = Some((ci, ni, d));
            }
        }
    }
    best
}

/// Najbliższy segment do punktu: (kontur, indeks segmentu, parametr t, odległość).
pub fn nearest_segment(s: &Shape, r: Rect, x: f32, y: f32) -> Option<(usize, usize, f32, f32)> {
    let mut best: Option<(usize, usize, f32, f32)> = None;
    let q = pt(x, y);
    for (ci, c) in s.contours.iter().enumerate() {
        for (si, seg) in contour_segments(c, r).iter().enumerate() {
            for k in 0..=40 {
                let t = k as f32 / 40.0;
                let p = match *seg {
                    Seg::Line(a, b) => pt(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t),
                    Seg::Cubic(a, b, c2, d) => cubic_at(a, b, c2, d, t),
                };
                let d = dist(p, q);
                if best.is_none_or(|bb| d < bb.3) {
                    best = Some((ci, si, t, d));
                }
            }
        }
    }
    best
}

/// Wstawia węzeł w segment `si` konturu `ci` w miejscu `t` (z podziałem krzywej de Casteljau).
pub fn insert_node(s: &mut Shape, ci: usize, si: usize, t: f32) {
    let c = &mut s.contours[ci];
    let n = c.nodes.len();
    if n < 2 {
        return;
    }
    let a = c.nodes[si];
    let b = c.nodes[(si + 1) % n];
    let p0 = pt(a.x, a.y);
    let p3 = pt(b.x, b.y);
    if a.cout.is_none() && b.cin.is_none() {
        let m = Node::corner(p0.x + (p3.x - p0.x) * t, p0.y + (p3.y - p0.y) * t);
        c.nodes.insert(si + 1, m);
        return;
    }
    let p1 = a.cout.map(|(x, y)| pt(x, y)).unwrap_or(p0);
    let p2 = b.cin.map(|(x, y)| pt(x, y)).unwrap_or(p3);
    let lerp = |u: Pt, v: Pt| pt(u.x + (v.x - u.x) * t, u.y + (v.y - u.y) * t);
    let q0 = lerp(p0, p1);
    let q1 = lerp(p1, p2);
    let q2 = lerp(p2, p3);
    let r0 = lerp(q0, q1);
    let r1 = lerp(q1, q2);
    let m = lerp(r0, r1);
    c.nodes[si].cout = Some((q0.x, q0.y));
    let bi = (si + 1) % n;
    c.nodes[bi].cin = Some((q2.x, q2.y));
    c.nodes.insert(si + 1, Node { x: m.x, y: m.y, cin: Some((r0.x, r0.y)), cout: Some((r1.x, r1.y)), smooth: true });
}

/// Zamienia węzeł w gładki (z uchwytami wzdłuż kierunku sąsiadów) albo w narożny.
pub fn toggle_smooth(s: &mut Shape, ci: usize, ni: usize) {
    let c = &mut s.contours[ci];
    let n = c.nodes.len();
    if n < 2 {
        return;
    }
    let node = c.nodes[ni];
    if node.cin.is_some() || node.cout.is_some() {
        c.nodes[ni].cin = None;
        c.nodes[ni].cout = None;
        c.nodes[ni].smooth = false;
        return;
    }
    let prev = c.nodes[(ni + n - 1) % n];
    let next = c.nodes[(ni + 1) % n];
    let (dx, dy) = (next.x - prev.x, next.y - prev.y);
    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
    let lp = dist(pt(prev.x, prev.y), pt(node.x, node.y)) / 3.0;
    let ln = dist(pt(next.x, next.y), pt(node.x, node.y)) / 3.0;
    let (ux, uy) = (dx / len, dy / len);
    c.nodes[ni].cin = Some((node.x - ux * lp, node.y - uy * lp));
    c.nodes[ni].cout = Some((node.x + ux * ln, node.y + uy * ln));
    c.nodes[ni].smooth = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_rect() {
        let s = Shape::rect();
        let p = shape_polys(&s, Rect::new(10.0, 10.0, 100.0, 50.0));
        let iv = band_inside(&p, 20.0, 30.0);
        assert_eq!(iv.len(), 1);
        assert!((iv[0].0 - 10.0).abs() < 0.01 && (iv[0].1 - 110.0).abs() < 0.01);
    }

    #[test]
    fn ring_has_hole() {
        let s = preset("ring").unwrap();
        let p = shape_polys(&s, Rect::new(0.0, 0.0, 100.0, 100.0));
        let iv = band_inside(&p, 48.0, 52.0);
        assert_eq!(iv.len(), 2);
    }

    #[test]
    fn subtract_mid() {
        assert_eq!(subtract(&[(0.0, 10.0)], &[(3.0, 5.0)]), vec![(0.0, 3.0), (5.0, 10.0)]);
    }

    #[test]
    fn union_shapes() {
        let (s, r) = boolean(&Shape::rect(), Rect::new(0.0, 0.0, 10.0, 10.0), &Shape::rect(), Rect::new(5.0, 5.0, 10.0, 10.0), BoolOp::Union).unwrap();
        assert!((r.w - 15.0).abs() < 0.01);
        assert_eq!(s.contours.len(), 1);
    }
}
