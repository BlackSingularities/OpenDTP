//! Format `.odtp`: archiwum ZIP z `mimetype`, `document.json` (cały model) i obrazami w `assets/`.
//! Obraz linkowany ma w `assets/` tylko podgląd, a pełny plik wczytujemy z dysku (`links`).
//! Zapis idzie do pliku tymczasowego i dopiero potem podmienia oryginał – przerwany zapis nie
//! niszczy dokumentu.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

use crate::model::{Asset, Document, ImageLink};

pub const MIME: &str = "application/x-opendtp";

pub fn to_bytes(doc: &Document) -> Result<Vec<u8>, String> {
    to_bytes_at(doc, None)
}

/// Zapis z folderem docelowym – ścieżki względne łączy liczymy od niego.
pub fn to_bytes_at(doc: &Document, dir: Option<&Path>) -> Result<Vec<u8>, String> {
    let mut owned;
    let doc = match dir {
        Some(d) if !doc.links.is_empty() => {
            owned = doc.clone();
            for l in owned.links.values_mut() {
                l.rel = Path::new(&l.path).strip_prefix(d).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            }
            &owned
        }
        _ => doc,
    };
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        z.start_file("mimetype", stored).map_err(|e| e.to_string())?;
        z.write_all(MIME.as_bytes()).map_err(|e| e.to_string())?;
        z.start_file("document.json", deflate).map_err(|e| e.to_string())?;
        let json = serde_json::to_vec_pretty(doc).map_err(|e| e.to_string())?;
        z.write_all(&json).map_err(|e| e.to_string())?;
        for (id, a) in &doc.assets {
            // Obrazy są już skompresowane. Linkowany – tylko podgląd.
            z.start_file(format!("assets/{id}"), stored).map_err(|e| e.to_string())?;
            if doc.links.contains_key(id) && !doc.missing.contains(id) {
                z.write_all(&preview(a)).map_err(|e| e.to_string())?;
            } else {
                z.write_all(&a.data).map_err(|e| e.to_string())?;
            }
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

pub fn save(doc: &Document, path: &Path) -> Result<(), String> {
    let bytes = to_bytes_at(doc, path.parent())?;
    let tmp = path.with_extension("odtp.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

pub fn from_bytes(bytes: &[u8]) -> Result<Document, String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| crate::lang::tr("To nie jest plik OpenDTP").to_string())?;
    let mut json = String::new();
    z.by_name("document.json").map_err(|_| crate::lang::tr("Plik nie zawiera dokumentu").to_string())?.read_to_string(&mut json).map_err(|e| e.to_string())?;
    let mut doc: Document = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().to_string();
        let Some(id) = name.strip_prefix("assets/") else { continue };
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|e| e.to_string())?;
        if let Some(a) = asset_from(data) {
            doc.assets.insert(id.to_string(), a);
        }
    }
    if doc.body.paras.is_empty() {
        doc.body = Default::default();
    }
    Ok(doc)
}

pub fn load(path: &Path) -> Result<Document, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut doc = from_bytes(&bytes)?;
    refresh_links(&mut doc, path.parent(), true);
    Ok(doc)
}

/// Podgląd obrazu linkowanego zapisywany w dokumencie (najdłuższy bok do 1200 px).
fn preview(a: &Asset) -> Vec<u8> {
    const MAX: u32 = 1200;
    if a.mime == "image/svg+xml" || a.data.len() < 400_000 || (a.width <= MAX && a.height <= MAX) {
        return a.data.to_vec();
    }
    let Ok(img) = image::load_from_memory(&a.data) else { return a.data.to_vec() };
    let small = img.thumbnail(MAX, MAX);
    let mut out = std::io::Cursor::new(Vec::new());
    let fmt = if a.mime == "image/jpeg" { image::ImageFormat::Jpeg } else { image::ImageFormat::Png };
    let ok = if fmt == image::ImageFormat::Jpeg { small.to_rgb8().write_to(&mut out, fmt).is_ok() } else { small.write_to(&mut out, fmt).is_ok() };
    if ok {
        out.into_inner()
    } else {
        a.data.to_vec()
    }
}

/// Czas modyfikacji (sekundy od 1970) i rozmiar pliku.
fn stamp(p: &Path) -> Option<(u64, u64)> {
    let m = std::fs::metadata(p).ok()?;
    let t = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some((t, m.len()))
}

/// Obraz z pliku na dysku jako łącze (obraz i opis łącza).
pub fn link_file(path: &Path) -> Result<(Asset, ImageLink), String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let a = asset_from(data).ok_or_else(|| crate::lang::tr("Nieobsługiwany format obrazu").to_string())?;
    let (modified, size) = stamp(path).unwrap_or((0, 0));
    let full = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let full = full.to_string_lossy().trim_start_matches(r"\\?\").to_string();
    let link = ImageLink { path: full, rel: String::new(), modified, size, width: a.width, height: a.height };
    Ok((a, link))
}

