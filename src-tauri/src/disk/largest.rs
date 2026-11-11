//! Keeps the N largest files seen during a walk.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub parent: String,
    pub bytes: u64,
    pub last_used_secs: i64,
    /// video | archive | installer | disk-image | other
    pub kind: &'static str,
}

pub fn kind_of(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "mov" | "mp4" | "m4v" | "mkv" | "avi" | "webm" => "video",
        "zip" | "tar" | "gz" | "tgz" | "xz" | "bz2" | "7z" | "rar" | "xip" => "archive",
        "pkg" | "mpkg" | "app" => "installer",
        "dmg" | "iso" | "img" | "raw" | "vmdk" | "qcow2" | "sparseimage" | "sparsebundle" => {
            "disk-image"
        }
        _ => "other",
    }
}

pub struct TopFiles {
    limit: usize,
    heap: BinaryHeap<Reverse<(u64, PathBuf, i64)>>,
}

impl TopFiles {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            heap: BinaryHeap::with_capacity(limit + 1),
        }
    }

    pub fn push(&mut self, bytes: u64, path: &Path, last_used: i64) {
        if self.limit == 0 {
            return;
        }
        if self.heap.len() == self.limit {
            match self.heap.peek() {
                Some(Reverse((min, _, _))) if bytes <= *min => return,
                _ => {}
            }
            self.heap.pop();
        }
        self.heap
            .push(Reverse((bytes, path.to_path_buf(), last_used)));
    }

    /// Largest first.
    pub fn into_vec(self) -> Vec<FileInfo> {
        let mut out: Vec<_> = self
            .heap
            .into_iter()
            .map(|Reverse((bytes, path, last_used))| FileInfo {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                parent: path
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                kind: kind_of(&path),
                path: path.display().to_string(),
                bytes,
                last_used_secs: last_used,
            })
            .collect();
        out.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_largest() {
        let mut top = TopFiles::new(3);
        for (i, size) in [5u64, 1, 9, 3, 7, 2].iter().enumerate() {
            top.push(*size, Path::new(&format!("/f{i}")), 0);
        }
        let sizes: Vec<_> = top.into_vec().iter().map(|f| f.bytes).collect();
        assert_eq!(sizes, vec![9, 7, 5]);
    }

    #[test]
    fn classifies_kinds() {
        assert_eq!(kind_of(Path::new("a/b.MOV")), "video");
        assert_eq!(kind_of(Path::new("Xcode_16.xip")), "archive");
        assert_eq!(kind_of(Path::new("ubuntu.iso")), "disk-image");
        assert_eq!(kind_of(Path::new("notes.txt")), "other");
    }
}
