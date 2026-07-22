use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;

/// Read a JSON file, falling back to Default on absence or corruption.
/// Corruption is reported to the caller via the returned flag so it can be logged
/// (the app must never fail to boot on a bad config file).
pub fn read_json<T: DeserializeOwned + Default>(path: &Path) -> (T, Option<String>) {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(v) => (v, None),
            Err(e) => (T::default(), Some(format!("corrupt {}: {e}", path.display()))),
        },
        Err(_) => (T::default(), None), // absent is normal on first run
    }
}

/// Atomic write: tmp file + fsync + rename. 0600 on unix.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(value)?;
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&data)?;
        f.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}
