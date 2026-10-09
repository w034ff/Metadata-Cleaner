//! Reads and rewrites JPEG, PNG and WebP files to remove their metadata,
//! without Tauri (design §3, §4).

pub mod detect;
pub mod error;
pub mod exif;
pub mod image_file;
pub mod iptc;
pub mod jpeg;
pub mod naming;
pub mod png;
pub mod report;
pub mod webp;
pub mod xmp;
