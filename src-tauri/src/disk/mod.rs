//! Disk Map: size tree of a folder and its largest files.

pub mod largest;
pub mod walk;

/// How many of the largest files a scan keeps.
pub const TOP_FILES: usize = 300;
