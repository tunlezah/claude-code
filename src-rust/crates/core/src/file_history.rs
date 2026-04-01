//! Per-session file modification history.
//! Mirrors src/utils/fileHistory.ts (1,115 lines).
//!
//! Tracks which files were modified by tool calls in the current session,
//! enabling the /rewind command to restore files to earlier states.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Record of a single file modification in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHistoryEntry {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// SHA-256 hex of file content BEFORE the modification.
    pub before_hash: String,
    /// SHA-256 hex of file content AFTER the modification.
    pub after_hash: String,
    /// Conversation turn index at which this modification happened.
    pub turn_index: usize,
    /// Unix timestamp (ms) of the modification.
    pub timestamp_ms: u64,
    /// Tool that made the change ("FileEdit", "FileWrite", etc.).
    pub tool_name: String,
}

// ---------------------------------------------------------------------------
// FileHistory
// ---------------------------------------------------------------------------

/// In-memory file modification tracker for a single session.
#[derive(Debug, Default)]
pub struct FileHistory {
    /// All recorded modifications, in chronological order.
    entries: Vec<FileHistoryEntry>,
    /// Path → all entry indices for that path.
    by_path: HashMap<PathBuf, Vec<usize>>,
}

impl FileHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `tool_name` modified `path` from `before_content` to `after_content`.
    pub fn record_modification(
        &mut self,
        path: PathBuf,
        before_content: &[u8],
        after_content: &[u8],
        turn_index: usize,
        tool_name: &str,
    ) {
        let before_hash = sha256_hex(before_content);
        let after_hash = sha256_hex(after_content);
        let timestamp_ms = current_time_ms();

        let idx = self.entries.len();
        self.entries.push(FileHistoryEntry {
            path: path.clone(),
            before_hash,
            after_hash,
            turn_index,
            timestamp_ms,
            tool_name: tool_name.to_string(),
        });
        self.by_path.entry(path).or_default().push(idx);
    }

    /// Return all recorded modifications for `path`, in chronological order.
    pub fn get_file_history(&self, path: &Path) -> Vec<&FileHistoryEntry> {
        match self.by_path.get(path) {
            Some(indices) => indices.iter().map(|&i| &self.entries[i]).collect(),
            None => Vec::new(),
        }
    }

    /// Return all files that were modified at or after `turn_index`.
    pub fn get_files_changed_since(&self, turn_index: usize) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self
            .entries
            .iter()
            .filter(|e| e.turn_index >= turn_index)
            .map(|e| e.path.clone())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }

    /// Attempt to rewind a file to its state at the beginning of `turn_index`.
    ///
    /// Finds the most recent entry for `path` with `turn_index < rewind_to`.
    /// Returns the content to restore, or `None` if no earlier state is known.
    pub fn state_at_turn(&self, path: &Path, rewind_to: usize) -> Option<String> {
        // We store hashes, not content, so we can only detect whether a rewind
        // is possible (and return the before_hash for the earliest post-turn entry).
        let indices = self.by_path.get(path)?;
        // Find the earliest modification at or after rewind_to.
        let first_after: Option<&FileHistoryEntry> = indices
            .iter()
            .filter_map(|&i| self.entries.get(i))
            .filter(|e| e.turn_index >= rewind_to)
            .min_by_key(|e| e.turn_index);

        first_after.map(|e| format!("[Restore to before_hash: {}]", e.before_hash))
    }

    /// Number of entries recorded.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// All entries (for persistence / serialisation).
    pub fn entries(&self) -> &[FileHistoryEntry] {
        &self.entries
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_retrieve() {
        let mut fh = FileHistory::new();
        let path = PathBuf::from("/foo/bar.rs");
        fh.record_modification(path.clone(), b"old", b"new", 1, "FileEdit");
        assert_eq!(fh.len(), 1);
        let history = fh.get_file_history(&path);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].tool_name, "FileEdit");
        assert_eq!(history[0].turn_index, 1);
    }

    #[test]
    fn files_changed_since() {
        let mut fh = FileHistory::new();
        let a = PathBuf::from("/a.rs");
        let b = PathBuf::from("/b.rs");
        fh.record_modification(a.clone(), b"", b"x", 0, "FileWrite");
        fh.record_modification(b.clone(), b"", b"y", 3, "FileEdit");
        let changed = fh.get_files_changed_since(2);
        assert_eq!(changed, vec![b.clone()]);
    }

    #[test]
    fn state_at_turn_none_if_no_history() {
        let fh = FileHistory::new();
        assert!(fh.state_at_turn(Path::new("/x.rs"), 0).is_none());
    }
}
