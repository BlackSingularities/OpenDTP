//! Model dokumentu OpenDTP: strony, style, historie tekstu (treść główna, nagłówki, stopki, ramki)
//! i obiekty na stronach (kształty tekstu, obrazy, kształty, wykresy, tekst na ścieżce).
//!
//! Jednostką długości jest punkt typograficzny (1 pt = 1/72 cala). Formatowanie znaków i akapitów
//! zapisujemy jako nadpisania (`Option`) nad stylem akapitu – zmiana stylu zmienia cały tekst,
//! który go używa, a bezpośrednie formatowanie zostaje.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

pub const MM: f32 = 72.0 / 25.4;

/// Kolor RGBA zapisywany w pliku jako `#rrggbb` albo `#rrggbbaa`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const NONE: Color = Color { r: 0, g: 0, b: 0, a: 0 };

    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 255 }
    }

    pub fn with_alpha(self, a: u8) -> Color {
        Color { a, ..self }
    }

    pub fn hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// Bez `#` i bez kanału alfa – tak jak w DOCX.
    pub fn hex6(&self) -> String {
        format!("{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim().trim_start_matches('#');
        let h = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        match s.len() {
            6 => Some(Color::rgb(h(0)?, h(2)?, h(4)?)),
            8 => Some(Color { r: h(0)?, g: h(2)?, b: h(4)?, a: h(6)? }),
            3 => {
                let c = |i: usize| u8::from_str_radix(s.get(i..i + 1)?, 16).ok().map(|v| v * 17);
                Some(Color::rgb(c(0)?, c(1)?, c(2)?))
            }
            _ => None,
        }
    }

    pub fn is_none(&self) -> bool {
        self.a == 0
    }
}

