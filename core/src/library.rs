//! Biblioteka elementów: fragmenty tekstu i obiekty (z obrazami) zapisane do ponownego użycia
//! w dowolnym dokumencie. Każdy element to mały dokument `.odtp` w folderze `library` ustawień.

use std::path::PathBuf;

use crate::model::*;

pub fn dir() -> PathBuf {
    let d = crate::paths::config().join("library");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Element biblioteki: nazwa (z nazwy pliku) i ścieżka.
pub struct Item {
    pub name: String,
    pub path: PathBuf,
    /// Zawiera obiekty (inaczej – sam tekst).
    pub objects: bool,
}

pub fn list() -> Vec<Item> {
    let mut v: Vec<Item> = std::fs::read_dir(dir())
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "odtp"))
                .map(|p| {
                    let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    // Rodzaj zapisany w nazwie pliku: „… [obiekty].odtp”.
                    let objects = name.ends_with(" [obiekty]");
                    Item { name: name.trim_end_matches(" [obiekty]").to_string(), path: p, objects }
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|i| i.name.to_lowercase());
    v
}

/// Nazwa pliku bez znaków niedozwolonych w Windows.
fn file_name(name: &str, objects: bool) -> String {
    let clean: String = name.chars().map(|c| if r#"\/:*?"<>|"#.contains(c) || c.is_control() { '_' } else { c }).collect();
    let clean = clean.trim().trim_end_matches('.').to_string();
    let clean = if clean.is_empty() { "Element".to_string() } else { clean };
    format!("{clean}{}.odtp", if objects { " [obiekty]" } else { "" })
}

/// Zapis fragmentu tekstu (akapity ze stylami użytymi w nich).
pub fn save_text(name: &str, src: &Document, paras: Vec<Paragraph>) -> Result<PathBuf, String> {
    let mut d = Document::new();
    d.styles = src.styles.clone();
    d.body = Story { paras };
    // Pola odwołujące się do treści dokumentu źródłowego nie mają sensu w innym dokumencie.
    for p in d.body.paras.iter_mut() {
        p.runs.retain(|r| !matches!(r.field, Some(Field::Anchor(_) | Field::Footnote(_) | Field::Bookmark(_) | Field::IndexMark(_) | Field::Ref(..))));
    }
    let path = dir().join(file_name(name, false));
    crate::io::odtp::save(&d, &path)?;
    Ok(path)
}

/// Zapis obiektów (położenie względem lewego górnego rogu grupy) z ich obrazami.
pub fn save_objects(name: &str, src: &Document, ids: &[u64]) -> Result<PathBuf, String> {
    let mut d = Document::new();
    let objs: Vec<Object> = src.objects.iter().filter(|o| ids.contains(&o.id) && o.cont_of.is_none()).cloned().collect();
    if objs.is_empty() {
        return Err(crate::lang::tr("Zaznacz obiekty do zapisania").to_string());
    }
    let x0 = objs.iter().map(|o| o.x).fold(f32::INFINITY, f32::min);
    let y0 = objs.iter().map(|o| o.y).fold(f32::INFINITY, f32::min);
    for mut o in objs {
        o.x -= x0;
        o.y -= y0;
        o.page = 0;
        if let ObjKind::Text { next, .. } = &mut o.kind {
            if next.is_some_and(|n| !ids.contains(&n)) {
                *next = None;
            }
        }
        if let ObjKind::Image { asset, .. } = &o.kind {
            if let Some(a) = src.assets.get(asset) {
                d.assets.insert(asset.clone(), a.clone());
            }
            if let Some(l) = src.links.get(asset) {
                d.links.insert(asset.clone(), l.clone());
            }
        }
        d.objects.push(o);
    }
    d.styles = src.styles.clone();
    let path = dir().join(file_name(name, true));
    crate::io::odtp::save(&d, &path)?;
    Ok(path)
}

pub fn load(path: &std::path::Path) -> Result<Document, String> {
    crate::io::odtp::load(path)
}

pub fn remove(path: &std::path::Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|e| e.to_string())
}
