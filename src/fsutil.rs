use std::io::Write;
use std::path::Path;

/// Replace a file atomically after its complete new contents are durable.
///
/// The temporary file lives beside the destination so the final rename stays
/// on one filesystem. Readers therefore observe either the old complete file
/// or the new complete file, never a truncated intermediate state.
pub(crate) fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;

    let existing_permissions = std::fs::metadata(path).ok().map(|m| m.permissions());
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| {
        format!(
            "failed to create temporary file in {}: {e}",
            parent.display()
        )
    })?;
    if let Some(permissions) = existing_permissions {
        temp.as_file()
            .set_permissions(permissions)
            .map_err(|e| format!("failed to preserve permissions for {}: {e}", path.display()))?;
    }
    temp.write_all(contents)
        .map_err(|e| format!("failed to write temporary file for {}: {e}", path.display()))?;
    temp.flush()
        .map_err(|e| format!("failed to flush temporary file for {}: {e}", path.display()))?;
    temp.as_file()
        .sync_all()
        .map_err(|e| format!("failed to sync temporary file for {}: {e}", path.display()))?;
    temp.persist(path)
        .map_err(|e| format!("failed to replace {}: {}", path.display(), e.error))?;

    // Persist the directory entry as well on platforms that support syncing a
    // directory file descriptor.
    #[cfg(unix)]
    std::fs::File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| format!("failed to sync {}: {e}", parent.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.txt");
        std::fs::write(&path, "old\n").unwrap();

        atomic_write(&path, b"new\n").unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), "new\n");
    }
}
