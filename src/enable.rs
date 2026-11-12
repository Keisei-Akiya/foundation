//! `foundation enable`: register every foundation hook in Hermes.
//!
//! Appends a `hooks:` block to `<HERMES_HOME>/config.yaml` and records consent for exactly those
//! `(event, command)` pairs in `<HERMES_HOME>/shell-hooks-allowlist.json`.
//! It never touches an existing `hooks:` section and never sets `hooks_auto_accept`.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct HookDef {
    event: &'static str,
    sub: &'static str,
    matcher: &'static str,
    timeout: u32,
    /// Only meaningful for `pre_tool_call`: block the tool call when the hook itself fails.
    fail_closed: bool,
}

const HOOKS: &[HookDef] = &[
    HookDef {
        event: "pre_tool_call",
        sub: "preflight",
        matcher: "^(terminal|bash|Bash|powershell|PowerShell|powershell_tool|pwsh)$",
        timeout: 5,
        fail_closed: false,
    },
    HookDef {
        event: "pre_tool_call",
        sub: "hookify",
        matcher: "^(terminal|bash|Bash|read_file|write_file|patch|edit_file|apply_patch)$",
        timeout: 5,
        fail_closed: false,
    },
    HookDef {
        event: "pre_tool_call",
        sub: "config-protection",
        matcher: "^(write_file|edit_file|patch|apply_patch)$",
        timeout: 10,
        fail_closed: true,
    },
    HookDef {
        event: "post_tool_call",
        sub: "post-observer",
        matcher: "^(write_file|edit_file|patch|apply_patch|Edit|Write|MultiEdit)$",
        timeout: 10,
        fail_closed: false,
    },
    HookDef {
        event: "post_tool_call",
        sub: "lifecycle",
        matcher: "^skill$",
        timeout: 10,
        fail_closed: false,
    },
];

const ALLOWLIST_FILENAME: &str = "shell-hooks-allowlist.json";

fn command_for(bin: &str, sub: &str) -> String {
    format!("\"{bin}\" {sub}")
}

fn check_bin_path(bin: &str) -> Result<(), String> {
    if !bin.starts_with('/') {
        return Err(format!("binary path must be absolute: {bin}"));
    }
    if bin
        .chars()
        .any(|c| c == '"' || c == '\'' || c == '\\' || c.is_control())
    {
        return Err(format!(
            "binary path contains an unsupported character: {bin}"
        ));
    }
    Ok(())
}

/// Top-level `hooks:` key present (ignores comments and indented lines).
fn has_hooks_section(config: &str) -> bool {
    config.lines().any(|l| {
        let t = l.trim_end();
        t == "hooks:" || t.starts_with("hooks: ") || t.starts_with("hooks:#")
    })
}

/// One list entry of `hooks.<event>`, with `-` placed at column `pad`.
fn entry_lines(h: &HookDef, bin: &str, pad: usize) -> Vec<String> {
    let sp = " ".repeat(pad);
    vec![
        format!("{sp}- command: '{}'", command_for(bin, h.sub)),
        format!("{sp}  matcher: '{}'", h.matcher),
        format!("{sp}  timeout: {}", h.timeout),
        format!("{sp}  fail_closed: {}", h.fail_closed),
    ]
}

fn hooks_block(bin: &str) -> String {
    let mut out = String::from("hooks:\n");
    for event in ["pre_tool_call", "post_tool_call"] {
        out.push_str(&format!("  {event}:\n"));
        for h in HOOKS.iter().filter(|h| h.event == event) {
            for l in entry_lines(h, bin, 4) {
                out.push_str(&l);
                out.push('\n');
            }
        }
    }
    out
}

