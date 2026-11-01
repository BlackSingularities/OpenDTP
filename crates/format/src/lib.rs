//! OpenDTP – własny format `.odtp`, tekst i Markdown, metapliki WMF/EMF i biblioteka elementów.

pub mod library;
pub mod wmf;
pub mod wmf_svg;

pub mod io {
    pub mod odtp;
    pub mod plain;
}

#[allow(unused_imports)]
use opendtp_base::{fsx, lang, paths, trf, util};
#[allow(unused_imports)]
use opendtp_doc::{geom, model, styles};
#[allow(unused_imports)]
use opendtp_text::{fonts, text};
