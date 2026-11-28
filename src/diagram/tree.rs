use super::home;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const MAX_NODES: usize = 1000;
const MAX_DEPTH: usize = 64;
const MAX_TEXT: usize = 4096;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    r#type: String,
    title: String,
    body: Body,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    nodes: Vec<Node>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Node {
    id: String,
    parent: Option<String>,
    label: String,
}
#[derive(Serialize)]
struct Row {
    id: String,
    parent: Option<String>,
    depth: usize,
    label: String,
    #[serde(skip)]
    branch_prefix: String,
    #[serde(skip)]
    branch: &'static str,
    lines: Vec<String>,
}

pub(super) fn run(args: &[String]) -> Result<String, (i32, String)> {
    if args.len() < 2 || args[0] != "tree" {
        return Err(usage());
    }
    match args[1].as_str() {
        "create" => {
            if args.len() != 9
                || args[3] != "--title"
                || args[5] != "--root-id"
                || args[7] != "--root-label"
            {
                return Err(usage());
            }
            let id = &args[2];
            validate_id(id)?;
            let doc = Document {
                version: 1,
                r#type: "tree".into(),
                title: args[4].clone(),
                body: Body {
                    nodes: vec![Node {
                        id: args[6].clone(),
                        parent: None,
                        label: args[8].clone(),
                    }],
                },
            };
            validate(&doc)?;
            let path = path(id)?;
            if path.exists() {
                return Err((1, "diagram already exists".into()));
            }
            atomic_write(&path, &doc)?;
            Ok(path.display().to_string())
        }
        "add" | "set" => {
            if args.len() != 9 || args[3] != "--id" || args[5] != "--parent" || args[7] != "--label"
            {
                return Err(usage());
            }
            let id = &args[2];
            validate_id(id)?;
            let node = Node {
                id: args[4].clone(),
                parent: if args[6] == "-" {
                    None
                } else {
                    Some(args[6].clone())
                },
                label: args[8].clone(),
            };
            validate_node_identifier("node id", &node.id)?;
            if let Some(parent) = node.parent.as_deref() {
                validate_node_identifier("parent id", parent)?;
            }
            let mut doc = read(id)?;
            if args[1] == "add" {
                if doc.body.nodes.iter().any(|n| n.id == node.id) {
                    return Err((1, format!("duplicate node id: {}", node.id)));
                }
                doc.body.nodes.push(node);
            } else {
                let i = doc
                    .body
                    .nodes
                    .iter()
                    .position(|n| n.id == node.id)
                    .ok_or((1, format!("unknown node id: {}", node.id)))?;
                doc.body.nodes[i] = node;
            }
            validate(&doc)?;
            atomic_write(&path(id)?, &doc)?;
            Ok(String::new())
        }
        "remove" => {
            if args.len() != 4 {
                return Err(usage());
            }
            let id = &args[2];
            validate_id(id)?;
            validate_node_identifier("node id", &args[3])?;
            let mut doc = read(id)?;
            let node_id = &args[3];
            if doc
                .body
                .nodes
                .iter()
                .any(|n| n.parent.as_deref() == Some(node_id.as_str()))
            {
                return Err((1, "cannot remove a node with children".into()));
            }
            let before = doc.body.nodes.len();
            doc.body.nodes.retain(|n| n.id != *node_id);
            if before == doc.body.nodes.len() {
                return Err((1, format!("unknown node id: {node_id}")));
            }
            validate(&doc)?;
            atomic_write(&path(id)?, &doc)?;
            Ok(String::new())
        }
        "render" | "rows" => {
            if args.len() < 3 || args.len() > 5 {
                return Err(usage());
            }
            let id = &args[2];
            validate_id(id)?;
            let width = if args.len() == 5 && args[3] == "--width" {
                args[4]
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n >= 8)
                    .ok_or(usage())?
            } else if args.len() == 3 {
                80
            } else {
                return Err(usage());
            };
            let doc = read(id)?;
            let mut rows = expanded_rows(&doc)?;
            let (title_lines, mut wrapped) = wrap_text(&doc.title, "", width)?;
            for row in &mut rows {
                let prefix = format!("{}{}── [{}] ", row.branch_prefix, row.branch, row.id);
                let (lines, did_wrap) = wrap_text(&row.label, &prefix, width)?;
                row.lines = lines;
                wrapped |= did_wrap;
            }
            if args[1] == "rows" {
                return serde_json::to_string(&rows).map_err(|e| (1, e.to_string()));
            }
            Ok(render(&title_lines, &rows, width, wrapped))
        }
        _ => Err(usage()),
    }
}
fn usage() -> (i32, String) {
    (2, "usage: foundation diagram tree <create ID --title TITLE --root-id ID --root-label LABEL | add ID --id ID --parent ID --label LABEL | set ID --id ID --parent ID --label LABEL | remove ID NODE_ID | render ID [--width N] | rows ID>".into())
}
fn validate_id(id: &str) -> Result<(), (i32, String)> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        Err((
            2,
            "invalid diagram id: use 1-80 ASCII letters, digits, hyphens, or underscores".into(),
        ))
    } else {
        Ok(())
    }
}
fn validate(doc: &Document) -> Result<(), (i32, String)> {
    if doc.version != 1 || doc.r#type != "tree" {
        return Err((1, "unknown diagram type or version".into()));
    }
    if doc.title.trim().is_empty() || doc.title.len() > MAX_TEXT {
        return Err((1, "required title is missing or too long".into()));
    }
    reject_ambiguous_glyphs("title", &doc.title)?;
    if doc.body.nodes.is_empty() || doc.body.nodes.len() > MAX_NODES {
        return Err((
            1,
            format!("node limit exceeded or tree is empty (max {MAX_NODES})"),
        ));
    }
    let mut ids = HashSet::new();
    for n in &doc.body.nodes {
        if n.id.is_empty()
            || n.id.len() > 128
            || n.label.trim().is_empty()
            || n.label.len() > MAX_TEXT
        {
            return Err((1, "required node id/label is missing or too long".into()));
        }
        validate_node_identifier("node id", &n.id)?;
        if let Some(parent) = n.parent.as_deref() {
            validate_node_identifier("parent id", parent)?;
        }
        reject_ambiguous_glyphs("node id", &n.id)?;
        reject_ambiguous_glyphs("label", &n.label)?;
        if !ids.insert(n.id.as_str()) {
            return Err((1, format!("duplicate node id: {}", n.id)));
        }
    }
    let index: HashMap<&str, &Node> = doc.body.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    for n in &doc.body.nodes {
        if let Some(p) = n.parent.as_deref() {
            if !index.contains_key(p) {
                return Err((1, format!("parent does not exist: {} -> {p}", n.id)));
            }
        }
        if n.parent.as_deref() == Some(n.id.as_str()) {
            return Err((1, format!("cycle detected at node {}", n.id)));
        }
    }
    for n in &doc.body.nodes {
        let mut cursor = Some(n.id.as_str());
        let mut seen = HashSet::new();
        let mut depth = 0;
        while let Some(id) = cursor {
            if !seen.insert(id) {
                return Err((1, format!("cycle detected at node {id}")));
            }
            depth += 1;
            if depth > MAX_DEPTH {
                return Err((1, format!("tree depth limit exceeded (max {MAX_DEPTH})")));
            }
            cursor = index[id].parent.as_deref();
        }
    }
    Ok(())
}
fn validate_node_identifier(field: &str, value: &str) -> Result<(), (i32, String)> {
    if value.chars().any(char::is_control) {
        return Err((
            1,
            format!("{field} contains a control character; use printable characters instead"),
        ));
    }
    Ok(())
}
fn reject_ambiguous_glyphs(field: &str, value: &str) -> Result<(), (i32, String)> {
    if value.contains(['✔', '⚠']) {
        return Err((
            1,
            format!(
                "{field} contains width-ambiguous symbol ✔ or ⚠; use ✓, ✗, ?, or ASCII instead"
            ),
        ));
    }
    Ok(())
}
fn terminal_safe(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_control() { '�' } else { ch })
        .collect()
}
fn expanded_rows(doc: &Document) -> Result<Vec<Row>, (i32, String)> {
    validate(doc)?;
    let mut children: HashMap<Option<&str>, Vec<&Node>> = HashMap::new();
    for n in &doc.body.nodes {
        children.entry(n.parent.as_deref()).or_default().push(n);
    }
    let mut rows = Vec::with_capacity(doc.body.nodes.len());
    fn visit<'a>(
        p: Option<&'a str>,
        d: usize,
        ancestor_has_next: &[bool],
        c: &HashMap<Option<&'a str>, Vec<&'a Node>>,
        rows: &mut Vec<Row>,
    ) {
        if let Some(ns) = c.get(&p) {
            for (index, n) in ns.iter().enumerate() {
                let has_next = index + 1 < ns.len();
                let branch_prefix = ancestor_has_next
                    .iter()
                    .map(|has_next| if *has_next { "│   " } else { "    " })
                    .collect::<String>();
                rows.push(Row {
                    id: terminal_safe(&n.id),
                    parent: n.parent.as_deref().map(terminal_safe),
                    depth: d,
                    label: terminal_safe(&n.label),
                    branch_prefix,
                    branch: if has_next { "├" } else { "└" },
                    lines: Vec::new(),
                });
                let mut child_ancestors = ancestor_has_next.to_vec();
                child_ancestors.push(has_next);
                visit(Some(&n.id), d + 1, &child_ancestors, c, rows);
            }
        }
    }
    visit(None, 0, &[], &children, &mut rows);
    if rows.len() != doc.body.nodes.len() {
        return Err((1, "parent consistency check failed".into()));
    }
    Ok(rows)
}