#[derive(Debug, PartialEq)]
enum ConfigPlan {
    /// All hook commands are already present; leave the file alone.
    AlreadyEnabled,
    /// Replace the file with this content (no `hooks:` section existed before).
    Write(String),
    /// Replace the file with this content, which adds the listed hooks (by subcommand)
    /// to the already existing `hooks:` section.
    Merge(String, Vec<&'static str>),
}

const UNMERGEABLE: &str = "Add the entries from hooks.example.yaml by hand.";

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_blank_or_comment(line: &str) -> bool {
    let t = line.trim();
    t.is_empty() || t.starts_with('#')
}

/// The value written after `key:` on the same line, without a trailing comment.
fn inline_value(rest: &str) -> &str {
    let r = rest.trim();
    if r.starts_with('#') {
        return "";
    }
    r.split(" #").next().unwrap_or("").trim()
}

fn is_empty_scalar(v: &str) -> bool {
    matches!(v, "{}" | "[]" | "null" | "~")
}

/// Exclusive end of the block opened by the key line at `start`: the line after the last
/// non-blank, non-comment line that is nested under it (or is a `- ` item at the same indent).
fn block_end(lines: &[String], start: usize, key_indent: usize) -> usize {
    let mut last = start + 1;
    for (i, l) in lines.iter().enumerate().skip(start + 1) {
        if is_blank_or_comment(l) {
            continue;
        }
        let ind = indent_of(l);
        if ind > key_indent || (ind == key_indent && l.trim_start().starts_with("- ")) {
            last = i + 1;
        } else {
            break;
        }
    }
    last
}

/// Line index of `<event>:` directly under the `hooks:` key at `h` (children at `child_indent`).
fn find_event(
    lines: &[String],
    h: usize,
    end: usize,
    child_indent: usize,
    event: &str,
) -> Option<usize> {
    (h + 1..end).find(|&i| {
        indent_of(&lines[i]) == child_indent
            && lines[i]
                .trim_start()
                .strip_prefix(event)
                .is_some_and(|r| r.starts_with(':'))
    })
}

/// Add the missing foundation hooks to the existing top-level `hooks:` key at line `h`.
/// Works on lines so comments and formatting elsewhere stay untouched.
fn merge_into_hooks(
    lines: &mut Vec<String>,
    h: usize,
    bin: &str,
    cr: &str,
) -> Result<Vec<&'static str>, String> {
    let unsupported = |what: &str| {
        format!("config.yaml has a `hooks:` section that cannot be merged automatically ({what}). {UNMERGEABLE}")
    };
    let mut added: Vec<&'static str> = Vec::new();
    let value =
        inline_value(lines[h].trim_end_matches('\r').trim_start_matches("hooks:")).to_string();
    if is_empty_scalar(&value) && value != "[]" {
        lines[h] = format!("hooks:{cr}");
    } else if !value.is_empty() {
        return Err(unsupported("`hooks:` has an inline value"));
    }

    for event in ["pre_tool_call", "post_tool_call"] {
        let end = block_end(lines, h, 0);
        let child_indent = (h + 1..end)
            .find(|&i| !is_blank_or_comment(&lines[i]))
            .map(|i| indent_of(&lines[i]))
            .unwrap_or(2);
        if child_indent == 0 {
            return Err(unsupported("`hooks:` is a list"));
        }
        let mut new_lines: Vec<String> = Vec::new();
        let insert_at = match find_event(lines, h, end, child_indent, event) {
            Some(k) => {
                let v = inline_value(
                    lines[k]
                        .trim_end_matches('\r')
                        .trim_start()
                        .trim_start_matches(event)
                        .trim_start_matches(':'),
                )
                .to_string();
                if is_empty_scalar(&v) {
                    lines[k] = format!("{}{event}:{cr}", " ".repeat(child_indent));
                } else if !v.is_empty() {
                    return Err(unsupported(&format!("`{event}:` has an inline value")));
                }
                let eb = block_end(lines, k, child_indent);
                let item_indent = (k + 1..eb)
                    .find(|&i| lines[i].trim_start().starts_with("- "))
                    .map(|i| indent_of(&lines[i]))
                    .unwrap_or(child_indent + 2);
                let present = lines[k + 1..eb]
                    .iter()
                    .filter(|l| !is_blank_or_comment(l))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                for hd in HOOKS.iter().filter(|hd| hd.event == event) {
                    if !present.contains(&command_for(bin, hd.sub)) {
                        new_lines.extend(entry_lines(hd, bin, item_indent));
                        added.push(hd.sub);
                    }
                }
                eb
            }
            None => {
                let item_indent = child_indent + 2;
                new_lines.push(format!("{}{event}:", " ".repeat(child_indent)));
                for hd in HOOKS.iter().filter(|hd| hd.event == event) {
                    new_lines.extend(entry_lines(hd, bin, item_indent));
                    added.push(hd.sub);
                }
                end
            }
        };
        if !new_lines.is_empty() {
            let tail = lines.split_off(insert_at);
            lines.extend(new_lines.into_iter().map(|l| format!("{l}{cr}")));
            lines.extend(tail);
        }
    }
    Ok(added)
}

