#[cfg(feature = "bz")]
pub(crate) mod bzip2;
#[cfg(feature = "gz")]
pub(crate) mod gzip;
#[cfg(feature = "lz")]
pub(crate) mod lz4;
#[cfg(feature = "xz")]
pub(crate) mod xz;
