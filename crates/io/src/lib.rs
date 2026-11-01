//! OpenDTP – wymiana z innymi programami: DOCX (import i eksport), PDF, EPUB, import ODT, RTF
//! i HTML, wklejanie HTML.

pub mod io {
    pub use opendtp_format::io::{odtp, plain};

    pub mod docx;
    pub mod docx_in;
    pub mod epub;
    pub mod html_in;
    pub mod odt_in;
    pub mod odt_out;
    pub mod pdf;
    pub mod rtf_in;
    pub mod rtf_out;
}

#[allow(unused_imports)]
use opendtp_base::{fsx, lang, paths, trf, util, version};
#[allow(unused_imports)]
use opendtp_doc::{edit, geom, model, styles};
#[allow(unused_imports)]
use opendtp_render::{layout, paint, raster};
#[allow(unused_imports)]
use opendtp_text::{fonts, text};
