//! Walks a folder in parallel and builds a size tree of its subfolders.

use std::collections::HashMap;
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;

use super::largest::{FileInfo, TopFiles};
use crate::fsx::{self, InodeSet};

#[derive(Debug, Default, Clone)]
struct DirNode {
    own_bytes: u64,
    own_files: u64,
    bytes: u64,
    files: u64,
    subdirs: Vec<PathBuf>,
}

#[derive(Debug, Default, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub root: String,
    pub bytes: u64,
    pub files: u64,
    pub dirs: u64,
    /// Folders that could not be read (often missing Full Disk Access).
    pub errors: u64,
    /// Mount points of other volumes that were not entered.
    pub other_volumes: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct Progress {
    pub files: u64,
    pub bytes: u64,
    pub current: String,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Child {
    pub path: String,
    pub name: String,
    pub bytes: u64,
    pub files: u64,
    /// "dir", or "files" for the loose files directly in the folder.
    pub kind: &'static str,
    pub has_children: bool,
}

pub struct Tree {
    pub root: PathBuf,
    pub stats: ScanStats,
    pub largest: Vec<FileInfo>,
    nodes: HashMap<PathBuf, DirNode>,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("cannot read {0}: {1}")]
    Root(String, std::io::Error),
    #[error("scan cancelled")]
    Cancelled,
}

type Walk = jwalk::WalkDirGeneric<((), Option<Metadata>)>;

pub fn scan(
    root: &Path,
    top_n: usize,
    cancel: &AtomicBool,
    on_progress: impl Fn(Progress),
) -> Result<Tree, ScanError> {
    let started = Instant::now();
    let root_meta = root
        .symlink_metadata()
        .map_err(|e| ScanError::Root(root.display().to_string(), e))?;
    let root_dev = root_meta.dev();
    let other_volumes = Arc::new(AtomicU64::new(0));
    let other_volumes_in_walk = Arc::clone(&other_volumes);

    let walk = Walk::new(root)
        .follow_links(false)
        .skip_hidden(false)
        .sort(false)
        .process_read_dir(move |_, _, _, children| {
            for child in children.iter_mut().flatten() {
                let meta = child.path().symlink_metadata().ok();
                if let Some(m) = &meta
                    && child.file_type.is_dir()
                    && m.dev() != root_dev
                {
                    child.read_children_path = None;
                    other_volumes_in_walk.fetch_add(1, Ordering::Relaxed);
                }
                child.client_state = meta;
            }
        });

    let mut nodes: HashMap<PathBuf, DirNode> = HashMap::new();
    nodes.insert(root.to_path_buf(), DirNode::default());
    let mut stats = ScanStats {
        root: root.display().to_string(),
        ..Default::default()
    };
    let mut seen = InodeSet::default();
    let mut top = TopFiles::new(top_n);
    let mut since_progress = 0u32;

    for entry in walk {
        if cancel.load(Ordering::Relaxed) {
            return Err(ScanError::Cancelled);
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                stats.errors += 1;
                continue;
            }
        };
        if entry.depth == 0 {
            continue;
        }
        let path = entry.path();
        let parent = entry.parent_path.to_path_buf();
        if entry.file_type.is_dir() {
            stats.dirs += 1;
            nodes.entry(path.clone()).or_default();
            nodes.entry(parent).or_default().subdirs.push(path);
            continue;
        }
        let Some(meta) = entry.client_state.as_ref() else {
            stats.errors += 1;
            continue;
        };
        if meta.dev() != root_dev || !seen.first_sight(meta) {
            continue;
        }
        let bytes = fsx::allocated(meta);
        let node = nodes.entry(parent).or_default();
        node.own_bytes += bytes;
        node.own_files += 1;
        stats.files += 1;
        stats.bytes += bytes;
        if meta.is_file() {
            top.push(bytes, &path, fsx::last_used_secs(meta));
        }
        since_progress += 1;
        if since_progress >= 25_000 {
            since_progress = 0;
            on_progress(Progress {
                files: stats.files,
                bytes: stats.bytes,
                current: path.display().to_string(),
            });
        }
    }

    roll_up(&mut nodes);
    stats.other_volumes = other_volumes.load(Ordering::Relaxed);
    stats.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(Tree {
        root: root.to_path_buf(),
        stats,
        largest: top.into_vec(),
        nodes,
    })
}

