//! Opt-in desktop notifications on macOS and WSL; Linux is intentionally a no-op.
use serde_json::Value;
use std::process::Command;

fn enabled() -> bool {
    std::env::var("FOUNDATION_DESKTOP_NOTIFY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn apple_quote(value: &str) -> String {
    value
        .replace('\\', "")
        .replace('"', "“")
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

fn powershell_quote(value: &str) -> String {
    value.replace('\'', "''")
}

fn wsl() -> bool {
    std::fs::read_to_string("/proc/version")
        .map(|s| s.to_ascii_lowercase().contains("microsoft"))
        .unwrap_or(false)
}

/// Return executable and argv for tests and callers; never invokes the command.
pub fn command_for(os: &str, is_wsl: bool) -> Option<(String, Vec<String>)> {
    const TITLE: &str = "Claude Code";
    const BODY: &str = "Task completed"; // Never copy conversation text into notifications.
    match os {
        "macos" => Some((
            "osascript".into(),
            vec![
                "-e".into(),
                format!(
                    "display notification \"{}\" with title \"{}\"",
                    apple_quote(BODY),
                    apple_quote(TITLE)
                ),
            ],
        )),
        "linux" if is_wsl => Some((
            "powershell.exe".into(),
            vec![
                "-Command".into(),
                format!(
                    "Import-Module BurntToast; New-BurntToastNotification -Text '{}', '{}'",
                    powershell_quote(TITLE),
                    powershell_quote(BODY)
                ),
            ],
        )),
        _ => None,
    }
}

pub fn run(_payload: &Value) {
    if !enabled() {
        return;
    }
    let os = match std::env::consts::OS {
        "macos" => "macos",
        "linux" => "linux",
        _ => "other",
    };
    let Some((program, args)) = command_for(os, wsl()) else {
        return;
    };
    let _ = Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::command_for;

    #[test]
    fn assembles_platform_commands_without_executing_them() {
        let (program, args) = command_for("macos", false).unwrap();
        assert_eq!(program, "osascript");
        assert!(args.join(" ").contains("Task completed"));
        let (program, args) = command_for("linux", true).unwrap();
        assert_eq!(program, "powershell.exe");
        assert!(args.join(" ").contains("BurntToast"));
        assert!(command_for("linux", false).is_none());
    }

    #[test]
    fn command_text_is_constant_and_does_not_include_user_content() {
        let (_, args) = command_for("macos", false).unwrap();
        assert!(!args.join(" ").contains("assistant_response"));
    }
}
