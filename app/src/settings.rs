//! Ustawienia programu (plik JSON w folderze ustawień – w wersji przenośnej obok programu).

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub theme: i32,
    pub accent: String,
    pub corners: i32,
    pub scale: f32,
    pub dense: bool,
    /// Przezroczystość interfejsu w procentach (0–60).
    pub transparency: i32,
    /// Język interfejsu: "" – jak Windows, "pl", "en".
    pub lang: String,
    /// 0 – mm, 1 – cm, 2 – cale, 3 – punkty.
    pub unit: i32,
    /// Indeks: wył., 1, 2, 5, 10 minut.
    pub autosave: i32,
    /// Otwieranie ostatniego dokumentu na starcie (domyślnie wyłączone – start od ekranu powitalnego).
    /// Nowy klucz, żeby dawne „restore: true” nie omijało ekranu powitalnego.
    #[serde(rename = "restore_last")]
    pub restore: bool,
    pub spell_live: bool,
    pub smart_quotes: bool,
    pub autocorrect: bool,
    pub capitalize: bool,
    pub autolist: bool,
    pub snap: bool,
    pub default_font: String,
    pub default_size: f32,
    pub author: String,
    pub update_check: bool,
    pub update_auto: bool,
    pub qat: Vec<String>,
    /// Dawna pojedyncza „Moja karta” – tylko do przeniesienia do `custom_tabs`.
    #[serde(skip_serializing)]
    pub custom_tab: Vec<String>,
    /// Karty skrótów (0–3): własna nazwa i polecenia z rejestru. `None` – jeszcze nie przeniesiono.
    pub custom_tabs: Option<Vec<CustomTab>>,
    pub recent: Vec<(String, String)>,
    pub recent_fonts: Vec<String>,
    pub zoom: i32,
    pub marks: bool,
    pub guides: bool,
    pub ribbon_collapsed: bool,
    pub last_doc: String,
    pub exact_shapes: bool,
    pub tab: i32,
    pub ai_provider: String,
    /// Klucz API zaszyfrowany DPAPI (base64) – odczyt tylko na tym koncie Windows.
    pub ai_key: String,
    pub ai_model: String,
    pub ai_temp: f32,
    pub ai_base: String,
    pub ruler: bool,
    /// Gęstość ekranu w pikselach na cal do skali 100% (0 – automatycznie z EDID monitora).
    pub screen_ppi: f32,
    /// Linijka pionowa po lewej stronie płótna.
    pub ruler_v: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            theme: 0,
            accent: "#5b8cff".into(),
            corners: 1,
            scale: 1.0,
            dense: false,
            transparency: 0,
            lang: String::new(),
            unit: 0,
            autosave: 2,
            restore: false,
            spell_live: true,
            smart_quotes: true,
            autocorrect: true,
            capitalize: true,
            autolist: true,
            snap: true,
            default_font: "Calibri".into(),
            default_size: 11.0,
            author: String::new(),
            update_check: true,
            update_auto: true,
            qat: default_qat(),
            custom_tab: Vec::new(),
            custom_tabs: None,
            recent: Vec::new(),
            recent_fonts: Vec::new(),
            zoom: 100,
            marks: false,
            guides: true,
            ribbon_collapsed: false,
            last_doc: String::new(),
            exact_shapes: true,
            tab: 0,
            ai_provider: "openrouter".into(),
            ai_key: String::new(),
            ai_model: String::new(),
            ai_temp: 0.7,
            ai_base: String::new(),
            screen_ppi: 0.0,
            ruler_v: false,
            ruler: true,
        }
    }
}

pub fn default_qat() -> Vec<String> {
    ["save", "undo", "redo", "export-pdf", "print"].iter().map(|s| s.to_string()).collect()
}

/// Najwięcej kart skrótów.
pub const MAX_TABS: usize = 3;

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CustomTab {
    /// Pusta – wyświetlamy przetłumaczone „Moja karta”.
    pub name: String,
    pub items: Vec<String>,
}

pub fn default_tab() -> Vec<String> {
    ["text-shape-star", "draw-shape", "image", "chart-column", "columns-2", "export-docx", "export-pdf"].iter().map(|s| s.to_string()).collect()
}

/// Szyfrowanie tajnych danych (klucz API) kontem użytkownika Windows (DPAPI).
pub fn protect(plain: &str) -> String {
    use base64::Engine;
    use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};
    if plain.is_empty() {
        return String::new();
    }
    let mut data = plain.as_bytes().to_vec();
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_mut_ptr() };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if CryptProtectData(&input, windows::core::PCWSTR::null(), None, None, None, 0, &mut out).is_err() {
            return String::new();
        }
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(out.pbData as *mut _)));
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }
}

pub fn unprotect(enc: &str) -> String {
    use base64::Engine;
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};
    let Ok(mut data) = base64::engine::general_purpose::STANDARD.decode(enc.trim()) else { return String::new() };
    if data.is_empty() {
        return String::new();
    }
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_mut_ptr() };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if CryptUnprotectData(&input, None, None, None, None, 0, &mut out).is_err() {
            return String::new();
        }
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(out.pbData as *mut _)));
        String::from_utf8(bytes).unwrap_or_default()
    }
}

fn file() -> std::path::PathBuf {
    opendtp_core::paths::config().join("ustawienia.json")
}

impl Prefs {
    pub fn load() -> Prefs {
        let mut p: Prefs = std::fs::read_to_string(file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        // Dotychczasowa „Moja karta” zostaje jako pierwsza karta skrótów; nowa instalacja startuje bez kart.
        if p.custom_tabs.is_none() {
            let old = std::mem::take(&mut p.custom_tab);
            p.custom_tabs = Some(if old.is_empty() { Vec::new() } else { vec![CustomTab { name: String::new(), items: old }] });
        }
        p
    }

    pub fn save(&self) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let tmp = file().with_extension("tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, file());
            }
        }
    }

    pub fn tabs(&self) -> &[CustomTab] {
        self.custom_tabs.as_deref().unwrap_or(&[])
    }

    pub fn tabs_mut(&mut self) -> &mut Vec<CustomTab> {
        self.custom_tabs.get_or_insert_with(Vec::new)
    }

    pub fn add_recent(&mut self, path: &str) {
        self.recent.retain(|(p, _)| !p.eq_ignore_ascii_case(path));
        self.recent.insert(0, (path.to_string(), opendtp_core::util::today()));
        self.recent.truncate(15);
    }

    pub fn add_recent_font(&mut self, f: &str) {
        self.recent_fonts.retain(|x| x != f);
        self.recent_fonts.insert(0, f.to_string());
        self.recent_fonts.truncate(6);
    }

    pub fn autosave_secs(&self) -> u64 {
        match self.autosave {
            1 => 60,
            2 => 120,
            3 => 300,
            4 => 600,
            _ => 0,
        }
    }
}
