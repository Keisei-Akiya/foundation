use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

fn locate(root: &Path, name: &str) -> Option<PathBuf> {
    let local = root.join("node_modules/.bin").join(name);
    local.is_file().then_some(local)
}
fn execute(bin: &Path, args: &[&str], cwd: &Path) -> Option<String> {
    let timeout = locate(cwd, "timeout").or_else(|| {
        ["/usr/bin/timeout", "/bin/timeout"]
            .iter()
            .map(PathBuf::from)
            .find(|p| p.is_file())
    });
    let mut cmd = Command::new(timeout.as_deref().unwrap_or(Path::new("timeout")));
    if timeout.is_some() {
        cmd.arg("5s");
    }
    let out = cmd.arg(bin).args(args).current_dir(cwd).output().ok()?;
    if out.status.success() {
        None
    } else {
        Some(
            String::from_utf8_lossy(&out.stderr)
                .chars()
                .take(1000)
                .collect(),
        )
    }
}
pub fn run(payload: &Value) {
    let Some(obj) = payload.as_object() else {
        return;
    };
    let root = Path::new(obj.get("cwd").and_then(Value::as_str).unwrap_or(""));
    let Some(paths) = obj.get("changed_paths").and_then(Value::as_array) else {
        return;
    };
    if !root.is_dir() {
        return;
    }
    let files: Vec<String> = paths
        .iter()
        .filter_map(Value::as_str)
        .filter(|s| {
            let p = Path::new(s);
            !p.is_absolute()
                && !p
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                && ["js", "jsx", "ts", "tsx", "mjs", "cjs"]
                    .contains(&p.extension().and_then(|e| e.to_str()).unwrap_or(""))
                && root.join(p).is_file()
                && !root.join(p).is_symlink()
        })
        .take(128)
        .map(str::to_owned)
        .collect();
    if files.is_empty() {
        return;
    }
    let mut issues = Vec::new();
    if let Some(bin) = locate(root, "biome") {
        let refs: Vec<&str> = files.iter().map(String::as_str).collect();
        let mut args = vec!["check", "--write"];
        args.extend(refs);
        if let Some(error) = execute(&bin, &args, root) {
            issues.push(format!("Biome reported issues: {error}"));
        }
    } else if let Some(bin) = locate(root, "prettier") {
        let refs: Vec<&str> = files.iter().map(String::as_str).collect();
        let mut args = vec!["--write"];
        args.extend(refs);
        if let Some(error) = execute(&bin, &args, root) {
            issues.push(format!("Prettier reported issues: {error}"));
        }
    }
    if files
        .iter()
        .any(|f| f.ends_with(".ts") || f.ends_with(".tsx"))
    {
        if let Some(bin) = locate(root, "tsc") {
            if let Some(error) = execute(&bin, &["--noEmit"], root) {
                issues.push(format!("TypeScript reported issues: {error}"));
            }
        }
    }
    if !issues.is_empty() {
        println!(
            "{}",
            json!({"action":"continue", "message":issues.join("\n")})
        );
    }
}
