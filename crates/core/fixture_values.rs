// The fictional metadata values written into the fixtures (design §11.1,
// NFR-06). Tests search cleaned output for them.
//
// Kept apart from examples/gen_fixtures.rs, which pulls these in with
// `include!`, so that tests of crates without the generator's dependencies
// (crates/worker) can include them too. Plain `//` comments only: an inner
// doc comment is not allowed where `include!` expands.

pub const AUTHOR: &str = "Example Author";
pub const EDITOR: &str = "Example Editor";
pub const SOFTWARE: &str = "Example Software";
pub const CAMERA_MAKE: &str = "Example Camera Make";
pub const CAMERA_MODEL: &str = "Example Camera Model";
pub const SERIAL_NUMBER: &str = "EX-12345678";
pub const DATE_TIME: &str = "2026:01:02 03:04:05";
pub const DATE_TIME_ISO: &str = "2026-01-02T03:04:05";
pub const DATE_TIME_PDF: &str = "D:20260102030405Z";
pub const COMMENT: &str = "Example Comment";
pub const TITLE: &str = "Example Title";
pub const SUBJECT: &str = "Example Subject";
pub const KEYWORDS: &str = "Example Keywords";
pub const COPYRIGHT: &str = "Copyright (C) 2026 Example Author";
pub const CITY: &str = "Example City";
pub const STATE: &str = "Example State";
pub const COUNTRY: &str = "Example Country";
