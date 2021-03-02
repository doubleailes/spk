use std::path::{Path, PathBuf};

use super::read_last_migration_version;
use crate::Result;

type MigrationFn = dyn (Fn(&PathBuf, &PathBuf) -> Result<()>) + Sync;

static MIGRATIONS: Vec<(&str, &MigrationFn)> = vec![];

/// Migrate a repository to the latest version and replace the existing data.
pub fn upgrade_repo<P: AsRef<Path>>(root: P) -> Result<PathBuf> {
    let root = root.as_ref().canonicalize()?;
    let repo_name = match &root.file_name() {
        None => return Err("Repository path must have a file name".into()),
        Some(name) => name.to_string_lossy(),
    };
    tracing::info!("migrating data...");
    let migrated_path = migrate_repo(&root)?;
    tracing::info!("swapping out migrated data...");
    let backup_path = root.with_file_name(format!("{}-backup", repo_name));
    std::fs::rename(&root, &backup_path)?;
    std::fs::rename(&migrated_path, &root)?;
    tracing::info!("purging old data...");
    std::fs::remove_dir_all(backup_path)?;
    Ok(root)
}

/// Migrate a repository at the given path to the latest version.
///
/// # Returns:
///    - the path to the migrated repo data
pub fn migrate_repo<P: AsRef<Path>>(root: P) -> Result<PathBuf> {
    let mut root = root.as_ref().canonicalize()?;
    let last_migration = read_last_migration_version(&root)?;
    let repo_name = match &root.file_name() {
        None => return Err("Repository path must have a file name".into()),
        Some(name) => name.to_string_lossy().to_string(),
    };

    for (version, migration_func) in MIGRATIONS.iter() {
        let version = semver::Version::parse(version).unwrap();
        if last_migration.major >= version.major {
            tracing::info!(
                "skip unnecessary migration [{:?} >= {:?}]",
                last_migration,
                version
            );
            continue;
        }

        let migrated_path = root.with_file_name(format!("{}-{}", repo_name, version.to_string()));
        if migrated_path.exists() {
            return Err(format!("found existing migration data: {:?}", migrated_path).into());
        }
        tracing::info!("migrating data from {} to {}...", last_migration, version);
        migration_func(&root, &migrated_path)?;
        root = root.with_file_name(format!("{}-migrated", repo_name));
        std::fs::rename(&migrated_path, &root)?;
    }

    Ok(root)
}
