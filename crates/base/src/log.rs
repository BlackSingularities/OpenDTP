//! Dziennik programu: `logs/opendtp.log` w folderze pamięci podręcznej (po 1 MB przechodzi
//! w `opendtp.1.log`) oraz zapis każdej awarii (panic) z miejscem i przebiegiem wywołań.
//!
//! Na wzór dziennika WordCraft: tekst, który komunikat awarii cytuje w odwrotnych apostrofach,
//! bywa treścią dokumentu, więc w dzienniku zastępujemy go wielokropkiem.

use std::io::Write;
use std::path::PathBuf;

use parking_lot::Mutex;

const LIMIT: u64 = 1024 * 1024;

static LOCK: Mutex<()> = Mutex::new(());

pub fn file() -> PathBuf {
    let dir = crate::paths::cache().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("opendtp.log")
}

fn write(level: &str, msg: &str) {
    let _g = LOCK.lock();
    let path = file();
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > LIMIT) {
        let _ = std::fs::rename(&path, path.with_file_name("opendtp.1.log"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {} [{level}] {msg}", crate::util::iso_now(), crate::version());
    }
}

pub fn info(msg: &str) {
    write("INFO", msg);
}

pub fn warn(msg: &str) {
    write("WARN", msg);
}

pub fn error(msg: &str) {
    write("ERROR", msg);
}

/// Zastępuje fragmenty w odwrotnych apostrofach wielokropkiem (mogą zawierać tekst dokumentu).
pub fn redact_quoted(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    for (i, part) in message.split('`').enumerate() {
        if i % 2 == 1 {
            out.push_str("`…");
        } else {
            if i > 0 {
                out.push('`');
            }
            out.push_str(part);
        }
    }
    out
}

/// Każda awaria trafia do dziennika (miejsce, komunikat bez treści dokumentu, przebieg wywołań).
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().map(|l| l.to_string()).unwrap_or_else(|| "nieznane miejsce".into());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(brak komunikatu)".into());
        let trace = std::backtrace::Backtrace::force_capture();
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        error(&format!("awaria w wątku {thread} ({location}): {}\n{trace}", redact_quoted(&message)));
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_quoted_text() {
        assert_eq!(redact_quoted("byte index 5 is not a char boundary; it is inside `ż` of `Zażółć`"), "byte index 5 is not a char boundary; it is inside `…` of `…`");
        assert_eq!(redact_quoted("no quotes"), "no quotes");
        assert_eq!(redact_quoted("open `secret"), "open `…");
    }
}
