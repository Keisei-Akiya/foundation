use regex::Regex;
use serde_json::{json, Value};

pub fn preflight(payload: &Value) {
    let Some(p) = payload.as_object() else {
        return;
    };
    let Some(name) = p.get("tool_name").and_then(Value::as_str) else {
        return;
    };
    let args = p
        .get("tool_input")
        .or_else(|| p.get("args"))
        .and_then(Value::as_object);
    let Some(args) = args else {
        return;
    };
    let command = if [
        "terminal",
        "bash",
        "Bash",
        "powershell",
        "PowerShell",
        "powershell_tool",
        "pwsh",
    ]
    .contains(&name)
    {
        args.get("command").and_then(Value::as_str).unwrap_or("")
    } else {
        ""
    };
    if Regex::new(r"(?i)\bgit\s+commit\b[^\n]*--no-verify\b")
        .unwrap()
        .is_match(command)
    {
        println!(
            "{}",
            json!({"action":"block","message":"Preflight block: detected literal git commit --no-verify; shell syntax and git hook-redirect variants are not covered."})
        );
        return;
    }
    if ["powershell", "powershell_tool", "pwsh"].contains(&name.to_ascii_lowercase().as_str()) {
        let destructive = Regex::new(r"(?i)\b(Remove-Item|Clear-Content|Stop-Process|Format-Volume|Remove-Partition)\b|\b(?:Remove-Item|del|erase)\b[^\n]*-Recurse[^\n]*-Force").unwrap();
        if destructive.is_match(command) {
            println!(
                "{}",
                json!({"action":"block","message":"Potentially destructive PowerShell operation; review target and rollback before retrying."})
            );
        }
    }
}