fn plan_config(existing: Option<&str>, bin: &str) -> Result<ConfigPlan, String> {
    let text = existing.unwrap_or("");
    if has_hooks_section(text) {
        let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
        let keys: Vec<usize> = (0..lines.len())
            .filter(|&i| has_hooks_section(&lines[i]))
            .collect();
        if keys.len() != 1 {
            return Err(format!(
                "config.yaml has more than one top-level `hooks:` key. {UNMERGEABLE}"
            ));
        }
        let h = keys[0];
        let cr = if lines[h].ends_with('\r') { "\r" } else { "" };
        let added = merge_into_hooks(&mut lines, h, bin, cr)?;
        let out = lines.join("\n");
        if !HOOKS
            .iter()
            .all(|hd| out.contains(&command_for(bin, hd.sub)))
        {
            return Err(format!(
                "could not merge hooks into `hooks:` safely. {UNMERGEABLE}"
            ));
        }
        if added.is_empty() && out == text {
            return Ok(ConfigPlan::AlreadyEnabled);
        }
        return Ok(ConfigPlan::Merge(out, added));
    }
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str("# Added by foundation enable\n");
    out.push_str(&hooks_block(bin));
    Ok(ConfigPlan::Write(out))
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn iso_utc(t: SystemTime) -> String {
    let d = t.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs() as i64;
    let (y, m, day) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{day:02}T{:02}:{:02}:{:02}.{:06}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        d.subsec_micros()
    )
}

fn merge_allowlist(
    mut data: Value,
    bin: &str,
    now: &str,
    mtime: Option<&str>,
) -> Result<Value, String> {
    let obj = data
        .as_object_mut()
        .ok_or("allowlist is not a JSON object")?;
    let approvals = obj
        .entry("approvals")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or("allowlist `approvals` is not a list")?;
    for h in HOOKS {
        let command = command_for(bin, h.sub);
        approvals.retain(|e| {
            !(e.get("event").and_then(Value::as_str) == Some(h.event)
                && e.get("command").and_then(Value::as_str) == Some(command.as_str()))
        });
        approvals.push(json!({
            "event": h.event,
            "command": command,
            "approved_at": now,
            "script_mtime_at_approval": mtime,
        }));
    }
    Ok(data)
}

fn atomic_write(path: &Path, content: &[u8], mode: Option<u32>) -> Result<(), String> {
    let dir = path.parent().ok_or("path has no parent")?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = dir.join(format!(".{name}.tmp-{}", std::process::id()));
    let result = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let m = match mode {
                Some(m) => m,
                None => fs::metadata(path)
                    .map(|md| md.permissions().mode() & 0o777)
                    .unwrap_or(0o600),
            };
            f.set_permissions(fs::Permissions::from_mode(m))?;
        }
        f.write_all(content)?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(|e| format!("write {}: {e}", path.display()))
}

struct Options {
    home: PathBuf,
    bin: String,
    dry_run: bool,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut home: Option<PathBuf> = None;
    let mut bin: Option<String> = None;
    let mut dry_run = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--hermes-home" => {
                home = Some(PathBuf::from(
                    it.next().ok_or("missing value for --hermes-home")?,
                ))
            }
            "--bin" => bin = Some(it.next().ok_or("missing value for --bin")?.clone()),
            "--dry-run" => dry_run = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let home = match home {
        Some(h) => h,
        None => match std::env::var_os("HERMES_HOME") {
            Some(h) if !h.is_empty() => PathBuf::from(h),
            _ => PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?).join(".hermes"),
        },
    };
    let bin = match bin {
        Some(b) => b,
        None => std::env::current_exe()
            .map_err(|e| format!("cannot resolve own path: {e}"))?
            .to_string_lossy()
            .into_owned(),
    };
    Ok(Options { home, bin, dry_run })
}

