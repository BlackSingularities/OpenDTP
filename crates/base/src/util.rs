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

/// Znak Windows-1250 (polskie litery i typografia; reszta górnej połowy jak Latin-1).
pub fn cp1250(b: u8) -> char {
    match b {
        0x80 => '€',
        0x84 => '„',
        0x85 => '…',
        0x8A => 'Š',
        0x8C => 'Ś',
        0x8D => 'Ť',
        0x8E => 'Ž',
        0x8F => 'Ź',
        0x91 => '‘',
        0x92 => '’',
        0x93 => '“',
        0x94 => '”',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        0x9A => 'š',
        0x9C => 'ś',
        0x9D => 'ť',
        0x9E => 'ž',
        0x9F => 'ź',
        0xA3 => 'Ł',
        0xA5 => 'Ą',
        0xAA => 'Ş',
        0xAF => 'Ż',
        0xB3 => 'ł',
        0xB9 => 'ą',
        0xBA => 'ş',
        0xBF => 'ż',
        0xC6 => 'Ć',
        0xCA => 'Ę',
        0xD1 => 'Ń',
        0xD3 => 'Ó',
        0xE6 => 'ć',
        0xEA => 'ę',
        0xF1 => 'ń',
        0xF3 => 'ó',
        b => b as char,
    }
}
