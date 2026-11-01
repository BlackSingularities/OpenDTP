//! Format `.odtp`: archiwum ZIP z `mimetype`, `document.json` (cały model) i obrazami w `assets/`.
//! Zapis idzie do pliku tymczasowego i dopiero potem podmienia oryginał – przerwany zapis nie
//! niszczy dokumentu.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

use crate::model::{Asset, Document};

pub const MIME: &str = "application/x-opendtp";

pub fn to_bytes(doc: &Document) -> Result<Vec<u8>, String> {
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
            // Obrazy są już skompresowane.
            z.start_file(format!("assets/{id}"), stored).map_err(|e| e.to_string())?;
            z.write_all(&a.data).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

pub fn save(doc: &Document, path: &Path) -> Result<(), String> {
    let bytes = to_bytes(doc)?;
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
    from_bytes(&bytes)
}

/// Obraz z bajtów pliku (rozpoznaje format i wymiary).
pub fn asset_from(data: Vec<u8>) -> Option<Asset> {
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
        _ => "bin",
    }
}
