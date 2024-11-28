use std::fmt::Debug;
use std::io;

use byteorder::WriteBytesExt;
use bytes::Bytes;
use paste::paste;

pub mod header;
mod traits;
pub mod types;

#[macro_use]
mod macros;

use header::BoxHeader;
pub use traits::BoxType;

use crate::boxes::types::av01::Av01;
use crate::boxes::types::av1c::Av1C;
use crate::boxes::types::avc1::Avc1;
use crate::boxes::types::avcc::AvcC;
use crate::boxes::types::btrt::Btrt;
use crate::boxes::types::clap::Clap;
use crate::boxes::types::co64::Co64;
use crate::boxes::types::colr::Colr;
use crate::boxes::types::ctts::Ctts;
use crate::boxes::types::dinf::Dinf;
use crate::boxes::types::dref::Dref;
use crate::boxes::types::edts::Edts;
use crate::boxes::types::elst::Elst;
use crate::boxes::types::esds::Esds;
use crate::boxes::types::ftyp::Ftyp;
use crate::boxes::types::hdlr::Hdlr;
use crate::boxes::types::hev1::Hev1;
use crate::boxes::types::hmhd::Hmhd;
use crate::boxes::types::hvcc::HvcC;
use crate::boxes::types::mdat::Mdat;
use crate::boxes::types::mdhd::Mdhd;
use crate::boxes::types::mdia::Mdia;
use crate::boxes::types::mehd::Mehd;
use crate::boxes::types::mfhd::Mfhd;
use crate::boxes::types::minf::Minf;
use crate::boxes::types::moof::Moof;
use crate::boxes::types::moov::Moov;
use crate::boxes::types::mp4a::Mp4a;
use crate::boxes::types::mvex::Mvex;
use crate::boxes::types::mvhd::Mvhd;
use crate::boxes::types::nmhd::Nmhd;
use crate::boxes::types::opus::Opus;
use crate::boxes::types::padb::Padb;
use crate::boxes::types::pasp::Pasp;
use crate::boxes::types::sbgp::Sbgp;
use crate::boxes::types::sdtp::Sdtp;
use crate::boxes::types::smhd::Smhd;
use crate::boxes::types::stbl::Stbl;
use crate::boxes::types::stco::Stco;
use crate::boxes::types::stdp::Stdp;
use crate::boxes::types::stsc::Stsc;
use crate::boxes::types::stsd::Stsd;
use crate::boxes::types::stsh::Stsh;
use crate::boxes::types::stss::Stss;
use crate::boxes::types::stsz::Stsz;
use crate::boxes::types::stts::Stts;
use crate::boxes::types::stz2::Stz2;
use crate::boxes::types::subs::Subs;
use crate::boxes::types::tfdt::Tfdt;
use crate::boxes::types::tfhd::Tfhd;
use crate::boxes::types::tkhd::Tkhd;
use crate::boxes::types::traf::Traf;
use crate::boxes::types::trak::Trak;
use crate::boxes::types::trex::Trex;
use crate::boxes::types::trun::Trun;
use crate::boxes::types::url::Url;
use crate::boxes::types::vmhd::Vmhd;

#[rustfmt::skip]
impl_box!(
    Ftyp, Moov, Mvhd, Mvex, Trak, Trex,
    Mehd, Mdia, Tkhd, Edts, Elst, Mdhd,
    Minf, Hdlr, Dinf, Stbl, Hmhd, Nmhd,
    Smhd, Vmhd, Dref, Stsd, Stsz, Stsc,
    Stco, Co64, Stts, Stss, Stz2, Stsh,
    Ctts, Stdp, Sbgp, Subs, Padb, Sdtp,
    Url, Avc1, Clap, Pasp, AvcC, Btrt,
    Mp4a, Esds, Moof, Mfhd, Traf, Tfhd,
    Tfdt, Trun, Mdat, Av01, Av1C, Colr,
    Hev1, HvcC, Opus,
);
