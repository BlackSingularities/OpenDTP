//! Foldery programu. Wersja przenośna (plik `portable.ini` obok programu) trzyma wszystko
//! w folderze `data` obok pliku .exe; zainstalowana – w %APPDATA% i %LOCALAPPDATA%.

use std::path::PathBuf;

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

pub fn portable() -> bool {
    exe_dir().is_some_and(|d| d.join("portable.ini").exists())
}

fn ensure(p: PathBuf) -> PathBuf {
    let _ = std::fs::create_dir_all(&p);
    p
}

/// Ustawienia, słownik użytkownika, ostatnie pliki.
pub fn config() -> PathBuf {
    if portable() {
        if let Some(d) = exe_dir() {
            return ensure(d.join("data"));
        }
    }
    ensure(dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("OpenDTP"))
}

/// Pamięć podręczna: aktualizacje, kopie odzyskiwania.
pub fn cache() -> PathBuf {
    if portable() {
        if let Some(d) = exe_dir() {
            return ensure(d.join("data").join("cache"));
        }
    }
    ensure(dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("OpenDTP"))
}

pub fn recovery() -> PathBuf {
    ensure(cache().join("recovery"))
}

/// Dodatkowe czcionki: folder `fonts` w ustawieniach (np. dołączone do dokumentów).
pub fn user_font_dirs() -> Vec<PathBuf> {
    let mut v = vec![config().join("fonts")];
    if let Some(d) = exe_dir() {
        v.push(d.join("fonts"));
    }
    v
}
