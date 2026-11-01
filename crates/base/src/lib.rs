//! OpenDTP – podstawa wspólna dla wszystkich modułów: tłumaczenia, ścieżki, drobne narzędzia,
//! wersja programu i dane autora.

pub mod fsx;
pub mod lang;
pub mod log;
pub mod paths;
pub mod util;

/// Wersja programu (CI podaje numer z tagu przy kompilacji).
pub fn version() -> &'static str {
    option_env!("OPENDTP_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// Autor i wsparcie – jedno źródło dla okna „O programie”, stopki ustawień i instalatora.
pub const AUTHOR_LINE: &str = "Pomysł i wykonanie: Adam Rędzikowski · redzikowski.com";
pub const AUTHOR_URL: &str = "https://redzikowski.com";
pub const SUPPORT_URL: &str = "https://buycoffee.to/adamredzikowski";
