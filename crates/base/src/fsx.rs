//! Bezpieczny zapis plików: najpierw plik tymczasowy obok docelowego, zapis na dysk (`sync_all`),
//! dopiero potem podmiana. Gdy coś pójdzie nie tak, stary plik zostaje nietknięty.
//!
//! Na wzór `safe_save` z WordCraft i `retry_rename` z DesignCraft (MIT/Apache-2.0): w Windows
//! antywirus, indeksowanie albo OneDrive potrafią na chwilę zablokować świeży plik, więc podmianę
//! ponawiamy kilka razy, zanim zgłosimy błąd.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Zapisuje `bytes` do `path` w całości albo wcale.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomic_with(path, |f| f.write_all(bytes))
}

/// Jak [`write_atomic`], ale treść pisze `fill` (np. strumieniowo). Panika w `fill` też zostawia
/// stary plik w spokoju i wraca jako błąd.
pub fn write_atomic_with(path: &Path, fill: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    if path.file_name().is_none() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "brak nazwy pliku"));
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    let (tmp, mut file) = match temporary(path) {
        Ok(t) => t,
        // Folder nie pozwala tworzyć plików pomocniczych: nowy plik zapisujemy wprost,
        // istniejącego nie ruszamy (nadpisanie mogłoby go zostawić uciętego).
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied && !path.exists() => return write_direct(path, fill),
        Err(e) => return Err(e),
    };
    let filled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fill(&mut file)))
        .unwrap_or_else(|_| Err(io::Error::other("zapis przerwany nieoczekiwanym błędem")))
        .and_then(|_| file.sync_all());
    drop(file);
    if let Err(e) = filled.and_then(|_| replace(&tmp, path)) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

fn temporary(dest: &Path) -> io::Result<(PathBuf, File)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = dest.file_name().unwrap_or_default().to_string_lossy();
    for _ in 0..64 {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let tmp = dest.with_file_name(format!(".~{name}.{}-{n}.tmp", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(f) => return Ok((tmp, f)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "nie udało się utworzyć pliku tymczasowego"))
}

fn write_direct(path: &Path, fill: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fill(&mut f)))
        .unwrap_or_else(|_| Err(io::Error::other("zapis przerwany nieoczekiwanym błędem")))
        .and_then(|_| f.sync_all());
    drop(f);
    if r.is_err() {
        let _ = fs::remove_file(path);
    }
    r
}

/// Podmiana z ponawianiem, gdy plik docelowy jest chwilowo zajęty (kody Windows 5, 32, 33).
fn replace(tmp: &Path, dest: &Path) -> io::Result<()> {
    let mut wait = Duration::from_millis(15);
    let mut last = None;
    for _ in 0..8 {
        match fs::rename(tmp, dest) {
            Ok(()) => return Ok(()),
            Err(e) if busy(&e) => {
                last = Some(e);
                std::thread::sleep(wait);
                wait = (wait * 2).min(Duration::from_millis(400));
            }
            Err(e) => return Err(e),
        }
    }
    Err(explain(last.unwrap_or_else(|| io::Error::other("podmiana pliku nie powiodła się"))))
}

fn busy(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

fn explain(e: io::Error) -> io::Error {
    io::Error::new(e.kind(), format!("{e}. Plik jest otwarty w innym programie albo folder jest tylko do odczytu – zamknij go albo zapisz pod inną nazwą."))
}

/// Usuwa pozostałości po przerwanych zapisach w folderze (pliki `.~*.tmp` starsze niż doba).
pub fn sweep_temporaries(dir: &Path) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !(name.starts_with(".~") && name.ends_with(".tmp")) {
            continue;
        }
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|a| a > Duration::from_secs(86_400));
        if old {
            let _ = fs::remove_file(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("opendtp-fsx-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn names(d: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        v.sort();
        v
    }

    #[test]
    fn replaces_and_leaves_no_temporary() {
        let d = dir("ok");
        let p = d.join("a.odtp");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        assert_eq!(names(&d), ["a.odtp"]);
    }

    #[test]
    fn failed_fill_keeps_old_file() {
        let d = dir("fail");
        let p = d.join("a.docx");
        write_atomic(&p, b"old").unwrap();
        let r = write_atomic_with(&p, |f| {
            f.write_all(b"half")?;
            Err(io::Error::other("boom"))
        });
        assert!(r.is_err());
        assert_eq!(fs::read(&p).unwrap(), b"old");
        assert_eq!(names(&d), ["a.docx"]);
    }

    #[test]
    fn panicking_fill_keeps_old_file() {
        let d = dir("panic");
        let p = d.join("a.pdf");
        write_atomic(&p, b"old").unwrap();
        let r = write_atomic_with(&p, |_| panic!("x"));
        assert!(r.is_err());
        assert_eq!(fs::read(&p).unwrap(), b"old");
        assert_eq!(names(&d), ["a.pdf"]);
    }
}