fn run_inner(args: &[String]) -> Result<(), String> {
    let opts = parse_args(args)?;
    check_bin_path(&opts.bin)?;
    let bin_path = Path::new(&opts.bin);
    if !bin_path.is_file() {
        return Err(format!("binary not found: {}", opts.bin));
    }
    let config_path = opts.home.join("config.yaml");
    let allow_path = opts.home.join(ALLOWLIST_FILENAME);

    let existing = match fs::read_to_string(&config_path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("read {}: {e}", config_path.display())),
    };
    let plan = plan_config(existing.as_deref(), &opts.bin)?;

    let allow_data = match fs::read_to_string(&allow_path) {
        Ok(t) => serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("{} is malformed: {e}", allow_path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({"approvals": []}),
        Err(e) => return Err(format!("read {}: {e}", allow_path.display())),
    };
    let mtime = fs::metadata(bin_path)
        .and_then(|m| m.modified())
        .ok()
        .map(iso_utc);
    let new_allow = merge_allowlist(
        allow_data,
        &opts.bin,
        &iso_utc(SystemTime::now()),
        mtime.as_deref(),
    )?;

    if opts.dry_run {
        match &plan {
            ConfigPlan::AlreadyEnabled => {
                println!("config.yaml: hooks already present (no change)")
            }
            ConfigPlan::Write(_) => println!(
                "config.yaml: would append hooks block:\n{}",
                hooks_block(&opts.bin)
            ),
            ConfigPlan::Merge(_, added) => println!(
                "config.yaml: would add {} hooks to the existing `hooks:` section: {}",
                added.len(),
                added.join(", ")
            ),
        }
        println!(
            "allowlist: would approve {} hooks in {}",
            HOOKS.len(),
            allow_path.display()
        );
        return Ok(());
    }

    fs::create_dir_all(&opts.home).map_err(|e| format!("create {}: {e}", opts.home.display()))?;
    // Same sibling lock file Hermes uses to serialise allowlist writes.
    let lock = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(opts.home.join(format!("{ALLOWLIST_FILENAME}.lock")))
        .map_err(|e| format!("open lock: {e}"))?;
    lock.lock().map_err(|e| format!("lock allowlist: {e}"))?;

    let merged: Option<Vec<&'static str>> = match &plan {
        ConfigPlan::Merge(_, added) => Some(added.clone()),
        _ => None,
    };
    match plan {
        ConfigPlan::AlreadyEnabled => println!("config.yaml: hooks already present (no change)"),
        ConfigPlan::Write(content) | ConfigPlan::Merge(content, _) => {
            if existing.is_some() {
                let secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let backup = opts.home.join(format!("config.yaml.bak-{secs}"));
                fs::copy(&config_path, &backup)
                    .map_err(|e| format!("backup {}: {e}", backup.display()))?;
                println!("config.yaml: backup -> {}", backup.display());
            }
            atomic_write(&config_path, content.as_bytes(), None)?;
            match merged {
                Some(added) => println!(
                    "config.yaml: added {} hooks to the existing `hooks:` section: {}",
                    added.len(),
                    added.join(", ")
                ),
                None => println!("config.yaml: appended {} hooks", HOOKS.len()),
            }
        }
    }
    let mut text = serde_json::to_string_pretty(&new_allow).map_err(|e| e.to_string())?;
    text.push('\n');
    atomic_write(&allow_path, text.as_bytes(), Some(0o600))?;
    println!(
        "allowlist: approved {} hooks ({})",
        HOOKS.len(),
        allow_path.display()
    );
    println!(
        "A running gateway picks this up within about 30 seconds. Check with: hermes hooks doctor"
    );
    Ok(())
}

