//! Czcionki systemu: lista rodzin, wybór kroju (z pogrubieniem i kursywą syntetyczną, gdy rodzina
//! ich nie ma), dane plików, metryki i zastępstwa dla znaków spoza kroju.
//!
//! Wczytane kroje żyją do końca programu (dane plików są „wyciekane” do `'static`) – dzięki temu
//! `ttf_parser::Face` i `rustybuzz::Face` można trzymać w pamięci bez ponownego parsowania.

use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::RwLock;

/// Indeks wczytanego kroju.
pub type FontId = u32;

pub struct Loaded {
    pub family: String,
    pub data: &'static [u8],
    pub index: u32,
    pub face: ttf_parser::Face<'static>,
    pub hb: rustybuzz::Face<'static>,
    pub upem: f32,
    pub ascent: f32,
    pub descent: f32,
    pub gap: f32,
    pub underline_pos: f32,
    pub underline_thick: f32,
    pub strike_pos: f32,
    pub strike_thick: f32,
    pub bold: bool,
    pub italic: bool,
}

impl Loaded {
    pub fn has(&self, c: char) -> bool {
        self.face.glyph_index(c).is_some()
    }
}

/// Wybrany krój i to, czego mu brakuje (syntetyczne pogrubienie lub pochylenie).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pick {
    pub id: FontId,
    pub fake_bold: bool,
    pub fake_italic: bool,
}

struct Registry {
    db: fontdb::Database,
    families: Vec<String>,
    loaded: Vec<&'static Loaded>,
    by_face: HashMap<fontdb::ID, FontId>,
    picks: HashMap<(String, bool, bool), Pick>,
    fallback: Vec<FontId>,
}

static REG: OnceLock<RwLock<Registry>> = OnceLock::new();

const FALLBACKS: &[&str] = &["Segoe UI", "Segoe UI Symbol", "Segoe UI Emoji", "Cambria Math", "Nirmala UI", "Microsoft YaHei", "Malgun Gothic", "Yu Gothic", "Arial", "Ebrima", "Leelawadee UI", "Gadugi", "Javanese Text"];
const DEFAULT: &str = "Calibri";

fn reg() -> &'static RwLock<Registry> {
    REG.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        if let Some(dir) = dirs::data_local_dir() {
            // Czcionki zainstalowane tylko dla użytkownika.
            db.load_fonts_dir(dir.join("Microsoft").join("Windows").join("Fonts"));
        }
        for d in crate::paths::user_font_dirs() {
            db.load_fonts_dir(d);
        }
        let mut fam: Vec<String> = db.faces().filter_map(|f| f.families.first().map(|(n, _)| n.clone())).collect();
        fam.sort_by_key(|a| a.to_lowercase());
        fam.dedup();
        RwLock::new(Registry { db, families: fam, loaded: Vec::new(), by_face: HashMap::new(), picks: HashMap::new(), fallback: Vec::new() })
    })
}

/// Wczytuje bazę czcionek (np. w tle przy starcie, zanim pojawi się dokument).
pub fn warm_up() {
    let _ = reg();
    let _ = pick(DEFAULT, false, false);
    let _ = fallbacks();
}

pub fn families() -> Vec<String> {
    reg().read().families.clone()
}

pub fn has_family(name: &str) -> bool {
    reg().read().families.iter().any(|f| f.eq_ignore_ascii_case(name))
}

