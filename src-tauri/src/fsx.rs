//! Size-on-disk helpers shared by Cleanup and Disk Map.

use std::collections::HashSet;
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Bytes the file really occupies on disk. Uses allocated blocks, so sparse files
/// (Docker.raw, VM images) report what they use, not their logical length.
pub fn allocated(meta: &Metadata) -> u64 {
    meta.blocks() * 512
}

/// Latest of last-access and last-modified time, as Unix seconds. Used as "last used".
pub fn last_used_secs(meta: &Metadata) -> i64 {
    meta.atime().max(meta.mtime())
}

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug, serde::Serialize, Clone, Copy)]
pub struct VolumeInfo {
    pub total: u64,
    pub free: u64,
}

/// Total and available bytes of the volume that holds `path`.
pub fn volume(path: &Path) -> Option<VolumeInfo> {
    let c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    let mut st = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: c is a valid C string and st points to writable statvfs storage.
    let rc = unsafe { libc::statvfs(c.as_ptr(), st.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    // SAFETY: statvfs returned 0, so it filled the struct.
    let st = unsafe { st.assume_init() };
    let block: u64 = st.f_frsize;
    Some(VolumeInfo {
        total: st.f_blocks as u64 * block,
        free: st.f_bavail as u64 * block,
    })
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub bytes: u64,
    pub files: u64,
    pub errors: u64,
}

/// Counts each hard-linked inode once.
#[derive(Default)]
pub struct InodeSet(HashSet<(u64, u64)>);

impl InodeSet {
    /// Returns true the first time an inode is seen (or when it has a single link).
    pub fn first_sight(&mut self, meta: &Metadata) -> bool {
        meta.nlink() <= 1 || self.0.insert((meta.dev(), meta.ino()))
    }
}

/// Size on disk of a file or a whole folder. Never follows symlinks, counts hard
/// links once and stays on the volume `path` lives on.
pub fn usage(path: &Path) -> Usage {
    let mut usage = Usage::default();
    let Ok(meta) = path.symlink_metadata() else {
        usage.errors += 1;
        return usage;
    };
    let mut seen = InodeSet::default();
    if !meta.is_dir() {
        if seen.first_sight(&meta) {
            usage.bytes = allocated(&meta);
            usage.files = 1;
        }
        return usage;
    }
    let dev = meta.dev();
    for entry in jwalk::WalkDir::new(path)
        .follow_links(false)
        .skip_hidden(false)
        .sort(false)
    {
        let Ok(entry) = entry else {
            usage.errors += 1;
            continue;
        };
        if entry.depth == 0 || entry.file_type.is_dir() {
            continue;
        }
        let Ok(meta) = entry.path().symlink_metadata() else {
            usage.errors += 1;
            continue;
        };
        if meta.dev() != dev || !seen.first_sight(&meta) {
            continue;
        }
        usage.bytes += allocated(&meta);
        usage.files += 1;
    }
    usage
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn sparse_file_counts_allocated_blocks_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sparse.img");
        let f = File::create(&path).unwrap();
        f.set_len(1 << 30).unwrap(); // 1 GiB logical, nothing written
        let u = usage(&path);
        assert!(u.bytes < 1 << 20, "sparse file reported {} bytes", u.bytes);
    }

    #[test]
    fn hard_links_are_counted_once() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.bin");
        let mut f = File::create(&a).unwrap();
        f.write_all(&vec![7u8; 64 * 1024]).unwrap();
        f.sync_all().unwrap();
        fs::hard_link(&a, dir.path().join("b.bin")).unwrap();
        let single = usage(&a).bytes;
        let both = usage(dir.path());
        assert_eq!(both.files, 1);
        assert_eq!(both.bytes, single);
    }

    #[test]
    fn symlinks_are_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let mut f = File::create(target.path().join("big.bin")).unwrap();
        f.write_all(&vec![1u8; 256 * 1024]).unwrap();
        f.sync_all().unwrap();
        std::os::unix::fs::symlink(target.path(), dir.path().join("link")).unwrap();
        let u = usage(dir.path());
        assert!(u.bytes < 64 * 1024, "followed symlink: {} bytes", u.bytes);
    }
}