fn wrap_text(text: &str, prefix: &str, width: usize) -> Result<(Vec<String>, bool), (i32, String)> {
    let prefix_width = UnicodeWidthStr::width(prefix);
    if prefix_width >= width {
        return Err((
            1,
            format!(
                "tree prefix needs {prefix_width} columns; increase --width to at least {}",
                prefix_width + 1
            ),
        ));
    }
    let continuation = " ".repeat(prefix_width);
    let available = width - prefix_width;
    let normalized: String = text
        .chars()
        .map(|ch| match ch {
            '\n' | '\r' => '↵',
            '\t' => '⇥',
            ch if ch.is_control() => '�',
            ch => ch,
        })
        .collect();
    let mut lines = Vec::new();
    let mut line = prefix.to_owned();
    let mut used = 0;
    let mut wrapped = false;
    for grapheme in UnicodeSegmentation::graphemes(normalized.as_str(), true) {
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if used + grapheme_width > available && used > 0 {
            lines.push(line);
            line = continuation.clone();
            used = 0;
            wrapped = true;
        }
        if grapheme_width > available {
            return Err((
                1,
                format!(
                    "text grapheme exceeds available width; increase --width to at least {}",
                    prefix_width + grapheme_width
                ),
            ));
        }
        line.push_str(grapheme);
        used += grapheme_width;
    }
    lines.push(line);
    Ok((lines, wrapped))
}

