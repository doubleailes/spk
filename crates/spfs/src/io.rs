use colored::*;

use super::config::load_config;
use crate::{encoding, storage, tracking, Result};

/// Return a nicely formatted string representation of the given reference.
pub fn format_digest<R: AsRef<str>>(
    reference: R,
    repo: Option<Box<dyn storage::Repository>>,
) -> Result<String> {
    let reference = reference.as_ref().to_string();
    let repo = match repo {
        Some(repo) => repo,
        None => {
            let config = load_config()?;
            Box::new(config.get_repository()?)
        }
    };

    let mut aliases: Vec<_> = match repo.find_aliases(reference.as_str()) {
        Ok(aliases) => aliases.into_iter().map(|r| r.to_string()).collect(),
        Err(crate::Error::InvalidReference(_)) => Default::default(),
        Err(err) => return Err(err),
    };

    let reference = if let Ok(digest) = encoding::parse_digest(&reference) {
        repo.get_shortened_digest(&digest)
    } else {
        reference
    };
    let mut all = vec![reference];
    all.append(&mut aliases);
    Ok(all.join(" -> "))
}

/// Return a human readable string rendering of the given diffs.
pub fn format_diffs(diffs: impl Iterator<Item = tracking::Diff>) -> String {
    let mut outputs = Vec::new();
    for diff in diffs {
        let mut abouts = Vec::new();
        match diff.entries {
            Some((a, b)) => {
                if a.mode != b.mode {
                    abouts.push("mode");
                }
                if a.object != b.object {
                    abouts.push("object");
                }
                if a.size != b.size {
                    abouts.push("size");
                }
            }
            None => (),
        }
        let about = if abouts.len() > 0 {
            format!(" [{}]", abouts.join(", ")).dimmed().to_string()
        } else {
            "".to_string()
        };
        let mut out = String::new();
        out += format!("{:>8}", diff.mode).bold().as_ref();
        out += format!("/spfs{}{}", diff.path, about).as_ref();
        let out = match diff.mode {
            tracking::DiffMode::Added => out.green(),
            tracking::DiffMode::Removed => out.red(),
            tracking::DiffMode::Changed => out.bright_blue(),
            _ => out.dimmed(),
        };
        outputs.push(out.to_string())
    }

    outputs.join("\n")
}

/// Return a string rendering of any given diffs which represent change.
pub fn format_changes(diffs: impl Iterator<Item = tracking::Diff>) -> String {
    format_diffs(diffs.filter(|x| if x.mode.is_unchanged() { false } else { true }))
}

/// Return a human-readable file size in bytes.
pub fn format_size(mut size: f64) -> String {
    for unit in &["B", "Ki", "Mi", "Gi", "Ti"] {
        if size.abs() < 1024.0 {
            return format!("{:3.1} {}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:3.1} Pi", size)
}
