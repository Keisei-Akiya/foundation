use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};

/// Shared, per-plugin state store rooted at `$HERMES_HOME/plugin-data/foundation`.
#[derive(Debug, Clone)]
pub struct StateStore {
    root: PathBuf,
}

impl StateStore {
    pub fn new(hermes_home: impl AsRef<Path>) -> io::Result<Self> {
        let root = hermes_home.as_ref().join("plugin-data/foundation");
        fs::create_dir_all(&root)?;
        set_dir_permissions(&root)?;
        Ok(Self { root })
    }

    /// Resolve the configured home without falling back to the user's real home.
    pub fn from_env() -> io::Result<Self> {
        Self::from_home_env(std::env::var_os("HERMES_HOME"))
    }

    fn from_home_env(home: Option<std::ffi::OsString>) -> io::Result<Self> {
        let home = home
            .filter(|value| !value.is_empty())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HERMES_HOME is not set"))?;
        Self::new(home)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn file_path(&self, name: &str) -> io::Result<PathBuf> {
        validate_name(name)?;
        Ok(self.root.join(format!("{name}.jsonl")))
    }

    /// Append one serialized record while holding an exclusive lock on its JSONL file.
    /// Cooperating writers cannot interleave records, and readers take a shared lock to
    /// observe only complete records. A trailing partial/corrupt line is ignored by `read`.
    pub fn append(&self, name: &str, record: &Value) -> io::Result<()> {
        let mut line = serde_json::to_vec(record).map_err(io::Error::other)?;
        line.push(b'\n');
        let path = self.file_path(name)?;
        let mut options = OpenOptions::new();
        options.create(true).append(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path)?;
        set_file_permissions(&path)?;
        file.lock()?;
        let result = (|| {
            file.write_all(&line)?;
            file.flush()
        })();
        let unlock_result = File::unlock(&file);
        result.and(unlock_result)
    }

    /// Read valid JSONL records in order, skipping malformed lines.
    pub fn read(&self, name: &str) -> io::Result<Vec<Value>> {
        let path = self.file_path(name)?;
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        file.lock_shared()?;
        let mut records = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if let Ok(value) = serde_json::from_str(&line) {
                records.push(value);
            }
        }
        Ok(records)
    }

    /// Disabled marker always takes precedence; otherwise explicit opt-in overrides default.
    pub fn is_opted_in(&self, name: &str, default: bool) -> io::Result<bool> {
        validate_name(name)?;
        if is_disabled_at(&self.root, name) {
            return Ok(false);
        }
        if self.root.join(format!("{name}.enabled")).exists() {
            return Ok(true);
        }
        Ok(default)
    }
}

pub fn validate_name(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err(invalid_input("invalid state name"));
    }
    Ok(())
}

pub fn validate_session_id(session_id: &str) -> io::Result<()> {
    if session_id.is_empty()
        || session_id.len() > 128
        || !session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err(invalid_input("invalid session id"));
    }
    Ok(())
}

/// Validate an untrusted relative path before joining it beneath the state root.
pub fn validate_state_path(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty()
        || path.components().any(|component| {
            !matches!(component, Component::Normal(_))
                || matches!(component, Component::Normal(part) if part.to_string_lossy().is_empty())
        })
    {
        return Err(invalid_input("invalid relative state path"));
    }
    Ok(())
}

pub fn is_disabled(hermes_home: &Path, name: &str) -> bool {
    validate_name(name).is_ok() && is_disabled_at(&hermes_home.join("plugin-data/foundation"), name)
}

fn is_disabled_at(root: &Path, name: &str) -> bool {
    root.join(format!("{name}.disabled")).is_file()
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(unix)]
fn set_dir_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_dir_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_file_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{is_disabled, validate_name, validate_session_id, validate_state_path, StateStore};
    use serde_json::json;
    use std::fs;
    use std::path::Path;

    #[test]
    fn resolves_foundation_store_only_under_explicit_home() {
        let dir = tempfile_dir();
        let store = StateStore::new(&dir).unwrap();
        assert_eq!(store.root(), dir.join("plugin-data/foundation"));
        assert!(StateStore::from_home_env(None).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn appends_and_reads_jsonl_with_private_permissions() {
        let dir = tempfile_dir();
        let store = StateStore::new(&dir).unwrap();
        store.append("observations", &json!({"x": 1})).unwrap();
        store.append("observations", &json!({"x": 2})).unwrap();
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(store.file_path("observations").unwrap())
            .unwrap();
        writeln!(file, "{{broken").unwrap();
        assert_eq!(
            store.read("observations").unwrap(),
            vec![json!({"x": 1}), json!({"x": 2})]
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(store.file_path("observations").unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_traversal_in_store_and_session_paths() {
        assert!(validate_name("../outside").is_err());
        assert!(validate_session_id("../escape").is_err());
        assert!(validate_state_path(Path::new("sessions/../../outside")).is_err());
        assert!(validate_state_path(Path::new("sessions/abc.jsonl")).is_ok());
    }

    #[test]
    fn concurrent_appends_are_read_as_complete_records() {
        let dir = tempfile_dir();
        let store = StateStore::new(&dir).unwrap();
        let writers: Vec<_> = (0..4)
            .map(|writer| {
                let store = store.clone();
                std::thread::spawn(move || {
                    for record in 0..25 {
                        store
                            .append("events", &json!({"writer": writer, "record": record}))
                            .unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        assert_eq!(store.read("events").unwrap().len(), 100);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn detects_opt_in_and_disabled_markers_consistently() {
        let dir = tempfile_dir();
        let store = StateStore::new(&dir).unwrap();
        assert!(!is_disabled(&dir, "capture"));
        assert!(!store.is_opted_in("capture", false).unwrap());
        fs::write(store.root().join("capture.enabled"), "").unwrap();
        assert!(store.is_opted_in("capture", false).unwrap());
        fs::write(store.root().join("capture.disabled"), "").unwrap();
        assert!(is_disabled(&dir, "capture"));
        assert!(!store.is_opted_in("capture", true).unwrap());
        fs::remove_dir_all(dir).unwrap();
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "foundation-state-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