pub fn run(args: &[String]) {
    if let Err(e) = run_inner(args) {
        eprintln!("foundation enable: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIN: &str = "/home/u/.hermes/bin/foundation";

    #[test]
    fn appends_to_empty_and_plain_config() {
        let ConfigPlan::Write(a) = plan_config(None, BIN).unwrap() else {
            panic!()
        };
        assert!(a.starts_with("# Added by"));
        let ConfigPlan::Write(b) = plan_config(Some("model: x"), BIN).unwrap() else {
            panic!()
        };
        assert!(b.starts_with("model: x\n\n# Added by"));
        assert!(b.contains("    - command: '\"/home/u/.hermes/bin/foundation\" lifecycle'\n"));
        for h in HOOKS {
            assert!(b.contains(&command_for(BIN, h.sub)));
        }
    }

    #[test]
    fn fail_closed_only_for_config_protection() {
        let ConfigPlan::Write(out) = plan_config(None, BIN).unwrap() else {
            panic!()
        };
        for h in HOOKS {
            let entry = format!(
                "    - command: '{}'\n      matcher: '{}'\n      timeout: {}\n      fail_closed: {}\n",
                command_for(BIN, h.sub),
                h.matcher,
                h.timeout,
                h.sub == "config-protection"
            );
            assert!(out.contains(&entry), "unexpected entry for {}", h.sub);
        }
        assert!(HOOKS
            .iter()
            .all(|h| h.event == "pre_tool_call" || !h.fail_closed));
    }

    fn merged(src: &str) -> (String, Vec<&'static str>) {
        match plan_config(Some(src), BIN).unwrap() {
            ConfigPlan::Merge(out, added) => (out, added),
            other => panic!("expected Merge, got {other:?}"),
        }
    }

    fn assert_all_present(out: &str) {
        for h in HOOKS {
            assert_eq!(
                out.matches(&command_for(BIN, h.sub)).count(),
                1,
                "{} should appear exactly once",
                h.sub
            );
        }
    }

    #[test]
    fn merges_into_foreign_hooks_section() {
        let src = "model: x\nhooks:\n  on_session_start:\n    - command: 'echo hi'\nother: 1\n";
        let (out, added) = merged(src);
        assert_eq!(added.len(), HOOKS.len());
        assert_all_present(&out);
        // existing content is untouched and keeps its place
        assert!(
            out.starts_with("model: x\nhooks:\n  on_session_start:\n    - command: 'echo hi'\n")
        );
        assert!(out.ends_with("other: 1\n"));
        // hooks section is still exactly one top-level key
        assert_eq!(out.lines().filter(|l| l.starts_with("hooks:")).count(), 1);
        // and the result is stable
        assert_eq!(
            plan_config(Some(&out), BIN).unwrap(),
            ConfigPlan::AlreadyEnabled
        );
    }

    #[test]
    fn merges_into_empty_hooks_forms() {
        for src in [
            "hooks:\n",
            "hooks: {}\n",
            "hooks: null # none\n",
            "hooks:\n  pre_tool_call: []\n",
        ] {
            let (out, added) = merged(src);
            assert_eq!(added.len(), HOOKS.len(), "{src:?}");
            assert_all_present(&out);
            assert_eq!(
                plan_config(Some(&out), BIN).unwrap(),
                ConfigPlan::AlreadyEnabled,
                "{src:?}"
            );
        }
    }

    #[test]
    fn appends_to_existing_event_list_with_own_indent() {
        let src = "hooks:\n    pre_tool_call:\n        - command: 'guard'\n          matcher: 'x'\n    post_tool_call:\n        - command: 'log'\n";
        let (out, _) = merged(src);
        assert_all_present(&out);
        assert!(out.contains(
            "        - command: 'guard'\n          matcher: 'x'\n        - command: '\""
        ));
        assert!(
            out.contains("    post_tool_call:\n        - command: 'log'\n        - command: '\"")
        );
    }

    #[test]
    fn adds_only_missing_hooks() {
        let (full, _) = merged("hooks: {}\n");
        let first = HOOKS.iter().find(|h| h.event == "pre_tool_call").unwrap();
        let cmd = command_for(BIN, first.sub);
        // drop one hook's four lines
        let mut lines: Vec<&str> = full.lines().collect();
        let i = lines.iter().position(|l| l.contains(&cmd)).unwrap();
        lines.drain(i..i + 4);
        let partial = lines.join("\n") + "\n";
        let (out, added) = merged(&partial);
        assert_eq!(added, vec![first.sub]);
        assert_all_present(&out);
    }

    #[test]
    fn keeps_comments_and_crlf() {
        let src = "# top\r\nhooks:  # mine\r\n  # note\r\n  on_session_start: []\r\nx: 1\r\n";
        let (out, _) = merged(src);
        assert!(
            out.starts_with("# top\r\nhooks:  # mine\r\n  # note\r\n  on_session_start: []\r\n")
        );
        assert!(out.ends_with("x: 1\r\n"));
        assert!(!out.contains("\n\n") && out.matches('\n').count() == out.matches("\r\n").count());
        assert_all_present(&out);
    }

    #[test]
    fn refuses_unmergeable_hooks_sections() {
        for src in [
            "hooks: *anchor\n",
            "hooks: [a]\n",
            "hooks:\n- a\n",
            "hooks:\n  pre_tool_call: *x\n",
            "hooks:\n  a: 1\nhooks:\n  b: 2\n",
        ] {
            let err = plan_config(Some(src), BIN).unwrap_err();
            assert!(err.contains("by hand"), "{src:?}: {err}");
        }
    }

    #[test]
    fn idempotent_on_own_output() {
        let ConfigPlan::Write(first) = plan_config(Some("a: 1\n"), BIN).unwrap() else {
            panic!()
        };
        assert_eq!(
            plan_config(Some(&first), BIN).unwrap(),
            ConfigPlan::AlreadyEnabled
        );
    }

    #[test]
    fn ignores_commented_or_nested_hooks_key() {
        assert!(!has_hooks_section("# hooks:\nfoo:\n  hooks: 1\n"));
        assert!(has_hooks_section("hooks:\n"));
        assert!(has_hooks_section("hooks: {}\n"));
    }

    #[test]
    fn rejects_unsafe_bin_paths() {
        assert!(check_bin_path("relative/bin").is_err());
        assert!(check_bin_path("/a/b'c").is_err());
        assert!(check_bin_path("/a/b\"c").is_err());
        assert!(check_bin_path("/a/b").is_ok());
    }

    #[test]
    fn allowlist_merge_is_idempotent_and_keeps_others() {
        let other = json!({"event":"on_x","command":"/other","approved_at":"t","script_mtime_at_approval":null});
        let base = json!({"approvals":[other.clone()], "extra": 1});
        let once = merge_allowlist(base, BIN, "2026-01-01T00:00:00.000000Z", Some("m")).unwrap();
        let twice =
            merge_allowlist(once.clone(), BIN, "2026-02-02T00:00:00.000000Z", Some("m")).unwrap();
        let list = twice["approvals"].as_array().unwrap();
        assert_eq!(list.len(), 1 + HOOKS.len());
        assert!(list.contains(&other));
        assert_eq!(twice["extra"], 1);
        assert_eq!(once["approvals"].as_array().unwrap().len(), 1 + HOOKS.len());
    }

    #[test]
    fn allowlist_rejects_bad_shape() {
        assert!(merge_allowlist(json!([]), BIN, "t", None).is_err());
        assert!(merge_allowlist(json!({"approvals": 1}), BIN, "t", None).is_err());
    }

    #[test]
    fn iso_utc_formats_epoch_and_leap_day() {
        assert_eq!(iso_utc(UNIX_EPOCH), "1970-01-01T00:00:00.000000Z");
        let t = UNIX_EPOCH + std::time::Duration::from_secs(1_709_210_096);
        assert_eq!(iso_utc(t), "2024-02-29T12:34:56.000000Z");
    }
}
