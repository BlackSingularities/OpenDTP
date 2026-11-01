//! OpenDTP – rdzeń: model dokumentu, style, geometria kształtów, czcionki, kształtowanie tekstu,
//! silnik składu, lista rysowania, renderer, edycja, pisownia, pliki, ustawienia i aktualizacje.

pub mod ai;
pub mod edit;
pub mod fonts;
pub mod geom;
pub mod io;
pub mod lang;
pub mod layout;
pub mod model;
pub mod paint;
pub mod paths;
pub mod raster;
pub mod sample;
pub mod spell;
pub mod styles;
pub mod text;
pub mod update;
pub mod util;

/// Wersja programu (CI podaje numer z tagu przy kompilacji).
pub fn version() -> &'static str {
    option_env!("OPENDTP_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// Autor i wsparcie – jedno źródło dla okna „O programie”, stopki ustawień i instalatora.
pub const AUTHOR_LINE: &str = "Pomysł i wykonanie: Adam Rędzikowski · redzikowski.com";
pub const AUTHOR_URL: &str = "https://redzikowski.com";
pub const SUPPORT_URL: &str = "https://buycoffee.to/adamredzikowski";
