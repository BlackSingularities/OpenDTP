//! Powiększenie i przewijanie z płynną animacją.

use super::*;

/// Powiększenie bez odświeżania interfejsu: punkt pod `anchor` (płótno) zostaje w miejscu.
pub(super) fn zoom_core(st: &mut State, z: f32, anchor: Option<(f32, f32)>) -> bool {
    let z = z.clamp(0.25, 4.0);
    let old = st.view.zoom;
    if (z - old).abs() < 1e-4 {
        return false;
    }
    // Punkt pod wskaźnikiem (albo środek widoku) zostaje w miejscu.
    let (ax, ay) = anchor.unwrap_or((st.view.vx + st.view.vw / 2.0, st.view.vy + st.view.vh / 2.0));
    let (page, px, py) = st.view.to_page(ax, ay);
    let (sx, sy) = (ax - st.view.vx, ay - st.view.vy);
    st.view.zoom = z;
    st.view.place(&st.ed);
    let (nx, ny) = st.view.to_canvas(page, px, py);
    let vx = (nx - sx).max(0.0);
    let vy = (ny - sy).max(0.0);
    st.view.vx = vx;
    st.view.vy = vy;
    if let Some(ui) = st.ui.upgrade() {
        let app = ui.global::<App>();
        app.set_content_w(st.view.content_w);
        app.set_content_h(st.view.content_h);
        app.set_view_x(-vx);
        app.set_view_y(-vy);
        app.set_zoom((z * 100.0).round() as i32);
    }
    st.prefs.zoom = (z * 100.0).round() as i32;
    true
}
/// Płynne powiększanie i przewijanie: cel ustawiają przyciski i kółko, widok dojeżdża do niego
/// co klatkę z łagodnym hamowaniem. W trakcie powiększania strony są skalowanymi obrazami,
/// ostro przerysowujemy je raz – na końcu.
#[derive(Default)]
pub(super) struct Anim {
    /// (od, do, punkt zaczepienia względem widoku, start).
    zoom: Option<(f32, f32, Option<(f32, f32)>, std::time::Instant)>,
    /// Docelowe przesunięcie widoku (x, y) w pikselach płótna.
    scroll: Option<(f32, f32)>,
}

thread_local! {
    static ANIM: std::cell::RefCell<Anim> = std::cell::RefCell::new(Anim::default());
    static ANIM_TIMER: slint::Timer = slint::Timer::default();
}
pub(super) const ZOOM_MS: f32 = 180.0;
/// Docelowe powiększenie (w trakcie animacji – to, do którego zmierzamy).
pub fn zoom_target(st: &State) -> f32 {
    ANIM.with(|a| a.borrow().zoom.map(|z| z.1)).unwrap_or(st.view.zoom)
}
/// Płynne powiększenie do `z`; `anchor` – punkt płótna, który ma zostać pod wskaźnikiem.
pub fn zoom_smooth(st: &mut State, z: f32, anchor: Option<(f32, f32)>) {
    let z = z.clamp(0.25, 4.0);
    if (z - st.view.zoom).abs() < 1e-4 {
        return;
    }
    let rel = anchor.map(|(x, y)| (x - st.view.vx, y - st.view.vy));
    ANIM.with(|a| {
        let mut a = a.borrow_mut();
        a.scroll = None;
        a.zoom = Some((st.view.zoom, z, rel, std::time::Instant::now()));
    });
    st.view.animating = true;
    start_anim();
}
/// Płynne przewinięcie o (dx, dy) pikseli; kolejne obroty kółka przesuwają cel dalej.
pub fn scroll_smooth(st: &mut State, dx: f32, dy: f32) {
    let max_x = (st.view.content_w - st.view.vw).max(0.0);
    let max_y = (st.view.content_h - st.view.vh).max(0.0);
    ANIM.with(|a| {
        let mut a = a.borrow_mut();
        let (tx, ty) = a.scroll.unwrap_or((st.view.vx, st.view.vy));
        a.scroll = Some(((tx + dx).clamp(0.0, max_x), (ty + dy).clamp(0.0, max_y)));
    });
    start_anim();
}
pub(super) fn start_anim() {
    ANIM_TIMER.with(|t| {
        if !t.running() {
            t.start(slint::TimerMode::Repeated, Duration::from_millis(16), anim_tick);
        }
    });
}
pub(super) fn anim_tick() {
    let Some(sh) = crate::shared() else { return };
    let Ok(mut st) = sh.try_borrow_mut() else { return };
    let st = &mut *st;
    let (zoom, scroll) = ANIM.with(|a| {
        let a = a.borrow();
        (a.zoom, a.scroll)
    });
    if let Some((from, to, rel, t0)) = zoom {
        let t = (t0.elapsed().as_secs_f32() * 1000.0 / ZOOM_MS).min(1.0);
        let e = 1.0 - (1.0 - t).powi(3);
        // Mnożnikowo – każdy krok powiększenia wygląda tak samo szybko.
        let z = from * (to / from).powf(e);
        let anchor = rel.map(|(x, y)| (st.view.vx + x, st.view.vy + y));
        if t >= 1.0 {
            ANIM.with(|a| a.borrow_mut().zoom = None);
            st.view.animating = false;
            zoom_core(st, to, anchor);
            st.view.invalidate();
            refresh(st);
        } else if zoom_core(st, z, anchor) {
            st.view.render(&st.ed);
        }
    }
    if let Some((tx, ty)) = scroll {
        let (cx, cy) = (st.view.vx, st.view.vy);
        let (mut nx, mut ny) = (cx + (tx - cx) * 0.28, cy + (ty - cy) * 0.28);
        if (tx - nx).abs() < 0.6 && (ty - ny).abs() < 0.6 {
            nx = tx;
            ny = ty;
            ANIM.with(|a| a.borrow_mut().scroll = None);
        }
        st.view.vx = nx;
        st.view.vy = ny;
        if let Some(ui) = st.ui.upgrade() {
            let app = ui.global::<App>();
            app.set_view_x(-nx);
            app.set_view_y(-ny);
        }
        st.view.render(&st.ed);
    }
    if ANIM.with(|a| {
        let a = a.borrow();
        a.zoom.is_none() && a.scroll.is_none()
    }) {
        ANIM_TIMER.with(|t| t.stop());
    }
}
