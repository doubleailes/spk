// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

use std::collections::HashSet;

use relative_path::RelativePathBuf;

use super::{Entry, EntryKind, Manifest};

#[cfg(test)]
#[path = "./diff_test.rs"]
mod diff_test;

/// Identifies a difference between two file system entries
#[derive(Debug, Eq, PartialEq, Clone, strum::EnumDiscriminants)]
pub enum DiffMode<U1 = (), U2 = ()> {
    Unchanged(Entry<U1>),
    Changed(Entry<U1>, Entry<U2>),
    Added(Entry<U2>),
    Removed(Entry<U1>),
}

impl<U1, U2> std::fmt::Display for DiffMode<U1, U2> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unchanged(..) => f.write_str("="),
            Self::Changed(..) => f.write_str("~"),
            Self::Added(..) => f.write_str("+"),
            Self::Removed(..) => f.write_str("-"),
        }
    }
}

impl<U1, U2> DiffMode<U1, U2> {
    pub fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged(..))
    }
    pub fn is_changed(&self) -> bool {
        matches!(self, Self::Changed(..))
    }
    pub fn is_added(&self) -> bool {
        matches!(self, Self::Added(..))
    }
    pub fn is_removed(&self) -> bool {
        matches!(self, Self::Removed(..))
    }

    /// True if the underlying entry is/was a directory.
    ///
    /// In the case of a change, both the original and final
    /// entries must be directories for this to return true.
    pub fn is_dir(&self) -> bool {
        match self {
            DiffMode::Unchanged(entry) => entry.is_dir(),
            DiffMode::Changed(a, b) => a.is_dir() && b.is_dir(),
            DiffMode::Added(entry) => entry.is_dir(),
            DiffMode::Removed(entry) => entry.is_dir(),
        }
    }
}

impl<U1> DiffMode<U1, U1> {
    /// The associated user data from the underlying entry.
    ///
    /// In the case of a [`Self::Changed`] entry, the original entry
    /// data is returned.
    pub fn user_data(&self) -> &U1 {
        match self {
            DiffMode::Unchanged(entry) => &entry.user_data,
            DiffMode::Changed(a, _) => &a.user_data,
            DiffMode::Added(entry) => &entry.user_data,
            DiffMode::Removed(entry) => &entry.user_data,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Diff<U1 = (), U2 = ()> {
    pub mode: DiffMode<U1, U2>,
    pub path: RelativePathBuf,
}

impl<U1, U2> std::fmt::Display for Diff<U1, U2> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{} {}{}",
            self.mode,
            self.path,
            self.details()
        ))
    }
}

impl<U1, U2> Diff<U1, U2> {
    fn details(&self) -> String {
        let mut details = String::new();
        if let DiffMode::Changed(a, b) = &self.mode {
            if a.mode != b.mode {
                details = format!("{details} {{{:06o} => {:06o}}}", a.mode, b.mode);
            }
            if a.kind != b.kind {
                details = format!("{details} {{{} => {}}}", a.kind, b.kind);
            }
            if a.object != b.object {
                details = format!("{details} {{!content!}}");
            }
        }
        details
    }
}

pub fn compute_diff<U1: Clone>(a: &Manifest<U1>, b: &Manifest<U1>) -> Vec<Diff<U1, U1>> {
    let mut changes = Vec::new();
    let mut all_entries: Vec<_> = a.walk().chain(b.walk()).collect();
    all_entries.sort();

    let mut visited = HashSet::new();
    for entry in all_entries.iter() {
        if !visited.insert(&entry.path) {
            continue;
        }
        match diff_path(a, b, &entry.path) {
            DiffPathResult::Diff(d) => changes.push(d),
            DiffPathResult::PathMissingFromBothManifests => tracing::debug!(path = ?entry.path,
                "path was missing from both manifests during diff, this should be impossible"
            ),
            DiffPathResult::UselessMask => tracing::debug!(path = ?entry.path,
                "path was masked in the right manifest but didn't exist in the left"
            ),
        }
    }

    changes
}

// Allow: most instances will be of the large variant; boxing is
// counter-productive.
#[allow(clippy::large_enum_variant)]
enum DiffPathResult<U1, U2> {
    /// Successful diff
    Diff(Diff<U1, U2>),
    /// The path was missing from both manifests (coding error?)
    PathMissingFromBothManifests,
    /// The path was masked in the right manifest but didn't exist in the left
    UselessMask,
}

/// Compares the two entries, creating a diff to represent their delta.
///
/// In the case of no change, the entry from `a` is returned.
fn diff_path<U1: Clone, U2: Clone>(
    a: &Manifest<U1>,
    b: &Manifest<U2>,
    path: &RelativePathBuf,
) -> DiffPathResult<U1, U2> {
    match (a.get_path(path), b.get_path(path)) {
        (None, None) => DiffPathResult::PathMissingFromBothManifests,

        (Some(a_entry), Some(b_entry)) if b_entry.kind == EntryKind::Mask => {
            DiffPathResult::Diff(Diff {
                mode: DiffMode::Removed(a_entry.clone()),
                path: path.clone(),
            })
        }

        (None, Some(b_entry)) if b_entry.kind == EntryKind::Mask => {
            debug_assert!(
                false,
                "detected a mask entry that deletes something that doesn't exist"
            );
            // Can't return `a` as documented since the left side is None.
            DiffPathResult::UselessMask
        }

        (None, Some(e)) => DiffPathResult::Diff(Diff {
            mode: DiffMode::Added(e.clone()),
            path: path.clone(),
        }),

        (Some(e), None) => DiffPathResult::Diff(Diff {
            mode: DiffMode::Removed(e.clone()),
            path: path.clone(),
        }),

        (Some(a_entry), Some(b_entry)) => DiffPathResult::Diff({
            if a_entry == b_entry {
                Diff {
                    // use the entry from `a` as it's more representative
                    // of the unchanged item (in the case of the user data
                    // being different, one would expect the original value)
                    mode: DiffMode::Unchanged(a_entry.clone()),
                    path: path.clone(),
                }
            } else {
                Diff {
                    mode: DiffMode::Changed(a_entry.clone(), b_entry.clone()),
                    path: path.clone(),
                }
            }
        }),
    }
}