/// Fills each folder's total from its own files plus its subfolders, deepest first.
fn roll_up(nodes: &mut HashMap<PathBuf, DirNode>) {
    let mut order: Vec<PathBuf> = nodes.keys().cloned().collect();
    order.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for path in order {
        let (mut bytes, mut files) = {
            let node = &nodes[&path];
            (node.own_bytes, node.own_files)
        };
        for sub in &nodes[&path].subdirs {
            if let Some(child) = nodes.get(sub) {
                bytes += child.bytes;
                files += child.files;
            }
        }
        let node = nodes.get_mut(&path).expect("node exists");
        node.bytes = bytes;
        node.files = files;
    }
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

impl Tree {
    pub fn node(&self, path: &Path) -> Option<Child> {
        let node = self.nodes.get(path)?;
        Some(Child {
            path: path.display().to_string(),
            name: name_of(path),
            bytes: node.bytes,
            files: node.files,
            kind: "dir",
            has_children: !node.subdirs.is_empty() || node.own_files > 0,
        })
    }

    /// Subfolders of `path`, largest first, plus one entry for its loose files.
    pub fn children(&self, path: &Path) -> Option<Vec<Child>> {
        let node = self.nodes.get(path)?;
        let mut out: Vec<Child> = node
            .subdirs
            .iter()
            .filter_map(|sub| self.node(sub))
            .filter(|c| c.bytes > 0)
            .collect();
        if node.own_files > 0 {
            out.push(Child {
                path: path.display().to_string(),
                name: format!("{} files", node.own_files),
                bytes: node.own_bytes,
                files: node.own_files,
                kind: "files",
                has_children: false,
            });
        }
        out.sort_by_key(|c| std::cmp::Reverse(c.bytes));
        Some(out)
    }

    /// Takes a trashed path out of the tree and subtracts its size from every parent.
    pub fn forget(&mut self, path: &Path) {
        let (bytes, files) = if let Some(node) = self.nodes.get(path) {
            (node.bytes, node.files)
        } else if let Some(f) = self.largest.iter().find(|f| Path::new(&f.path) == path) {
            (f.bytes, 1)
        } else {
            return;
        };
        let doomed: Vec<PathBuf> = self
            .nodes
            .keys()
            .filter(|p| p.starts_with(path))
            .cloned()
            .collect();
        for p in &doomed {
            self.nodes.remove(p);
        }
        self.largest
            .retain(|f| !Path::new(&f.path).starts_with(path));
        if let Some(parent) = path.parent()
            && let Some(node) = self.nodes.get_mut(parent)
        {
            node.subdirs.retain(|s| s != path);
            if doomed.is_empty() {
                node.own_bytes = node.own_bytes.saturating_sub(bytes);
                node.own_files = node.own_files.saturating_sub(files);
            }
        }
        for ancestor in path.ancestors().skip(1) {
            if let Some(node) = self.nodes.get_mut(ancestor) {
                node.bytes = node.bytes.saturating_sub(bytes);
                node.files = node.files.saturating_sub(files);
            }
            if ancestor == self.root {
                break;
            }
        }
        self.stats.bytes = self.stats.bytes.saturating_sub(bytes);
        self.stats.files = self.stats.files.saturating_sub(files);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, len: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![1u8; len]).unwrap();
    }

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        write(&root.join("big/a.bin"), 512 * 1024);
        write(&root.join("big/deep/b.bin"), 256 * 1024);
        write(&root.join("small/c.txt"), 4096);
        write(&root.join("loose.txt"), 4096);
        (dir, root)
    }

    #[test]
    fn totals_roll_up_and_children_sort_by_size() {
        let (_d, root) = fixture();
        let tree = scan(&root, 10, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(tree.stats.files, 4);
        assert_eq!(tree.stats.dirs, 3);
        let top = tree.node(&root).unwrap();
        assert_eq!(top.bytes, tree.stats.bytes);
        let kids = tree.children(&root).unwrap();
        assert_eq!(kids[0].name, "big");
        assert!(kids[0].bytes >= 768 * 1024);
        assert!(kids.iter().any(|c| c.kind == "files" && c.files == 1));
        assert_eq!(tree.largest[0].name, "a.bin");
    }

    #[test]
    fn forget_subtracts_from_parents() {
        let (_d, root) = fixture();
        let mut tree = scan(&root, 10, &AtomicBool::new(false), |_| {}).unwrap();
        let before = tree.node(&root).unwrap().bytes;
        let deep = tree.node(&root.join("big/deep")).unwrap().bytes;
        tree.forget(&root.join("big/deep"));
        assert_eq!(tree.node(&root).unwrap().bytes, before - deep);
        assert!(tree.node(&root.join("big/deep")).is_none());
        assert!(tree.largest.iter().all(|f| f.name != "b.bin"));
    }

    #[test]
    fn cancel_stops_the_scan() {
        let (_d, root) = fixture();
        let res = scan(&root, 10, &AtomicBool::new(true), |_| {});
        assert!(matches!(res, Err(ScanError::Cancelled)));
    }
}