impl Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
        let s = String::deserialize(d)?;
        Color::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("zły kolor: {s}")))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Underline {
    #[default]
    None,
    Single,
    Double,
    Dotted,
    Wave,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Script {
    #[default]
    Normal,
    Super,
    Sub,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Caps {
    #[default]
    None,
    All,
    Small,
}

/// Nadpisania formatowania znaków (puste pole – wartość ze stylu).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharFmt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<Underline>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Wyróżnienie tła tekstu; przezroczysty – brak.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<Script>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<Caps>,
    /// Światło między znakami w punktach (może być ujemne).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking: Option<f32>,
    /// Szerokość znaków w procentach (skalowanie poziome).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
    /// Przesunięcie linii bazowej w punktach (dodatnie – w górę).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// Ligatury (domyślnie włączone).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ligatures: Option<bool>,
    /// Obrys tekstu (kontur) w punktach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<f32>,
    /// Kerning z czcionki (domyślnie włączony).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kerning: Option<bool>,
    /// Funkcje OpenType oddzielone przecinkami, np. „onum,tnum,ss01”.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<String>,
}

impl CharFmt {
    /// Nakłada `o` na siebie (pola ustawione w `o` wygrywają).
    pub fn merge(&mut self, o: &CharFmt) {
        macro_rules! m { ($($f:ident),*) => { $( if o.$f.is_some() { self.$f = o.$f.clone(); } )* } }
        m!(font, size, bold, italic, underline, strike, color, highlight, script, caps, tracking, scale, baseline, lang, link, ligatures, outline, kerning, features);
    }

    pub fn is_empty(&self) -> bool {
        *self == CharFmt::default()
    }
}

/// Pełne, rozstrzygnięte formatowanie znaków.
#[derive(Clone, Debug, PartialEq)]
pub struct CharProps {
    pub font: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: Underline,
    pub strike: bool,
    pub color: Color,
    pub highlight: Color,
    pub script: Script,
    pub caps: Caps,
    pub tracking: f32,
    pub scale: f32,
    pub baseline: f32,
    pub lang: String,
    pub link: Option<String>,
    pub ligatures: bool,
    pub outline: f32,
    pub kerning: bool,
    pub features: String,
}

impl Default for CharProps {
    fn default() -> Self {
        CharProps {
            font: "Calibri".into(),
            size: 11.0,
            bold: false,
            italic: false,
            underline: Underline::None,
            strike: false,
            color: Color::BLACK,
            highlight: Color::NONE,
            script: Script::Normal,
            caps: Caps::None,
            tracking: 0.0,
            scale: 100.0,
            baseline: 0.0,
            lang: "pl".into(),
            link: None,
            ligatures: true,
            outline: 0.0,
            kerning: true,
            features: String::new(),
        }
    }
}

impl CharProps {
    pub fn apply(&mut self, f: &CharFmt) {
        if let Some(v) = &f.font {
            self.font = v.clone();
        }
        macro_rules! a { ($($f:ident),*) => { $( if let Some(v) = f.$f { self.$f = v; } )* } }
        a!(size, bold, italic, underline, strike, color, highlight, script, caps, tracking, scale, baseline, ligatures, outline, kerning);
        if let Some(v) = &f.features {
            self.features = v.clone();
        }
        if let Some(v) = &f.lang {
            self.lang = v.clone();
        }
        if let Some(v) = &f.link {
            self.link = if v.is_empty() { None } else { Some(v.clone()) };
        }
    }

    /// Rozmiar faktycznie rysowany (indeks górny i dolny są mniejsze).
    pub fn draw_size(&self) -> f32 {
        match self.script {
            Script::Normal => self.size,
            _ => self.size * 0.62,
        }
    }

    /// Przesunięcie linii bazowej (w górę dodatnie).
    pub fn rise(&self) -> f32 {
        self.baseline
            + match self.script {
                Script::Normal => 0.0,
                Script::Super => self.size * 0.34,
                Script::Sub => -self.size * 0.14,
            }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum LineSpacing {
    /// Wielokrotność naturalnej wysokości wiersza.
    Multiple(f32),
    /// Dokładnie tyle punktów.
    Exact(f32),
    /// Co najmniej tyle punktów.
    AtLeast(f32),
}

impl Default for LineSpacing {
    fn default() -> Self {
        LineSpacing::Multiple(1.15)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Break {
    #[default]
    None,
    Column,
    Page,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ListKind {
    Bullet,
    Dash,
    Check,
    Arrow,
    Number,
    NumberParen,
    LowerLetter,
    UpperLetter,
    LowerRoman,
    UpperRoman,
}

impl ListKind {
    pub fn numbered(self) -> bool {
        !matches!(self, ListKind::Bullet | ListKind::Dash | ListKind::Check | ListKind::Arrow)
    }

    /// Znacznik dla danego numeru (od 1) i poziomu.
    pub fn marker(self, n: u32, level: u8) -> String {
        match self {
            ListKind::Bullet => ["•", "◦", "▪"][level as usize % 3].into(),
            ListKind::Dash => "–".into(),
            ListKind::Check => "✓".into(),
            ListKind::Arrow => "➔".into(),
            ListKind::Number => format!("{n}."),
            ListKind::NumberParen => format!("{n})"),
            ListKind::LowerLetter => format!("{})", letters(n)),
            ListKind::UpperLetter => format!("{}.", letters(n).to_uppercase()),
            ListKind::LowerRoman => format!("{}.", roman(n).to_lowercase()),
            ListKind::UpperRoman => format!("{}.", roman(n)),
        }
    }
}

pub fn letters(mut n: u32) -> String {
    let mut s = Vec::new();
    while n > 0 {
        n -= 1;
        s.push((b'a' + (n % 26) as u8) as char);
        n /= 26;
    }
    s.iter().rev().collect()
}

pub fn roman(mut n: u32) -> String {
    const T: [(u32, &str); 13] = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut s = String::new();
    for (v, r) in T {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListFmt {
    pub kind: ListKind,
    #[serde(default)]
    pub level: u8,
    /// Numer początkowy (wymusza nową numerację od tego akapitu).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<u32>,
}

/// Obramowanie akapitu: ramka dookoła (bez pojedynczych krawędzi).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoxBorder {
    pub width: f32,
    pub color: Color,
    #[serde(default)]
    pub radius: f32,
    #[serde(default = "default_pad")]
    pub padding: f32,
}

fn default_pad() -> f32 {
    4.0
}

/// Nadpisania formatowania akapitu.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParaFmt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_left: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_right: Option<f32>,
    /// Wcięcie pierwszego wiersza (ujemne – wysunięcie).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_line: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_before: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_after: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<LineSpacing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_next: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_lines: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widows: Option<bool>,
    /// `Some(None)` – lista wyłączona mimo stylu.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<Option<ListFmt>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub break_before: Option<Break>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shading: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<Option<BoxBorder>>,
    /// Inicjał na tyle wierszy (0 – brak).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_cap: Option<u8>,
    /// Własne tabulatory w punktach od lewej krawędzi tekstu.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyphenate: Option<bool>,
}

impl ParaFmt {
    pub fn merge(&mut self, o: &ParaFmt) {
        macro_rules! m { ($($f:ident),*) => { $( if o.$f.is_some() { self.$f = o.$f.clone(); } )* } }
        m!(align, indent_left, indent_right, first_line, space_before, space_after, line, keep_next, keep_lines, widows, list, break_before, shading, border, drop_cap, tabs, hyphenate);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParaProps {
    pub align: Align,
    pub indent_left: f32,
    pub indent_right: f32,
    pub first_line: f32,
    pub space_before: f32,
    pub space_after: f32,
    pub line: LineSpacing,
    pub keep_next: bool,
    pub keep_lines: bool,
    pub widows: bool,
    pub list: Option<ListFmt>,
    pub break_before: Break,
    pub shading: Color,
    pub border: Option<BoxBorder>,
    pub drop_cap: u8,
    pub tabs: Vec<f32>,
    pub hyphenate: bool,
}

impl Default for ParaProps {
    fn default() -> Self {
        ParaProps {
            align: Align::Left,
            indent_left: 0.0,
            indent_right: 0.0,
            first_line: 0.0,
            space_before: 0.0,
            space_after: 8.0,
            line: LineSpacing::Multiple(1.15),
            keep_next: false,
            keep_lines: false,
            widows: true,
            list: None,
            break_before: Break::None,
            shading: Color::NONE,
            border: None,
            drop_cap: 0,
            tabs: Vec::new(),
            hyphenate: false,
        }
    }
}

impl ParaProps {
    pub fn apply(&mut self, f: &ParaFmt) {
        macro_rules! a { ($($f:ident),*) => { $( if let Some(v) = f.$f { self.$f = v; } )* } }
        a!(align, indent_left, indent_right, first_line, space_before, space_after, line, keep_next, keep_lines, widows, list, break_before, shading, border, drop_cap, hyphenate);
        if let Some(t) = &f.tabs {
            self.tabs = t.clone();
        }
    }
}

/// Styl akapitu (z formatowaniem znaków).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Style {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(default)]
    pub para: ParaFmt,
    #[serde(default)]
    pub chars: CharFmt,
    /// Poziom w konspekcie (nagłówki: 1..9) – nawigator, spis treści, eksport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<u8>,
    /// Styl widoczny w galerii stylów na wstążce.
    #[serde(default = "yes")]
    pub gallery: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Field {
    PageNumber,
    PageCount,
    Date,
    Time,
    Title,
    Author,
    /// Kotwica obiektu: obiekt przesuwa się razem z akapitem, w którym stoi ten znak.
    Anchor(u64),
}

/// Znak zastępujący pole w tekście akapitu (jedno miejsce kursora).
pub const FIELD_CHAR: char = '\u{FFFC}';

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub text: String,
    #[serde(default, skip_serializing_if = "CharFmt::is_empty")]
    pub fmt: CharFmt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<Field>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    #[serde(default = "normal_id")]
    pub style: String,
    #[serde(default)]
    pub fmt: ParaFmt,
    pub runs: Vec<Run>,
}

fn normal_id() -> String {
    "normal".into()
}

impl Default for Paragraph {
    fn default() -> Self {
        Paragraph { style: normal_id(), fmt: ParaFmt::default(), runs: vec![Run::default()] }
    }
}

impl Paragraph {
    pub fn with_text(style: &str, text: &str) -> Paragraph {
        Paragraph { style: style.into(), fmt: ParaFmt::default(), runs: vec![Run { text: text.into(), ..Default::default() }] }
    }

    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.runs.iter().map(|r| r.text.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Przebieg zawierający przesunięcie `off` i przesunięcie wewnątrz niego. Na granicy –
    /// przebieg poprzedni (pisanie dziedziczy formatowanie znaku przed kursorem).
    pub fn run_at(&self, off: usize) -> (usize, usize) {
        let mut start = 0;
        for (i, r) in self.runs.iter().enumerate() {
            let end = start + r.text.len();
            if off <= end && (off > start || i == 0 || r.text.is_empty()) {
                if off == start && i > 0 {
                    return (i - 1, self.runs[i - 1].text.len());
                }
                return (i, off - start);
            }
            start = end;
        }
        let last = self.runs.len().saturating_sub(1);
        (last, self.runs.get(last).map(|r| r.text.len()).unwrap_or(0))
    }

    /// Dzieli przebiegi tak, by `off` był granicą; zwraca indeks pierwszego przebiegu od `off`.
    pub fn split_at(&mut self, off: usize) -> usize {
        let mut start = 0;
        for i in 0..self.runs.len() {
            let len = self.runs[i].text.len();
            if off == start {
                return i;
            }
            if off < start + len {
                let local = off - start;
                let tail = self.runs[i].text.split_off(local);
                let r = Run { text: tail, fmt: self.runs[i].fmt.clone(), field: self.runs[i].field };
                self.runs.insert(i + 1, r);
                return i + 1;
            }
            start += len;
        }
        self.runs.len()
    }

    /// Scala sąsiednie przebiegi o tym samym formatowaniu i usuwa puste (zostaje co najmniej jeden).
    pub fn normalize(&mut self) {
        let mut out: Vec<Run> = Vec::with_capacity(self.runs.len());
        for r in self.runs.drain(..) {
            if r.text.is_empty() {
                continue;
            }
            if let Some(last) = out.last_mut() {
                if last.fmt == r.fmt && last.field.is_none() && r.field.is_none() {
                    last.text.push_str(&r.text);
                    continue;
                }
            }
            out.push(r);
        }
        if out.is_empty() {
            out.push(Run::default());
        }
        self.runs = out;
    }

    /// Formatowanie znaku w miejscu `off` (to, które dostanie wpisywany tekst).
    pub fn fmt_at(&self, off: usize) -> CharFmt {
        let (i, _) = self.run_at(off);
        self.runs.get(i).map(|r| r.fmt.clone()).unwrap_or_default()
    }
}

/// Historia tekstu – ciąg akapitów płynący przez strony, kolumny albo łańcuch ramek.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Story {
    pub paras: Vec<Paragraph>,
}

impl Default for Story {
    fn default() -> Self {
        Story { paras: vec![Paragraph::default()] }
    }
}

impl Story {
    pub fn plain(text: &str, style: &str) -> Story {
        let paras: Vec<Paragraph> = text.split('\n').map(|l| Paragraph::with_text(style, l)).collect();
        Story { paras: if paras.is_empty() { vec![Paragraph::default()] } else { paras } }
    }

    pub fn text(&self) -> String {
        self.paras.iter().map(|p| p.text()).collect::<Vec<_>>().join("\n")
    }
}

/// Węzeł kształtu we współrzędnych ramki obiektu (0..1 w poziomie i pionie). Uchwyty krzywych
/// (`cin` przed węzłem, `cout` za nim) w tych samych współrzędnych; brak – odcinek prosty.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub x: f32,
    pub y: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cin: Option<(f32, f32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cout: Option<(f32, f32)>,
    /// Węzeł gładki: przeciąganie jednego uchwytu obraca drugi.
    #[serde(default)]
    pub smooth: bool,
}

impl Node {
    pub fn corner(x: f32, y: f32) -> Node {
        Node { x, y, cin: None, cout: None, smooth: false }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Contour {
    pub nodes: Vec<Node>,
    #[serde(default = "yes")]
    pub closed: bool,
}

/// Kształt: jeden lub więcej konturów (otwory – reguła parzystości).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub contours: Vec<Contour>,
}

impl Shape {
    pub fn rect() -> Shape {
        Shape::polygon(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
    }

    pub fn polygon(pts: &[(f32, f32)]) -> Shape {
        Shape { contours: vec![Contour { nodes: pts.iter().map(|&(x, y)| Node::corner(x, y)).collect(), closed: true }] }
    }

    pub fn is_rect(&self) -> bool {
        *self == Shape::rect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum WrapMode {
    /// Tekst ignoruje obiekt, obiekt nad tekstem.
    #[default]
    None,
    /// Tekst opływa z obu stron (także w otworach pomiędzy).
    Both,
    /// Tekst tylko po lewej stronie obiektu.
    Left,
    /// Tekst tylko po prawej stronie obiektu.
    Right,
    /// Tekst po stronie z większą ilością miejsca.
    Largest,
    /// Tekst tylko nad i pod obiektem.
    TopBottom,
    /// Obiekt za tekstem.
    Behind,
}

impl WrapMode {
    pub fn excludes(self) -> bool {
        !matches!(self, WrapMode::None | WrapMode::Behind)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wrap {
    pub mode: WrapMode,
    /// Odstęp tekstu od obiektu.
    #[serde(default = "default_wrap_dist")]
    pub dist: f32,
    /// Opływanie konturu kształtu (inaczej – prostokąta ramki).
    #[serde(default = "yes")]
    pub contour: bool,
}

fn default_wrap_dist() -> f32 {
    9.0
}

impl Default for Wrap {
    fn default() -> Self {
        Wrap { mode: WrapMode::None, dist: default_wrap_dist(), contour: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrokeStyle {
    pub color: Color,
    pub width: f32,
    #[serde(default)]
    pub dash: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Fill {
    Solid { color: Color },
    /// Gradient liniowy pod kątem (stopnie, 0 – z lewej do prawej).
    Linear { from: Color, to: Color, angle: f32 },
    Radial { from: Color, to: Color },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VAlign {
    #[default]
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageFit {
    /// Wypełnia kształt, nadmiar przycięty.
    #[default]
    Cover,
    /// Cały obraz w kształcie.
    Contain,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChartKind {
    #[default]
    Column,
    Bar,
    Line,
    Area,
    Pie,
    Doughnut,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Series {
    pub name: String,
    pub values: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chart {
    pub kind: ChartKind,
    #[serde(default)]
    pub title: String,
    pub labels: Vec<String>,
    pub series: Vec<Series>,
    #[serde(default = "yes")]
    pub legend: bool,
    #[serde(default = "yes")]
    pub grid: bool,
    #[serde(default = "yes")]
    pub values: bool,
    #[serde(default = "chart_font")]
    pub font: String,
}

fn chart_font() -> String {
    "Segoe UI".into()
}

impl Default for Chart {
    fn default() -> Self {
        Chart {
            kind: ChartKind::Column,
            title: String::new(),
            labels: vec!["I kw.".into(), "II kw.".into(), "III kw.".into(), "IV kw.".into()],
            series: vec![
                Series { name: "2025".into(), values: vec![12.0, 18.0, 15.0, 22.0], color: None },
                Series { name: "2026".into(), values: vec![16.0, 21.0, 19.0, 27.0], color: None },
            ],
            legend: true,
            grid: true,
            values: true,
            font: chart_font(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub rows: Vec<Vec<String>>,
    #[serde(default = "yes")]
    pub header: bool,
    /// Co drugi wiersz z delikatnym tłem.
    #[serde(default = "yes")]
    pub stripes: bool,
    #[serde(default = "chart_font")]
    pub font: String,
    #[serde(default = "table_size")]
    pub size: f32,
    #[serde(default = "table_accent")]
    pub accent: Color,
    #[serde(default = "table_line")]
    pub line: Color,
    /// Wyrównanie kolumn liczbowych do prawej.
    #[serde(default = "yes")]
    pub numbers_right: bool,
    /// Względne szerokości kolumn (puste – dopasowanie do treści).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub widths: Vec<f32>,
    /// Ile wierszy mieści się na tej stronie (reszta przechodzi do kontynuacji na następnej).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub take: Option<usize>,
}

/// Komórka scalona z poprzednią (z lewej).
pub const MERGED: &str = "<";

impl Table {
    /// Wiersze pokazane na tej stronie.
    pub fn shown(&self) -> &[Vec<String>] {
        let n = self.take.unwrap_or(self.rows.len()).min(self.rows.len());
        &self.rows[..n]
    }

    pub fn cols(&self) -> usize {
        self.rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1)
    }
}

fn table_size() -> f32 {
    10.0
}

fn table_accent() -> Color {
    Color::rgb(0x1f, 0x4e, 0x79)
}

fn table_line() -> Color {
    Color::rgb(0xd0, 0xd5, 0xdd)
}

impl Default for Table {
    fn default() -> Self {
        Table {
            rows: vec![
                vec!["Pozycja".into(), "Ilość".into(), "Cena".into(), "Razem".into()],
                vec!["Projekt".into(), "1".into(), "1200".into(), "1200".into()],
                vec!["Skład".into(), "12".into(), "80".into(), "960".into()],
                vec!["Druk".into(), "500".into(), "1,2".into(), "600".into()],
            ],
            header: true,
            stripes: true,
            font: chart_font(),
            size: table_size(),
            accent: table_accent(),
            line: table_line(),
            numbers_right: true,
            widths: Vec::new(),
            take: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ObjKind {
    /// Ramka tekstowa o dowolnym kształcie. Tekst należy do pierwszej ramki łańcucha (`next`).
    Text {
        story: Story,
        #[serde(default = "default_pad")]
        padding: f32,
        #[serde(default = "one")]
        columns: u8,
        #[serde(default = "default_gap")]
        gap: f32,
        #[serde(default)]
        valign: VAlign,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next: Option<u64>,
    },
    Image {
        asset: String,
        #[serde(default)]
        fit: ImageFit,
        /// Własny kontur opływania (np. wykryty z przezroczystości) – współrzędne 0..1.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        contour: Option<Shape>,
    },
    /// Sam kształt (wypełnienie i obrys).
    Shape,
    Chart { chart: Chart },
    /// Tabela: wiersze komórek, pierwszy wiersz może być nagłówkiem.
    Table { table: Table },
    /// Tekst biegnący po krzywej kształtu.
    PathText {
        story: Story,
        /// Przesunięcie początku tekstu wzdłuż ścieżki (0..1).
        #[serde(default)]
        offset: f32,
    },
}

fn one() -> u8 {
    1
}

fn default_gap() -> f32 {
    12.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    pub kind: ObjKind,
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub shape: Shape,
    #[serde(default)]
    pub wrap: Wrap,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<StrokeStyle>,
    #[serde(default = "full")]
    pub opacity: f32,
    /// Zaokrąglenie narożników (punkty) – dla kształtów z narożnikami prostymi.
    #[serde(default)]
    pub corner: f32,
    #[serde(default)]
    pub shadow: bool,
    #[serde(default)]
    pub locked: bool,
    /// Odstęp od punktu zakotwiczenia (koniec poprzedniego akapitu) w pionie.
    #[serde(default = "anchor_dy")]
    pub anchor_dy: f32,
    /// Kontynuacja tabeli dzielonej na strony (tworzona przez skład, nie edytowana).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cont_of: Option<u64>,
}

fn anchor_dy() -> f32 {
    4.0
}

fn full() -> f32 {
    1.0
}

impl Object {
    pub fn story(&self) -> Option<&Story> {
        match &self.kind {
            ObjKind::Text { story, .. } | ObjKind::PathText { story, .. } => Some(story),
            _ => None,
        }
    }

    pub fn story_mut(&mut self) -> Option<&mut Story> {
        match &mut self.kind {
            ObjKind::Text { story, .. } | ObjKind::PathText { story, .. } => Some(story),
            _ => None,
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }

    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            ObjKind::Text { .. } => "text",
            ObjKind::Image { .. } => "image",
            ObjKind::Shape => "shape",
            ObjKind::Table { .. } => "table",
            ObjKind::Chart { .. } => "chart",
            ObjKind::PathText { .. } => "path-text",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageSetup {
    pub width: f32,
    pub height: f32,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    #[serde(default = "one")]
    pub columns: u8,
    #[serde(default = "default_gap")]
    pub column_gap: f32,
    /// Linia między kolumnami.
    #[serde(default)]
    pub column_rule: bool,
    #[serde(default = "default_hd")]
    pub header_dist: f32,
    #[serde(default = "default_hd")]
    pub footer_dist: f32,
    #[serde(default)]
    pub different_first: bool,
    #[serde(default = "white")]
    pub color: Color,
    /// Numeracja stron od tej liczby.
    #[serde(default = "one32")]
    pub first_number: u32,
    /// Znak wodny – ukośny napis na każdej stronie (pusty – brak).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub watermark: String,
    /// Obramowanie strony (odstęp od krawędzi kartki w punktach).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<StrokeStyle>,
    #[serde(default = "border_inset")]
    pub border_inset: f32,
}

fn border_inset() -> f32 {
    24.0
}

fn default_hd() -> f32 {
    35.4
}

fn white() -> Color {
    Color::WHITE
}

fn one32() -> u32 {
    1
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup {
            width: 210.0 * MM,
            height: 297.0 * MM,
            margin_top: 25.0 * MM,
            margin_bottom: 25.0 * MM,
            margin_left: 25.0 * MM,
            margin_right: 25.0 * MM,
            columns: 1,
            column_gap: 12.5 * MM / 1.0,
            column_rule: false,
            header_dist: default_hd(),
            footer_dist: default_hd(),
            different_first: false,
            color: Color::WHITE,
            first_number: 1,
            watermark: String::new(),
            border: None,
            border_inset: border_inset(),
        }
    }
}

impl PageSetup {
    pub fn landscape(&self) -> bool {
        self.width > self.height
    }

    pub fn content_width(&self) -> f32 {
        self.width - self.margin_left - self.margin_right
    }

    /// Kolumny treści głównej: (x0, x1).
    pub fn column_spans(&self) -> Vec<(f32, f32)> {
        let n = self.columns.max(1) as f32;
        let cw = self.content_width();
        let w = (cw - self.column_gap * (n - 1.0)) / n;
        (0..self.columns.max(1)).map(|i| {
            let x0 = self.margin_left + i as f32 * (w + self.column_gap);
            (x0, x0 + w)
        }).collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub keywords: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub modified: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocSettings {
    /// Język domyślny tekstu (pisownia).
    #[serde(default = "pl")]
    pub lang: String,
    /// Polskie sierotki: krótkie wyrazy (a, i, o, u, w, z…) nie zostają na końcu wiersza.
    #[serde(default = "yes")]
    pub orphans: bool,
    #[serde(default)]
    pub hyphenate: bool,
    /// Siatka bazowa (punkty, 0 – wyłączona).
    #[serde(default)]
    pub baseline_grid: f32,
}

fn pl() -> String {
    "pl".into()
}

impl Default for DocSettings {
    fn default() -> Self {
        DocSettings { lang: pl(), orphans: true, hyphenate: false, baseline_grid: 0.0 }
    }
}

/// Plik osadzony w dokumencie (obraz).
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub mime: String,
    pub data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(default = "format_version")]
    pub format: u32,
    #[serde(default)]
    pub meta: Meta,
    pub page: PageSetup,
    pub styles: Vec<Style>,
    pub body: Story,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<Story>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<Story>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_first: Option<Story>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer_first: Option<Story>,
    #[serde(default)]
    pub objects: Vec<Object>,
    #[serde(default)]
    pub settings: DocSettings,
    /// Najmniejsza liczba stron (puste strony na obiekty).
    #[serde(default)]
    pub min_pages: usize,
    #[serde(skip)]
    pub assets: BTreeMap<String, Asset>,
    #[serde(default = "one64")]
    pub next_id: u64,
}

fn format_version() -> u32 {
    1
}

fn one64() -> u64 {
    1
}

/// Który tekst: treść główna, nagłówek/stopka albo historia obiektu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StoryRef {
    Body,
    Header,
    Footer,
    HeaderFirst,
    FooterFirst,
    Obj(u64),
}

impl Default for Document {
    fn default() -> Self {
        Document::new()
    }
}

impl Document {
    pub fn new() -> Document {
        Document {
            format: 1,
            meta: Meta::default(),
            page: PageSetup::default(),
            styles: crate::styles::defaults(),
            body: Story::default(),
            header: None,
            footer: None,
            header_first: None,
            footer_first: None,
            objects: Vec::new(),
            settings: DocSettings::default(),
            min_pages: 0,
            assets: BTreeMap::new(),
            next_id: 1,
        }
    }

    pub fn new_id(&mut self) -> u64 {
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        id
    }

    pub fn style(&self, id: &str) -> Option<&Style> {
        self.styles.iter().find(|s| s.id == id)
    }

    pub fn object(&self, id: u64) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn object_mut(&mut self, id: u64) -> Option<&mut Object> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    pub fn story(&self, r: StoryRef) -> Option<&Story> {
        match r {
            StoryRef::Body => Some(&self.body),
            StoryRef::Header => self.header.as_ref(),
            StoryRef::Footer => self.footer.as_ref(),
            StoryRef::HeaderFirst => self.header_first.as_ref(),
            StoryRef::FooterFirst => self.footer_first.as_ref(),
            StoryRef::Obj(id) => self.object(id).and_then(|o| o.story()),
        }
    }

    pub fn story_mut(&mut self, r: StoryRef) -> Option<&mut Story> {
        match r {
            StoryRef::Body => Some(&mut self.body),
            StoryRef::Header => self.header.as_mut(),
            StoryRef::Footer => self.footer.as_mut(),
            StoryRef::HeaderFirst => self.header_first.as_mut(),
            StoryRef::FooterFirst => self.footer_first.as_mut(),
            StoryRef::Obj(id) => self.object_mut(id).and_then(|o| o.story_mut()),
        }
    }

    /// Łańcuch ramek zaczynający się od `head` (z zabezpieczeniem przed pętlą).
    pub fn chain(&self, head: u64) -> Vec<u64> {
        let mut out = vec![head];
        let mut cur = head;
        while let Some(ObjKind::Text { next: Some(n), .. }) = self.object(cur).map(|o| &o.kind) {
            if out.contains(n) || self.object(*n).is_none() {
                break;
            }
            out.push(*n);
            cur = *n;
        }
        out
    }

    /// Ramka będąca początkiem łańcucha, do którego należy `id` (sama `id`, gdy jest pierwsza).
    pub fn chain_head(&self, id: u64) -> u64 {
        let mut cur = id;
        let mut seen = vec![id];
        loop {
            let prev = self.objects.iter().find(|o| matches!(o.kind, ObjKind::Text { next: Some(n), .. } if n == cur));
            match prev {
                Some(p) if !seen.contains(&p.id) => {
                    seen.push(p.id);
                    cur = p.id;
                }
                _ => return cur,
            }
        }
    }

    /// Rozstrzygnięte formatowanie akapitu i bazowe formatowanie jego znaków (styl + nadpisania akapitu).
    pub fn resolve_para(&self, p: &Paragraph) -> (ParaProps, CharProps) {
        let mut chain = Vec::new();
        let mut cur = self.style(&p.style).or_else(|| self.style("normal"));
        while let Some(s) = cur {
            if chain.iter().any(|c: &&Style| c.id == s.id) || chain.len() > 16 {
                break;
            }
            chain.push(s);
            cur = s.based_on.as_deref().and_then(|b| self.style(b));
        }
        let mut pp = ParaProps::default();
        let mut cp = CharProps { lang: self.settings.lang.clone(), ..CharProps::default() };
        for s in chain.iter().rev() {
            pp.apply(&s.para);
            cp.apply(&s.chars);
        }
        pp.apply(&p.fmt);
        (pp, cp)
    }

    pub fn outline_level(&self, p: &Paragraph) -> Option<u8> {
        let mut cur = self.style(&p.style);
        let mut n = 0;
        while let Some(s) = cur {
            if let Some(l) = s.outline {
                return Some(l);
            }
            n += 1;
            if n > 16 {
                break;
            }
            cur = s.based_on.as_deref().and_then(|b| self.style(b));
        }
        None
    }

    pub fn page_count_hint(&self) -> usize {
        let objs = self.objects.iter().map(|o| o.page + 1).max().unwrap_or(1);
        objs.max(self.min_pages).max(1)
    }
}
