use std::{path::PathBuf, sync::LazyLock};
#[cfg(any(windows, test))]
use std::{fs, io, path::Path};

use anyhow::{Context, Result};

pub(crate) fn index_override() -> Option<PathBuf> {
    std::env::var_os("INDEXRAY_INDEX_DIR").map(PathBuf::from)
}

pub(crate) fn index_dir() -> Result<PathBuf> {
    if let Some(path) = index_override() {
        return Ok(path);
    }
    static DATA_DIR: LazyLock<Result<PathBuf>> = LazyLock::new(default_data_dir);
    match &*DATA_DIR {
        Ok(dir) => Ok(dir.join("index")),
        Err(err) => anyhow::bail!("{err:#}"),
    }
}

fn default_data_dir() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "IndexRay")
        .context("IndexRay data directory is unavailable")?;
    let target = dirs.data_local_dir();
    #[cfg(windows)]
    {
        let old = directories::ProjectDirs::from("", "", "Folioseek")
            .context("Legacy data directory is unavailable")?;
        migrate_data(old.data_local_dir(), target)?;
    }
    Ok(target.to_owned())
}

#[cfg(any(windows, test))]
fn migrate_data(old: &Path, target: &Path) -> Result<()> {
    // Never merge two Tantivy indexes or replace an existing IndexRay profile.
    if target.try_exists()? || !old.try_exists()? {
        return Ok(());
    }
    fs::create_dir_all(target.parent().context("Invalid IndexRay data path")?)?;
    match fs::rename(old, target) {
        Ok(()) => Ok(()),
        // Another IndexRay process may have completed the same move.
        Err(err) if matches!(err.kind(), io::ErrorKind::NotFound | io::ErrorKind::AlreadyExists)
            && target.try_exists()? => Ok(()),
        Err(err) => Err(err).with_context(|| format!(
            "Close the previous app before moving its data from {} to {}",
            old.display(), target.display(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_moves_the_profile_once_and_preserves_existing_indexray_data() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let old = dir.path().join("old/data");
        let target = dir.path().join("new/data");
        fs::create_dir_all(old.join("index"))?;
        fs::write(old.join("index/meta.json"), "index")?;
        fs::write(old.join("config.json"), "settings")?;
        migrate_data(&old, &target)?;
        assert!(!old.exists());
        assert_eq!(fs::read_to_string(target.join("index/meta.json"))?, "index");
        fs::create_dir_all(&old)?;
        fs::write(old.join("config.json"), "other settings")?;
        migrate_data(&old, &target)?;
        assert_eq!(fs::read_to_string(target.join("config.json"))?, "settings");
        assert_eq!(fs::read_to_string(old.join("config.json"))?, "other settings");
        Ok(())
    }
}
