//! OpenDTP – rdzeń: jedno wejście do wszystkich modułów (`crates/*`) pod dotychczasowymi ścieżkami,
//! żeby interfejs nie musiał znać podziału na crate'y.

pub use opendtp_ai::ai;
pub use opendtp_base::{fsx, lang, log, paths, trf, util, version, AUTHOR_LINE, AUTHOR_URL, SUPPORT_URL};
pub use opendtp_doc::{edit, geom, model, styles};
pub use opendtp_format::{library, wmf, wmf_svg};
pub use opendtp_render::{layout, paint, preflight, raster, sample};
pub use opendtp_text::{fonts, spell, text};
pub use opendtp_tools::{a11y, cite, compare, csv, inspect, labels, merge, tables};
pub use opendtp_update::update;

/// Pliki: własny format `.odtp`, eksport DOCX/ODT/RTF/PDF/TXT/MD/HTML/EPUB, import DOCX/ODT/RTF/HTML/TXT/MD.
pub mod io {
    pub use opendtp_io::io::{docx, docx_in, epub, html_in, odt_in, odt_out, odtp, pdf, plain, rtf_in, rtf_out};
}
