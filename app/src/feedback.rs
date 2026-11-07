//! Zgłoszenia do redzikowski.com (wspólny moduł aplikacji Slint):
//!  - raport o błędzie – awaria z poprzedniego uruchomienia, wysyłana tylko za zgodą
//!    użytkownika (trafia do „Raportów błędów” w panelu),
//!  - zgłoszenie błędu albo sugestia funkcjonalności pisane ręcznie (trafia do
//!    „Sugestii i zgłoszeń”).
//!
//! Okno to `FeedbackWindow` z `ui/feedback.slint`. Adres e-mail jest zawsze opcjonalny.
//! Plik jest taki sam w OpenMP, OpenDTP i PowerDesk – zmieniaj go we wszystkich.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use slint::ComponentHandle;

use crate::FeedbackWindow;

const ENDPOINT: &str = "https://redzikowski.com/api/feedback";
const CRASH_FILE: &str = "feedback-pending-crash.txt";

struct Config {
    slug: &'static str,
    name: &'static str,
    version: String,
    dir: PathBuf,
    polish: fn() -> bool,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Wywołaj raz na początku `main`. `crash_dir` – folder na plik awarii; `capture_panics` = false,
/// gdy program ma już własny zapis awarii i sam przekazuje jej opis do [`open_crash`].
pub fn install(slug: &'static str, name: &'static str, version: impl Into<String>, crash_dir: PathBuf, polish: fn() -> bool, capture_panics: bool) {
    if CONFIG.set(Config { slug, name, version: version.into(), dir: crash_dir, polish }).is_err() || !capture_panics {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(c) = CONFIG.get() {
            let bt = std::backtrace::Backtrace::force_capture();
            let thread = std::thread::current().name().unwrap_or("?").to_string();
            let text = format!("{} {} · thread {thread}\n\n{info}\n\n{bt}", c.name, c.version);
            let _ = std::fs::create_dir_all(&c.dir);
            let _ = std::fs::write(c.dir.join(CRASH_FILE), text);
        }
        previous(info);
    }));
}

/// Awaria zapisana przez [`install`] przy poprzednim uruchomieniu – pokazuje okno z pytaniem o wysłanie.
#[allow(dead_code)] // OpenMP ma własny zapis awarii i woła open_crash
pub fn offer_pending_crash() {
    let Some(c) = CONFIG.get() else { return };
    let path = c.dir.join(CRASH_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let _ = std::fs::remove_file(&path);
    if !text.trim().is_empty() {
        open_crash(text);
    }
}

fn os_name() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

fn body(kind: &str, message: &str, email: &str, include_device: bool, stack: Option<&str>) -> String {
    let c = CONFIG.get();
    let mut m = serde_json::Map::new();
    m.insert("kind".into(), kind.into());
    m.insert("app".into(), c.map(|c| c.slug).unwrap_or("").into());
    m.insert("appName".into(), c.map(|c| c.name).unwrap_or("").into());
    m.insert("appVersion".into(), c.map(|c| c.version.as_str()).unwrap_or("").into());
    m.insert("platform".into(), if cfg!(target_os = "windows") { "windows" } else { "other" }.into());
    m.insert("locale".into(), if c.is_some_and(|c| (c.polish)()) { "pl" } else { "en" }.into());
    m.insert("message".into(), message.trim().into());
    m.insert("email".into(), email.trim().into());
    if include_device {
        m.insert("os".into(), os_name().into());
        m.insert("extra".into(), serde_json::json!({ "cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0) }));
    }
    if let Some(s) = stack {
        m.insert("stack".into(), s.chars().take(60_000).collect::<String>().into());
    }
    serde_json::Value::Object(m).to_string()
}

fn post(json: String, pl: bool) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(10)).timeout(Duration::from_secs(20)).build();
    match agent.post(ENDPOINT).set("Content-Type", "application/json; charset=utf-8").send_string(&json) {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(429, _)) => Err(if pl { "Za dużo zgłoszeń – spróbuj później." } else { "Too many reports – try again later." }.into()),
        Err(ureq::Error::Status(code, _)) => Err(if pl { format!("Serwer odrzucił zgłoszenie (HTTP {code}).") } else { format!("The server rejected the report (HTTP {code}).") }),
        Err(e) => Err(if pl { format!("Brak połączenia z redzikowski.com: {e}") } else { format!("Can't reach redzikowski.com: {e}") }),
    }
}

fn email_ok(email: &str) -> bool {
    let e = email.trim();
    e.is_empty() || (e.contains('@') && e.rsplit('@').next().is_some_and(|d| d.contains('.')) && !e.contains(' '))
}

thread_local! {
    static WINDOW: RefCell<Option<FeedbackWindow>> = const { RefCell::new(None) };
}

/// Okno zwalniamy dopiero po powrocie z jego własnego wywołania zwrotnego.
fn forget_later() {
    let _ = slint::invoke_from_event_loop(|| {
        WINDOW.with(|w| w.borrow_mut().take());
    });
}

/// Formularz „Zgłoś błąd lub sugestię”.
pub fn open_form() {
    show(None);
}

/// Raport o awarii z gotowym opisem (stos wywołań, wersja).
pub fn open_crash(stack: String) {
    show(Some(stack));
}

fn show(crash: Option<String>) {
    let Some(c) = CONFIG.get() else { return };
    let pl = (c.polish)();
    // Okno już otwarte: wysuń je na wierzch zamiast tworzyć drugie.
    let reused = WINDOW.with(|w| {
        let w = w.borrow();
        let Some(win) = w.as_ref() else { return false };
        if crash.is_some() && !win.get_crash_mode() {
            return false;
        }
        let _ = win.show();
        true
    });
    if reused {
        return;
    }
    let Ok(win) = FeedbackWindow::new() else { return };
    win.set_pl(pl);
    win.set_app_name(c.name.into());
    win.set_crash_mode(crash.is_some());
    win.set_crash_text(crash.clone().unwrap_or_default().into());
    let weak = win.as_weak();
    win.on_dismiss(move || {
        if let Some(w) = weak.upgrade() {
            let _ = w.hide();
        }
        forget_later();
    });
    let weak = win.as_weak();
    win.on_send(move || {
        let Some(w) = weak.upgrade() else { return };
        let email = w.get_email().to_string();
        if !email_ok(&email) {
            w.set_error(if pl { "To nie wygląda na adres e-mail." } else { "That doesn't look like an e-mail address." }.into());
            return;
        }
        let message = w.get_message().to_string();
        let json = match &crash {
            Some(stack) => body("crash", &message, &email, true, Some(stack)),
            None => body(if w.get_feature() { "feature" } else { "bug" }, &message, &email, w.get_include_device(), None),
        };
        w.set_busy(true);
        w.set_error("".into());
        let weak = w.as_weak();
        std::thread::spawn(move || {
            let result = post(json, pl);
            let _ = weak.upgrade_in_event_loop(move |w| {
                w.set_busy(false);
                match result {
                    Ok(()) => w.set_sent(true),
                    Err(e) => w.set_error(e.into()),
                }
            });
        });
    });
    win.window().on_close_requested({
        let weak = win.as_weak();
        move || {
            if weak.upgrade().is_some_and(|w| w.get_busy()) {
                return slint::CloseRequestResponse::KeepWindowShown;
            }
            forget_later();
            slint::CloseRequestResponse::HideWindow
        }
    });
    let _ = win.show();
    WINDOW.with(|w| *w.borrow_mut() = Some(win));
}
