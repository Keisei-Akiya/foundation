//! Pre-commit helper: auto-formats staged Rust files and checks the staged snapshot.
//!
//! Run from the repository root with `cargo run --quiet --locked --bin precommit --`.
//! Fixes are written to index blobs only. A worktree file is rewritten only when it
//! matched its index blob before fixing. The real index and worktree change only after
//! every check passes.

use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{self, Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

type Bytes = Vec<u8>;

const CHECKS: &[&[&str]] = &[
    &["fmt", "--all", "--", "--check"],
    &[
        "clippy",
        "--all-targets",
        "--all-features",
        "--locked",
        "--",
        "-D",
        "warnings",
    ],
];

struct Repo {
    root: PathBuf,
    index: PathBuf,
    target: PathBuf,
}

struct Entry {
    mode: Bytes,
    blob: Bytes,
}

struct Fix {
    path: Bytes,
    mode: Bytes,
    new_blob: Bytes,
    contents: Bytes,
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Result<Self, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = env::temp_dir().join(format!("precommit-{label}-{}-{nanos}", process::id()));
        fs::create_dir(&path).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
        Ok(Self { path })
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl Repo {
    fn run(&self, args: &[&OsStr], input: Option<&[u8]>) -> Result<Bytes, String> {
        git(&self.root, Some(&self.index), args, input)
    }
}

fn main() -> ExitCode {
    match precommit() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("precommit: {message}");
            ExitCode::FAILURE
        }
    }
}

fn precommit() -> Result<(), String> {
    let cwd = env::current_dir().map_err(|e| format!("cannot read current directory: {e}"))?;
    let root = PathBuf::from(line(&git(
        &cwd,
        None,
        &[os("rev-parse"), os("--show-toplevel")],
        None,
    )?));
    let index = match env::var_os("GIT_INDEX_FILE") {
        Some(index) => cwd.join(index),
        None => root.join(line(&git(
            &root,
            None,
            &[os("rev-parse"), os("--git-path"), os("index")],
            None,
        )?)),
    };
    let target = cwd.join(
        env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| root.join("target").into_os_string()),
    );
    let repo = Repo {
        root,
        index,
        target,
    };

    let entries = index_entries(&repo)?;
    let staged = split_nul(&repo.run(
        &[
            os("diff"),
            os("--cached"),
            os("--no-renames"),
            os("--name-only"),
            os("-z"),
            os("--diff-filter=ACMR"),
        ],
        None,
    )?);
    let rust_paths: Vec<Bytes> = staged
        .into_iter()
        .filter(|path| path.ends_with(b".rs"))
        .collect();
    let unstaged: HashSet<Bytes> =
        split_nul(&repo.run(&[os("diff-files"), os("--name-only"), os("-z")], None)?)
            .into_iter()
            .collect();

    let fixes = format_staged(&repo, &rust_paths, &entries)?;
    verify(&repo, &fixes)?;
    let mirrored = apply(&repo, &fixes, &unstaged)?;
    eprintln!(
        "precommit: checks passed; formatted {} staged Rust file(s), {mirrored} also updated in worktree",
        fixes.len()
    );
    Ok(())
}

/// Formats staged Rust files in an exported copy of the index and collects changed blobs.
fn format_staged(
    repo: &Repo,
    paths: &[Bytes],
    entries: &HashMap<Bytes, Entry>,
) -> Result<Vec<Fix>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let work = TempDir::new("format")?;
    export(repo, &work.path)?;
    cargo(&work.path, &repo.target, &["fmt", "--all"])?;

    let mut fixes = Vec::new();
    for path in paths {
        let entry = entries
            .get(path)
            .ok_or_else(|| format!("staged file missing from index: {}", show(path)))?;
        if entry.mode != b"100644" && entry.mode != b"100755" {
            continue;
        }
        let contents = fs::read(work.path.join(OsStr::from_bytes(path)))
            .map_err(|e| format!("cannot read formatted {}: {e}", show(path)))?;
        let mut path_arg = OsString::from("--path=");
        path_arg.push(OsStr::from_bytes(path));
        let new_blob = trim_newline(&repo.run(
            &[
                os("hash-object"),
                os("-w"),
                os("--stdin"),
                path_arg.as_os_str(),
            ],
            Some(&contents),
        )?)
        .to_vec();
        if new_blob != entry.blob {
            fixes.push(Fix {
                path: path.clone(),
                mode: entry.mode.clone(),
                new_blob,
                contents,
            });
        }
    }
    Ok(fixes)
}