fn render(title_lines: &[String], rows: &[Row], width: usize, wrapped: bool) -> String {
    let mut out = title_lines.join("\n");
    out.push('\n');
    for row in rows {
        out.push_str(&row.lines.join("\n"));
        out.push('\n');
    }
    if wrapped {
        let marker = "[折り返しあり]";
        if UnicodeWidthStr::width(marker) <= width {
            out.push_str(marker);
            out.push('\n');
        }
    }
    out
}
fn path(id: &str) -> Result<PathBuf, (i32, String)> {
    let mut dir = home()?;
    for component in ["plugin-data", "foundation", "diagrams"] {
        dir.push(component);
        match fs::symlink_metadata(&dir) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err((
                    1,
                    format!(
                        "diagram storage path component must not be a symlink: {}",
                        dir.display()
                    ),
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err((
                    1,
                    format!(
                        "diagram storage path component is not a directory: {}",
                        dir.display()
                    ),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    let mut builder = fs::DirBuilder::new();
                    builder.mode(0o700);
                    builder
                        .create(&dir)
                        .map_err(|e| (1, format!("create diagram directory: {e}")))?;
                }
                #[cfg(not(unix))]
                fs::create_dir(&dir).map_err(|e| (1, format!("create diagram directory: {e}")))?;
            }
            Err(error) => return Err((1, format!("inspect diagram storage path: {error}"))),
        }
    }
    Ok(dir.join(format!("{id}.json")))
}
fn read(id: &str) -> Result<Document, (i32, String)> {
    let p = path(id)?;
    let m = fs::symlink_metadata(&p).map_err(|e| (1, format!("read diagram: {e}")))?;
    if m.file_type().is_symlink() || !m.is_file() {
        return Err((1, "diagram file must be a regular file".into()));
    }
    let data = fs::read(p).map_err(|e| (1, format!("read diagram: {e}")))?;
    let doc: Document =
        serde_json::from_slice(&data).map_err(|e| (1, format!("invalid diagram JSON: {e}")))?;
    validate(&doc)?;
    Ok(doc)
}
fn atomic_write(p: &Path, doc: &Document) -> Result<(), (i32, String)> {
    let parent = p.parent().ok_or((1, "invalid diagram path".into()))?;
    let name = p.file_name().unwrap_or_default().to_string_lossy();
    let temp = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(doc).map_err(|e| (1, e.to_string()))?;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| (1, format!("create temporary diagram: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| (1, e.to_string()))?;
    }
    if let Err(e) = f.write_all(&bytes).and_then(|_| f.sync_all()) {
        let _ = fs::remove_file(&temp);
        return Err((1, format!("write diagram: {e}")));
    }
    drop(f);
    if let Err(e) = fs::rename(&temp, p) {
        let _ = fs::remove_file(&temp);
        return Err((1, format!("replace diagram: {e}")));
    }
    Ok(())
}
