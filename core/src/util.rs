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

/// Znacznik czasu ISO 8601 (metadane plików).
pub fn iso_now() -> String {
    let t = local();
    format!("{}-{:02}-{:02}T{:02}:{:02}:{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}