/// Runs the project checks against the index as it would be after the fixes.
fn verify(repo: &Repo, fixes: &[Fix]) -> Result<(), String> {
    let work = TempDir::new("verify")?;
    let candidate = Repo {
        root: repo.root.clone(),
        index: work.path.join("index"),
        target: repo.target.clone(),
    };
    if repo.index.exists() {
        fs::copy(&repo.index, &candidate.index).map_err(|e| format!("cannot copy index: {e}"))?;
    }
    if !fixes.is_empty() {
        candidate.run(
            &[os("update-index"), os("-z"), os("--index-info")],
            Some(&index_info(fixes)),
        )?;
    }

    let snapshot = work.path.join("snapshot");
    fs::create_dir(&snapshot).map_err(|e| format!("cannot create snapshot: {e}"))?;
    export(&candidate, &snapshot)?;
    for args in CHECKS {
        cargo(&snapshot, &repo.target, args)?;
    }
    Ok(())
}

/// Updates the real index, then mirrors fixes into fully staged worktree files.
/// Returns the number of worktree files mirrored.
fn apply(repo: &Repo, fixes: &[Fix], unstaged: &HashSet<Bytes>) -> Result<usize, String> {
    let mirrored: Vec<&Fix> = fixes
        .iter()
        .filter(|fix| !unstaged.contains(&fix.path))
        .collect();
    let mut originals = Vec::new();
    for fix in &mirrored {
        let path = repo.root.join(OsStr::from_bytes(&fix.path));
        let original =
            fs::read(&path).map_err(|e| format!("cannot read {}: {e}", show(&fix.path)))?;
        originals.push((path, original));
    }

    let result = mirrored
        .iter()
        .try_for_each(|fix| {
            let path = repo.root.join(OsStr::from_bytes(&fix.path));
            fs::write(&path, &fix.contents)
                .map_err(|e| format!("cannot write {}: {e}", show(&fix.path)))
        })
        .and_then(|()| {
            repo.run(
                &[os("update-index"), os("-z"), os("--index-info")],
                Some(&index_info(fixes)),
            )
            .map(|_| ())
        });
    if let Err(message) = result {
        for (path, original) in originals {
            let _ = fs::write(path, original);
        }
        return Err(message);
    }
    Ok(mirrored.len())
}

fn export(repo: &Repo, dest: &Path) -> Result<(), String> {
    let mut prefix = OsString::from("--prefix=");
    prefix.push(dest);
    prefix.push("/");
    repo.run(
        &[os("checkout-index"), os("--all"), prefix.as_os_str()],
        None,
    )
    .map(|_| ())
}

fn index_entries(repo: &Repo) -> Result<HashMap<Bytes, Entry>, String> {
    let listing = repo.run(&[os("ls-files"), os("--stage"), os("-z")], None)?;
    let mut entries = HashMap::new();
    for record in listing.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let tab = record
            .iter()
            .position(|b| *b == b'\t')
            .ok_or("malformed ls-files output")?;
        let fields: Vec<&[u8]> = record[..tab].split(|b| *b == b' ').collect();
        let (mode, blob, stage) = match fields[..] {
            [mode, blob, stage] => (mode, blob, stage),
            _ => return Err("malformed ls-files output".into()),
        };
        let path = &record[tab + 1..];
        if stage != b"0" {
            return Err(format!("unresolved conflict in {}", show(path)));
        }
        entries.insert(
            path.to_vec(),
            Entry {
                mode: mode.to_vec(),
                blob: blob.to_vec(),
            },
        );
    }
    Ok(entries)
}

fn index_info(fixes: &[Fix]) -> Bytes {
    let mut info = Vec::new();
    for fix in fixes {
        info.extend_from_slice(&fix.mode);
        info.push(b' ');
        info.extend_from_slice(&fix.new_blob);
        info.push(b'\t');
        info.extend_from_slice(&fix.path);
        info.push(0);
    }
    info
}

fn cargo(dir: &Path, target: &Path, args: &[&str]) -> Result<(), String> {
    let status = Command::new("cargo")
        .args(args)
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", target)
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "`cargo {}` failed in staged snapshot ({status})",
            args.join(" ")
        ))
    }
}

fn git(
    dir: &Path,
    index: Option<&Path>,
    args: &[&OsStr],
    input: Option<&[u8]>,
) -> Result<Bytes, String> {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    if let Some(index) = index {
        command.env("GIT_INDEX_FILE", index);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if let Some(input) = input {
        child
            .stdin
            .take()
            .ok_or("git stdin is unavailable")?
            .write_all(input)
            .map_err(|e| format!("cannot write to git: {e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("cannot wait for git: {e}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let command = args
            .first()
            .map_or(String::new(), |a| a.to_string_lossy().into_owned());
        Err(format!("git {command} failed ({})", output.status))
    }
}

fn os(text: &str) -> &OsStr {
    OsStr::new(text)
}

fn line(bytes: &[u8]) -> OsString {
    OsStr::from_bytes(trim_newline(bytes)).to_os_string()
}

fn trim_newline(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

fn split_nul(bytes: &[u8]) -> Vec<Bytes> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(<[u8]>::to_vec)
        .collect()
}

fn show(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
