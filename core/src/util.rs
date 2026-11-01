//! Drobiazgi: data i godzina lokalna (pola dokumentu, metadane).

fn local() -> windows::Win32::Foundation::SYSTEMTIME {
    unsafe { windows::Win32::System::SystemInformation::GetLocalTime() }
}

/// Dzisiejsza data w formacie dd.mm.rrrr.
pub fn today() -> String {
    let t = local();
    format!("{:02}.{:02}.{}", t.wDay, t.wMonth, t.wYear)
}

pub fn now_time() -> String {
    let t = local();
    format!("{:02}:{:02}", t.wHour, t.wMinute)
}

/// Data z znacznika ISO 8601 („2026-11-01T12:00:00”) w zapisie „01.11.2026”; pusty – pusty.
pub fn short_date(iso: &str) -> String {
    match (iso.get(0..4), iso.get(5..7), iso.get(8..10)) {
        (Some(y), Some(m), Some(d)) => format!("{d}.{m}.{y}"),
        _ => String::new(),
    }
}

/// Znacznik czasu ISO 8601 (metadane plików).
pub fn iso_now() -> String {
    let t = local();
    format!("{}-{:02}-{:02}T{:02}:{:02}:{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}
