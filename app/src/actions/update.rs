//! Sprawdzanie, pobieranie i instalacja aktualizacji.

use super::*;

pub(super) fn set_update(text: &str, ready: bool, busy: bool) {
    let Some(sh) = crate::shared() else { return };
    let Ok(st) = sh.try_borrow() else { return };
    if let Some(ui) = st.ui.upgrade() {
        let app = ui.global::<App>();
        app.set_update_status(text.into());
        app.set_update_ready(ready);
        app.set_update_busy(busy);
    }
}
pub fn check_update(manual: bool) {
    let Some(sh) = crate::shared() else { return };
    {
        let Ok(st) = sh.try_borrow() else { return };
        if !manual && !st.prefs.update_check {
            return;
        }
        if st.update_busy {
            return;
        }
    }
    set_update(tr("Sprawdzanie aktualizacji…"), false, true);
    std::thread::spawn(move || {
        let res = opendtp_core::update::check();
        let _ = slint::invoke_from_event_loop(move || {
            let Some(sh) = crate::shared() else { return };
            let when = opendtp_core::util::now_time();
            match res {
                Ok(Some(rel)) => {
                    let v = rel.version.clone();
                    let auto = sh.borrow().prefs.update_auto;
                    sh.borrow_mut().update = Some(rel);
                    set_update(&opendtp_core::trf!("Dostępna wersja {v} (masz {}) · sprawdzono o {when}", opendtp_core::version()), true, false);
                    if auto {
                        let mut st = sh.borrow_mut();
                        download_update(&mut st, false);
                    } else {
                        let st = sh.borrow();
                        toast(&st, &opendtp_core::trf!("Jest nowa wersja OpenDTP {v} – Plik → Ustawienia → Zaktualizuj"));
                    }
                }
                Ok(None) => {
                    set_update(&opendtp_core::trf!("Masz najnowszą wersję {} · sprawdzono o {when}", opendtp_core::version()), false, false);
                    if manual {
                        toast(&sh.borrow(), tr("Masz najnowszą wersję"));
                    }
                }
                Err(e) => {
                    set_update(&opendtp_core::trf!("Nie udało się sprawdzić ({}) · {when}", e), false, false);
                    if manual {
                        toast(&sh.borrow(), tr("Nie udało się sprawdzić aktualizacji"));
                    }
                }
            }
        });
    });
}
/// „Zaktualizuj”: pobranie (jeśli trzeba) i instalacja od razu, z ponownym uruchomieniem programu.
pub fn install_update(st: &mut State) {
    if let Some(p) = st.pending_setup.clone() {
        run_setup(st, &p, true);
        return;
    }
    download_update(st, true);
}
pub(super) fn run_setup(st: &mut State, path: &std::path::Path, now: bool) {
    if now && st.ed.dirty {
        let _ = files::save(st, false);
        if st.ed.dirty {
            toast(st, tr("Zapisz dokument przed aktualizacją"));
            return;
        }
    }
    let applied = if opendtp_core::update::installed() { opendtp_core::update::apply(path, now) } else { opendtp_core::update::apply_portable(path) };
    match applied {
        Ok(()) => {
            if now {
                st.pending_close = true;
                files::quit(st);
            }
        }
        Err(e) => set_update(&opendtp_core::trf!("Instalacja nie powiodła się: {}", e), true, false),
    }
}
/// Pobiera wydanie w tle. `now` – zainstaluj od razu; inaczej przy zamknięciu programu.
pub(super) fn download_update(st: &mut State, now: bool) {
    let Some(rel) = st.update.clone() else {
        check_update(true);
        return;
    };
    if st.update_busy {
        return;
    }
    st.update_busy = true;
    set_update(tr("Pobieranie aktualizacji…"), true, true);
    std::thread::spawn(move || {
        let res = opendtp_core::update::download(&rel, |done, total| {
            let pct = if total > 0 { done * 100 / total } else { 0 };
            let _ = slint::invoke_from_event_loop(move || set_update(&opendtp_core::trf!("Pobieranie aktualizacji… {}%", pct), true, true));
        });
        let _ = slint::invoke_from_event_loop(move || {
            let Some(sh) = crate::shared() else { return };
            sh.borrow_mut().update_busy = false;
            match res {
                Ok(path) => {
                    let mut st = sh.borrow_mut();
                    st.pending_setup = Some(path.clone());
                    if now {
                        run_setup(&mut st, &path, true);
                    } else {
                        set_update(&opendtp_core::trf!("Wersja {} pobrana – zainstaluje się przy zamknięciu programu", rel.version), true, false);
                        toast(&st, tr("Aktualizacja pobrana – zainstaluje się po zamknięciu OpenDTP"));
                    }
                }
                Err(e) => set_update(&e, true, false),
            }
        });
    });
}
#[allow(dead_code)]
pub(super) fn later(f: impl FnOnce() + 'static) {
    slint::Timer::single_shot(Duration::ZERO, f);
}
