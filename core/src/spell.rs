//! Sprawdzanie pisowni (słowniki Hunspell: polski SJP.PL i angielski) – wczytywane w tle przy
//! starcie, z pamięcią wyników, słownikiem użytkownika i listą „ignoruj” na czas sesji.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use parking_lot::{Mutex, RwLock};
use spellbook::Dictionary;

static PL_AFF: &str = include_str!("../dict/pl_PL.aff");
static PL_DIC: &str = include_str!("../dict/pl_PL.dic");
static EN_AFF: &str = include_str!("../dict/en_US.aff");
static EN_DIC: &str = include_str!("../dict/en_US.dic");

struct Dicts {
    pl: Option<Dictionary>,
    en: Option<Dictionary>,
}

static DICTS: OnceLock<Dicts> = OnceLock::new();
static CACHE: OnceLock<Mutex<HashMap<(String, bool), bool>>> = OnceLock::new();
static USER: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
static IGNORE: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();

fn user_file() -> std::path::PathBuf {
    crate::paths::config().join("slownik-uzytkownika.txt")
}

fn user() -> &'static RwLock<HashSet<String>> {
    USER.get_or_init(|| {
        let set = std::fs::read_to_string(user_file()).map(|s| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default();
        RwLock::new(set)
    })
}

fn ignore() -> &'static RwLock<HashSet<String>> {
    IGNORE.get_or_init(|| RwLock::new(HashSet::new()))
}

fn cache() -> &'static Mutex<HashMap<(String, bool), bool>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Wczytuje słowniki (kilka sekund – wywołuj w osobnym wątku).
pub fn load() {
    DICTS.get_or_init(|| Dicts { pl: Dictionary::new(PL_AFF, PL_DIC).ok(), en: Dictionary::new(EN_AFF, EN_DIC).ok() });
    let _ = user();
}

pub fn ready() -> bool {
    DICTS.get().is_some()
}

fn dict(lang: &str) -> Option<&'static Dictionary> {
    let d = DICTS.get()?;
    if lang.starts_with("en") {
        d.en.as_ref()
    } else {
        d.pl.as_ref()
    }
}

fn clean(word: &str) -> &str {
    word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '\'' && c != '’')
}

/// Czy wyraz jest błędny. Przed wczytaniem słowników – nigdy.
pub fn misspelled(word: &str, lang: &str) -> bool {
    let w = clean(word);
    if w.chars().count() < 2 || w.chars().any(|c| c.is_ascii_digit()) || w.chars().all(|c| c.is_uppercase()) {
        return false;
    }
    let Some(d) = dict(lang) else { return false };
    if user().read().contains(w) || ignore().read().contains(w) {
        return false;
    }
    let en = lang.starts_with("en");
    let key = (w.to_string(), en);
    if let Some(v) = cache().lock().get(&key) {
        return *v;
    }
    let ok = d.check(w) || (w.contains('-') && w.split('-').all(|p| p.is_empty() || d.check(p)));
    let mut c = cache().lock();
    if c.len() > 200_000 {
        c.clear();
    }
    c.insert(key, !ok);
    !ok
}

pub fn suggest(word: &str, lang: &str) -> Vec<String> {
    let Some(d) = dict(lang) else { return Vec::new() };
    let mut out = Vec::new();
    d.suggest(clean(word), &mut out);
    out.truncate(8);
    out
}

pub fn add_to_user(word: &str) {
    let w = clean(word).to_string();
    let mut u = user().write();
    u.insert(w);
    let mut list: Vec<&String> = u.iter().collect();
    list.sort();
    let _ = std::fs::write(user_file(), list.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    cache().lock().clear();
}

pub fn ignore_word(word: &str) {
    ignore().write().insert(clean(word).to_string());
    cache().lock().clear();
}

pub fn user_words() -> Vec<String> {
    let mut v: Vec<String> = user().read().iter().cloned().collect();
    v.sort();
    v
}

pub fn remove_user_word(word: &str) {
    let mut u = user().write();
    u.remove(word);
    let mut list: Vec<&String> = u.iter().collect();
    list.sort();
    let _ = std::fs::write(user_file(), list.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    cache().lock().clear();
}