/// Plik łącza: pełna ścieżka, a gdy jej nie ma – ścieżka względna od folderu dokumentu.
pub fn link_path(l: &ImageLink, dir: Option<&Path>) -> Option<std::path::PathBuf> {
    let full = Path::new(&l.path);
    if full.is_file() {
        return Some(full.to_path_buf());
    }
    let rel = dir?.join(&l.rel);
    (!l.rel.is_empty() && rel.is_file()).then_some(rel)
}

/// Sprawdza pliki łączy: wczytuje zmienione (albo wszystkie przy `force`) i oznacza brakujące.
/// Zwraca klucze obrazów, których wygląd mógł się zmienić.
pub fn refresh_links(doc: &mut Document, dir: Option<&Path>, force: bool) -> Vec<String> {
    let mut changed = Vec::new();
    let keys: Vec<String> = doc.links.keys().cloned().collect();
    for k in keys {
        let l = doc.links[&k].clone();
        match link_path(&l, dir) {
            Some(p) => {
                let st = stamp(&p).unwrap_or((0, 0));
                let was_missing = doc.missing.remove(&k);
                if !force && !was_missing && st == (l.modified, l.size) {
                    continue;
                }
                match link_file(&p) {
                    Ok((a, nl)) => {
                        // Bez zmian w treści (np. tylko otwarcie dokumentu) – nie zgłaszamy.
                        let same = doc.assets.get(&k).is_some_and(|old| old.data == a.data);
                        doc.assets.insert(k.clone(), a);
                        doc.links.insert(k.clone(), ImageLink { rel: l.rel.clone(), ..nl });
                        if !same {
                            changed.push(k);
                        }
                    }
                    // Plik w trakcie zapisu albo uszkodzony – spróbujemy przy następnym sprawdzeniu.
                    Err(_) => {}
                }
            }
            None => {
                if doc.missing.insert(k.clone()) {
                    changed.push(k);
                }
            }
        }
    }
    changed
}

/// Pakowanie: dokument i kopie wszystkich obrazów linkowanych w jednym folderze
/// (`<folder>/<nazwa>.odtp` i `<folder>/obrazy/`). Zwraca ścieżkę zapisanego dokumentu.
pub fn package(doc: &Document, dir: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    let img_dir = dir.join("obrazy");
    std::fs::create_dir_all(&img_dir).map_err(|e| e.to_string())?;
    let mut d = doc.clone();
    let mut used = std::collections::HashSet::new();
    for (k, l) in d.links.iter_mut() {
        let file = Path::new(&l.path).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| k.clone());
        let mut target = file.clone();
        let mut n = 2;
        while !used.insert(target.to_lowercase()) {
            let p = Path::new(&file);
            let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let ext = p.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default();
            target = format!("{stem}-{n}{ext}");
            n += 1;
        }
        let dest = img_dir.join(&target);
        // Brakujący plik – zapisujemy to, co mamy (podgląd).
        match std::fs::copy(&l.path, &dest) {
            Ok(_) => {}
            Err(_) => {
                let a = doc.assets.get(k).ok_or_else(|| format!("{k}"))?;
                std::fs::write(&dest, a.data.as_slice()).map_err(|e| e.to_string())?;
            }
        }
        l.path = dest.to_string_lossy().to_string();
        l.rel = format!("obrazy/{target}");
    }
    d.missing.clear();
    let out = dir.join(format!("{name}.odtp"));
    save(&d, &out)?;
    Ok(out)
}

/// Obraz z bajtów pliku (rozpoznaje format i wymiary).
pub fn asset_from(data: Vec<u8>) -> Option<Asset> {
    // SVG: grafika wektorowa – wymiary z drzewa SVG.
    let head = String::from_utf8_lossy(&data[..data.len().min(2048)]).to_lowercase();
    if head.contains("<svg") {
        let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).ok()?;
        let s = tree.size();
        return Some(Asset { mime: "image/svg+xml".into(), data: Arc::new(data), width: s.width().round().max(1.0) as u32, height: s.height().round().max(1.0) as u32 });
    }
    let fmt = image::guess_format(&data).ok()?;
    let mime = match fmt {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Gif => "image/gif",
        image::ImageFormat::WebP => "image/webp",
        image::ImageFormat::Bmp => "image/bmp",
        _ => return None,
    };
    let (w, h) = image::ImageReader::new(std::io::Cursor::new(&data)).with_guessed_format().ok()?.into_dimensions().ok()?;
    Some(Asset { mime: mime.into(), data: Arc::new(data), width: w, height: h })
}

pub fn ext_of(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpeg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}