fn load(r: &mut Registry, face: fontdb::ID) -> Option<FontId> {
    if let Some(id) = r.by_face.get(&face) {
        return Some(*id);
    }
    let info = r.db.face(face)?;
    let family = info.families.first().map(|(n, _)| n.clone()).unwrap_or_default();
    let bold = info.weight.0 >= 600;
    let italic = info.style != fontdb::Style::Normal;
    let (bytes, index) = r.db.with_face_data(face, |d, i| (d.to_vec(), i))?;
    let data: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    let f = ttf_parser::Face::parse(data, index).ok()?;
    let hb = rustybuzz::Face::from_slice(data, index)?;
    let upem = f.units_per_em() as f32;
    let (ascent, descent, gap) = match f.tables().os2 {
        Some(os2) if os2.use_typographic_metrics() => (os2.typographic_ascender() as f32, os2.typographic_descender() as f32, os2.typographic_line_gap() as f32),
        Some(os2) => (os2.windows_ascender() as f32, -(os2.windows_descender() as f32).abs(), 0.0),
        None => (f.ascender() as f32, f.descender() as f32, f.line_gap() as f32),
    };
    let ul = f.underline_metrics();
    let st = f.strikeout_metrics();
    let loaded = Loaded {
        family,
        data,
        index,
        upem,
        ascent,
        descent: descent.min(0.0),
        gap: gap.max(0.0),
        underline_pos: ul.map(|m| m.position as f32).unwrap_or(-upem * 0.1),
        underline_thick: ul.map(|m| m.thickness as f32).unwrap_or(upem * 0.05).max(upem * 0.03),
        strike_pos: st.map(|m| m.position as f32).unwrap_or(upem * 0.3),
        strike_thick: st.map(|m| m.thickness as f32).unwrap_or(upem * 0.05).max(upem * 0.03),
        bold,
        italic,
        face: f,
        hb,
    };
    let id = r.loaded.len() as FontId;
    r.loaded.push(Box::leak(Box::new(loaded)));
    r.by_face.insert(face, id);
    Some(id)
}

fn query(r: &Registry, family: &str, bold: bool, italic: bool) -> Option<fontdb::ID> {
    let q = fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight: if bold { fontdb::Weight::BOLD } else { fontdb::Weight::NORMAL },
        stretch: fontdb::Stretch::Normal,
        style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
    };
    r.db.query(&q)
}

/// Krój dla rodziny i stylu. Nieznana rodzina – czcionka domyślna.
pub fn pick(family: &str, bold: bool, italic: bool) -> Pick {
    let key = (family.to_string(), bold, italic);
    if let Some(p) = reg().read().picks.get(&key) {
        return *p;
    }
    let mut r = reg().write();
    let face = query(&r, family, bold, italic).or_else(|| query(&r, DEFAULT, bold, italic)).or_else(|| query(&r, "Arial", bold, italic)).or_else(|| r.db.faces().next().map(|f| f.id));
    let p = match face.and_then(|f| load(&mut r, f)) {
        Some(id) => {
            let l = r.loaded[id as usize];
            Pick { id, fake_bold: bold && !l.bold, fake_italic: italic && !l.italic }
        }
        None => Pick { id: 0, fake_bold: false, fake_italic: false },
    };
    r.picks.insert(key, p);
    p
}

pub fn get(id: FontId) -> Option<&'static Loaded> {
    reg().read().loaded.get(id as usize).copied()
}

fn fallbacks() -> Vec<FontId> {
    {
        let r = reg().read();
        if !r.fallback.is_empty() {
            return r.fallback.clone();
        }
    }
    let mut r = reg().write();
    let mut out = Vec::new();
    for f in FALLBACKS {
        if let Some(face) = query(&r, f, false, false) {
            if let Some(id) = load(&mut r, face) {
                out.push(id);
            }
        }
    }
    r.fallback = out.clone();
    out
}

/// Krój zastępczy zawierający znak `c` (brak – `None`).
pub fn fallback_for(c: char) -> Option<FontId> {
    fallbacks().into_iter().find(|&id| get(id).is_some_and(|l| l.has(c)))
}

/// Czy w systemie jest dokładnie ta rodzina (inaczej podpowiadamy w interfejsie).
pub fn resolve_name(family: &str) -> String {
    let r = reg().read();
    r.families.iter().find(|f| f.eq_ignore_ascii_case(family)).cloned().unwrap_or_else(|| family.to_string())
}
