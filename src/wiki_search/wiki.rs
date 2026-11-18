use regex::Regex;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug)]
pub struct Page {
    pub path: String,
    pub title: String,
    pub summary: String,
    pub domains: Vec<String>,
}
fn files(dir: &Path, root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for item in fs::read_dir(dir)
        .map_err(|e| format!("cannot read wiki directory {}: {e}", dir.display()))?
    {
        let p = item.map_err(|e| e.to_string())?.path();
        let rel = p.strip_prefix(root).unwrap();
        if rel.components().any(|c| {
            let s = c.as_os_str().to_string_lossy();
            s.starts_with('.') || s == "raw"
        }) {
            continue;
        }
        if p.is_dir() {
            files(&p, root, out)?
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p)
        }
    }
    Ok(())
}
fn title(text: &str) -> Option<String> {
    let fm = Regex::new(r"(?s)^---\s*\n(.*?)\n---(?:\s*\n|$)").unwrap();
    let title = Regex::new(r"(?m)^title:\s*(.*?)\s*$").unwrap();
    if let Some(c) = fm.captures(text) {
        if let Some(v) = title.captures(&c[1]) {
            let s = unquote(&v[1]);
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    Regex::new(r"(?m)^#\s+(.+?)\s*$")
        .unwrap()
        .captures(text)
        .map(|c| c[1].to_string())
}
fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2
        && ((s.starts_with('\'') && s.ends_with('\'')) || (s.starts_with('"') && s.ends_with('"')))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}
fn frontmatter_domains(text: &str) -> Vec<String> {
    let Some(header) = text
        .strip_prefix("---")
        .and_then(|s| s.split_once("\n---").map(|p| p.0))
    else {
        return Vec::new();
    };
    let Some(value) = header
        .lines()
        .find_map(|line| line.strip_prefix("domains:").map(str::trim))
    else {
        return Vec::new();
    };
    let value = value.trim_start_matches('[').trim_end_matches(']');
    value
        .split(',')
        .map(|s| unquote(s.trim()))
        .filter(|s| !s.is_empty())
        .collect()
}
pub fn schema_domains(root: &Path) -> Result<Vec<(String, String)>, String> {
    let schema = fs::read_to_string(root.join("SCHEMA.md"))
        .map_err(|e| format!("cannot read SCHEMA.md: {e}"))?;
    let mut domains = Vec::new();
    for line in schema.lines() {
        let cells: Vec<_> = line.split('|').map(str::trim).collect();
        if cells.len() >= 4 {
            let name = cells[1].trim_matches('*').trim();
            let desc = cells[2];
            if !name.is_empty() && name != "Domain" && !name.starts_with('-') && !desc.is_empty() {
                domains.push((name.to_string(), desc.to_string()));
            }
        }
    }
    if domains.is_empty() {
        return Err("no domains found in SCHEMA.md".into());
    }
    Ok(domains)
}
pub fn discover(root: &Path) -> Result<Vec<Page>, String> {
    if !root.is_dir() {
        return Err(format!("wiki directory does not exist: {}", root.display()));
    }
    let mut summaries = HashMap::new();
    let index = root.join("index.md");
    if let Ok(t) = fs::read_to_string(index) {
        let re = Regex::new(r"^\s*-\s+\[\[([^]|#]+)(?:\|[^]]*)?\]\]\s*(?:[—–-]\s*(.*))?$").unwrap();
        for l in t.lines() {
            if let Some(c) = re.captures(l) {
                summaries.insert(
                    c[1].trim().to_string(),
                    c.get(2).map_or("", |m| m.as_str()).trim().to_string(),
                );
            }
        }
    }
    let mut paths = Vec::new();
    files(root, root, &mut paths)?;
    paths.sort();
    let pages: Vec<_> = paths
        .into_iter()
        .filter_map(|p| {
            let rel = p
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            if matches!(rel.as_str(), "index.md" | "SCHEMA.md" | "log.md") {
                return None;
            }
            let text = fs::read_to_string(&p).ok()?;
            let title = title(&text)?;
            let stem = p.file_stem()?.to_string_lossy().to_string();
            let domains = frontmatter_domains(&text);
            Some(Page {
                path: rel,
                title,
                summary: summaries.get(&stem).cloned().unwrap_or_default(),
                domains,
            })
        })
        .collect();
    if pages.is_empty() {
        return Err(format!("no wiki pages found in {}", root.display()));
    }
    Ok(pages)
}
