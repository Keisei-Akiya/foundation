mod common;

use serde_json::Value;
use std::fs;

#[test]
fn creates_tree_adds_node_and_renders_japanese_with_wrapping_notice() {
    let home = common::home("logic-tree", "render");
    let h = home.to_str().unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree",
            "create",
            "demo",
            "--title",
            "検討",
            "--root-id",
            "root",
            "--root-label",
            "結論",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree",
            "add",
            "demo",
            "--id",
            "reason",
            "--parent",
            "root",
            "--label",
            "これは幅を超えて折り返す日本語の理由",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    let (code, out, err) = common::run(
        "diagram",
        &["tree", "render", "demo", "--width", "20"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(out.contains("折り返し"));
    assert!(out.contains("reason"));
    let file = home.join("plugin-data/foundation/diagrams/demo.json");
    let diagram: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
    assert_eq!(diagram["version"], 1);
    assert_eq!(diagram["type"], "tree");
    assert_eq!(diagram["body"]["nodes"].as_array().unwrap().len(), 2);
}

#[test]
fn rejects_bad_tree_and_unsafe_identifier_without_writing() {
    let home = common::home("logic-tree", "reject");
    let h = home.to_str().unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree",
            "create",
            "../escape",
            "--title",
            "x",
            "--root-id",
            "root",
            "--root-label",
            "x",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!(code, 2);
    assert!(err.contains("invalid diagram id"));
    assert!(!home.join("plugin-data/foundation/diagrams").exists());
}

#[test]
fn validation_reports_duplicate_and_missing_parent_ids() {
    let home = common::home("logic-tree", "validate");
    let h = home.to_str().unwrap();
    let (code, _, _) = common::run(
        "diagram",
        &[
            "tree",
            "create",
            "bad",
            "--title",
            "x",
            "--root-id",
            "root",
            "--root-label",
            "x",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!(code, 0);
    let path = home.join("plugin-data/foundation/diagrams/bad.json");
    let mut doc: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    doc["body"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"child","parent":"missing","label":"bad"}));
    fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &["tree", "render", "bad"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!(code, 1);
    assert!(err.contains("parent does not exist"));
    doc["body"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"root","parent":null,"label":"duplicate"}));
    fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &["tree", "render", "bad"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!(code, 1);
    assert!(err.contains("duplicate node id"));
    doc["body"]["nodes"] = serde_json::json!([{"id":"root","parent":null,"label":"x"}]);
    doc["type"] = Value::String("future".into());
    fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &["tree", "render", "bad"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!(code, 1);
    assert!(err.contains("unknown diagram type"));
    doc["type"] = Value::String("tree".into());
    doc.as_object_mut().unwrap().remove("title");
    fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &["tree", "render", "bad"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!(code, 1);
    assert!(err.contains("invalid diagram JSON"));
}

#[test]
fn rows_and_render_share_stable_branch_lines_and_fit_width() {
    let home = common::home("logic-tree", "formatted-rows");
    let h = home.to_str().unwrap();
    for args in [
        vec![
            "tree",
            "create",
            "demo",
            "--title",
            "非常に長いタイトルです。これはタイトルの折り返し確認です",
            "--root-id",
            "root",
            "--root-label",
            "Root",
        ],
        vec![
            "tree",
            "add",
            "demo",
            "--id",
            "first",
            "--parent",
            "root",
            "--label",
            "First child",
        ],
        vec![
            "tree",
            "add",
            "demo",
            "--id",
            "leaf",
            "--parent",
            "first",
            "--label",
            "A long Japanese child label 日本語",
        ],
        vec![
            "tree", "add", "demo", "--id", "last", "--parent", "root", "--label", "Last",
        ],
    ] {
        let (code, _, err) = common::run("diagram", &args, "", &[("HERMES_HOME", h)], &home);
        assert_eq!((code, err.as_str()), (0, ""));
    }
    let (code, rows_out, err) = common::run(
        "diagram",
        &["tree", "rows", "demo", "--width", "24"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    let rows: Value = serde_json::from_str(&rows_out).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows[1]["depth"], 1);
    assert_eq!(rows[1]["parent"], "root");
    assert!(
        rows[1]["lines"].as_array().is_some(),
        "rows must expose formatted physical lines"
    );
    let (code, rendered, err) = common::run(
        "diagram",
        &["tree", "render", "demo", "--width", "24"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    for row in rows {
        for line in row["lines"].as_array().unwrap() {
            assert!(
                rendered.contains(line.as_str().unwrap()),
                "render omitted row line: {line}"
            );
        }
    }
    assert!(rows[1]["lines"][0].as_str().unwrap().contains("├──"));
    assert!(rows[3]["lines"][0].as_str().unwrap().contains("└──"));
    assert!(rendered.contains("折り返し"));
    assert!(rendered
        .lines()
        .all(|line| unicode_width::UnicodeWidthStr::width(line) <= 24));
    let (code, _, err) = common::run(
        "diagram",
        &["tree", "render", "demo", "--width", "8"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!(code, 1);
    assert!(err.contains("width") && err.contains("increase"));
}

#[test]
fn render_sanitizes_controls_and_keeps_wide_graphemes_within_width() {
    let home = common::home("logic-tree", "unicode-layout");
    let h = home.to_str().unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree",
            "create",
            "unicode",
            "--title",
            "Title\nnext",
            "--root-id",
            "root",
            "--root-label",
            "日本語 👨‍👩‍👧‍👦\u{1b}[31mred\u{1b}[0m",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    let (code, rendered, err) = common::run(
        "diagram",
        &["tree", "render", "unicode", "--width", "28"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(!rendered.contains('\u{1b}'));
    assert!(rendered.contains("↵"));
    assert!(rendered.contains("👨‍👩‍👧‍👦"));
    assert!(rendered
        .lines()
        .all(|line| unicode_width::UnicodeWidthStr::width(line) <= 28));
}

#[test]
fn rows_are_fully_expanded_with_depth_and_parent_metadata() {
    let home = common::home("logic-tree", "rows");
    let h = home.to_str().unwrap();
    for args in [
        vec![
            "tree",
            "create",
            "demo",
            "--title",
            "T",
            "--root-id",
            "root",
            "--root-label",
            "Root",
        ],
        vec![
            "tree", "add", "demo", "--id", "child", "--parent", "root", "--label", "Child",
        ],
        vec![
            "tree", "add", "demo", "--id", "leaf", "--parent", "child", "--label", "Leaf",
        ],
    ] {
        let (code, _, err) = common::run("diagram", &args, "", &[("HERMES_HOME", h)], &home);
        assert_eq!((code, err.as_str()), (0, ""));
    }
    let (code, out, err) = common::run(
        "diagram",
        &["tree", "rows", "demo"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    let rows: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 3);
    assert_eq!(rows[0]["depth"], 0);
    assert!(rows[0]["parent"].is_null());
    assert_eq!(rows[1]["depth"], 1);
    assert_eq!(rows[1]["parent"], "root");
    assert_eq!(rows[2]["depth"], 2);
    assert_eq!(rows[2]["parent"], "child");
}

#[test]
fn rejects_node_and_depth_limits() {
    let home = common::home("logic-tree", "limits");
    let h = home.to_str().unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree",
            "create",
            "demo",
            "--title",
            "T",
            "--root-id",
            "root",
            "--root-label",
            "Root",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    let path = home.join("plugin-data/foundation/diagrams/demo.json");
    let mut doc: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    doc["body"]["nodes"] = Value::Array(
        (0..1001)
            .map(|i| serde_json::json!({"id":format!("n{i}"),"parent":null,"label":"n"}))
            .collect(),
    );
    fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &["tree", "render", "demo"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!(code, 1);
    assert!(err.contains("node limit exceeded"));
    doc["body"]["nodes"] = Value::Array(
        (0..66)
            .map(|i| serde_json::json!({
                "id":format!("n{i}"),
                "parent":if i == 0 { Value::Null } else { Value::String(format!("n{}", i - 1)) },
                "label":"n"
            }))
            .collect(),
    );
    fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &["tree", "render", "demo"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!(code, 1);
    assert!(err.contains("depth limit exceeded"));
}

#[test]
fn rejects_cycles_without_corrupting_saved_tree() {
    let home = common::home("logic-tree", "cycle");
    let h = home.to_str().unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree",
            "create",
            "demo",
            "--title",
            "T",
            "--root-id",
            "root",
            "--root-label",
            "Root",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree", "add", "demo", "--id", "child", "--parent", "root", "--label", "Child",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree", "set", "demo", "--id", "root", "--parent", "child", "--label", "Root",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!(code, 1);
    assert!(err.contains("cycle detected"));
    let (code, out, err) = common::run(
        "diagram",
        &["tree", "rows", "demo"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(out.contains("\"depth\":0"));
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_storage_ancestors_without_touching_outside_files() {
    use std::os::unix::fs::symlink;

    for component in ["plugin-data", "foundation"] {
        let home = common::home("logic-tree", component);
        let outside = common::home("logic-tree-outside", component);
        let external_home = outside;
        let outside_file = external_home.join("plugin-data/foundation/diagrams/demo.json");
        fs::create_dir_all(outside_file.parent().unwrap()).unwrap();
        let (code, _, err) = common::run(
            "diagram",
            &[
                "tree",
                "create",
                "demo",
                "--title",
                "T",
                "--root-id",
                "root",
                "--root-label",
                "Root",
            ],
            "",
            &[("HERMES_HOME", external_home.to_str().unwrap())],
            &external_home,
        );
        assert_eq!((code, err.as_str()), (0, ""));
        let original = fs::read(&outside_file).unwrap();
        if component == "plugin-data" {
            symlink(external_home.join("plugin-data"), home.join("plugin-data")).unwrap();
        } else {
            fs::create_dir_all(home.join("plugin-data")).unwrap();
            symlink(
                external_home.join("plugin-data/foundation"),
                home.join("plugin-data/foundation"),
            )
            .unwrap();
        }

        let (code, _, err) = common::run(
            "diagram",
            &[
                "tree", "add", "demo", "--id", "child", "--parent", "root", "--label", "Changed",
            ],
            "",
            &[("HERMES_HOME", home.to_str().unwrap())],
            &home,
        );
        assert_eq!(code, 1, "component {component}: {err}");
        assert!(err.contains("symlink"), "component {component}: {err}");
        assert_eq!(fs::read(&outside_file).unwrap(), original);
    }
}

#[cfg(unix)]
#[test]
fn creates_private_diagrams_directory_without_changing_shared_parent_modes() {
    use std::os::unix::fs::PermissionsExt;

    let home = common::home("logic-tree", "permissions");
    let shared = home.join("plugin-data/foundation");
    fs::create_dir_all(&shared).unwrap();
    fs::set_permissions(home.join("plugin-data"), fs::Permissions::from_mode(0o751)).unwrap();
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o755)).unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree",
            "create",
            "demo",
            "--title",
            "T",
            "--root-id",
            "root",
            "--root-label",
            "Root",
        ],
        "",
        &[("HERMES_HOME", home.to_str().unwrap())],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        fs::metadata(shared.join("diagrams"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(home.join("plugin-data"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o751
    );
    assert_eq!(
        fs::metadata(&shared).unwrap().permissions().mode() & 0o777,
        0o755
    );
}

#[test]
fn rendered_rows_never_emit_terminal_controls_from_title_or_labels() {
    let home = common::home("logic-tree", "terminal-controls");
    let h = home.to_str().unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree",
            "create",
            "demo",
            "--title",
            "Title\u{85}\u{1b}[31m",
            "--root-id",
            "root",
            "--root-label",
            "Label",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree",
            "add",
            "demo",
            "--id",
            "child",
            "--parent",
            "root",
            "--label",
            "Label\u{1b}[2J",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    for args in [vec!["tree", "render", "demo"], vec!["tree", "rows", "demo"]] {
        let (code, out, err) = common::run("diagram", &args, "", &[("HERMES_HOME", h)], &home);
        assert_eq!((code, err.as_str()), (0, ""));
        assert!(
            !out.chars().any(|ch| ch.is_control() && ch != '\n'),
            "{out:?}"
        );
        assert!(
            !out.contains('\u{85}') && !out.contains('\u{9b}'),
            "{out:?}"
        );
    }
}

#[test]
fn rejects_width_ambiguous_symbols_in_title_id_label_and_rows() {
    let home = common::home("logic-tree", "ambiguous-glyphs");
    let h = home.to_str().unwrap();
    for (field, args) in [
        (
            "title",
            vec![
                "tree",
                "create",
                "demo",
                "--title",
                "bad ✔",
                "--root-id",
                "root",
                "--root-label",
                "Root",
            ],
        ),
        (
            "root id",
            vec![
                "tree",
                "create",
                "demo",
                "--title",
                "Title",
                "--root-id",
                "root⚠",
                "--root-label",
                "Root",
            ],
        ),
        (
            "label",
            vec![
                "tree",
                "create",
                "demo",
                "--title",
                "Title",
                "--root-id",
                "root",
                "--root-label",
                "bad ⚠",
            ],
        ),
    ] {
        let (code, _, err) = common::run("diagram", &args, "", &[("HERMES_HOME", h)], &home);
        assert_eq!(code, 1, "{field}: {err}");
        assert!(err.contains("✔") || err.contains("⚠"), "{field}: {err}");
        assert!(
            err.contains("ASCII") || err.contains("幅"),
            "{field}: {err}"
        );
    }
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree",
            "create",
            "demo",
            "--title",
            "Title",
            "--root-id",
            "root",
            "--root-label",
            "Root",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    for (node_id, label) in [("bad✔", "Label"), ("child", "bad ⚠")] {
        let (code, _, err) = common::run(
            "diagram",
            &[
                "tree", "add", "demo", "--id", node_id, "--parent", "root", "--label", label,
            ],
            "",
            &[("HERMES_HOME", h)],
            &home,
        );
        assert_eq!(code, 1, "{node_id}/{label}: {err}");
        assert!(err.contains("✔") || err.contains("⚠"), "{err}");
    }
    fs::write(
        home.join("plugin-data/foundation/diagrams/demo.json"),
        r#"{"version":1,"type":"tree","title":"Title","body":{"nodes":[{"id":"root","parent":null,"label":"Bad ⚠"}]}}"#,
    )
    .unwrap();
    let (code, out, err) = common::run(
        "diagram",
        &["tree", "rows", "demo"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("width-ambiguous symbol"));
    assert!(err.contains("✔") && err.contains("⚠"));
}

#[test]
fn rejects_control_characters_in_node_ids_and_parents_without_mutating_files() {
    let home = common::home("logic-tree", "unsafe-node-ids");
    let h = home.to_str().unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &[
            "tree",
            "create",
            "demo",
            "--title",
            "Title",
            "--root-id",
            "root",
            "--root-label",
            "Root",
        ],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    let path = home.join("plugin-data/foundation/diagrams/demo.json");
    let baseline = fs::read(&path).unwrap();
    let cases = [
        vec![
            "tree", "add", "demo", "--id", "x\u{85}", "--parent", "root", "--label", "Child",
        ],
        vec![
            "tree", "add", "demo", "--id", "child", "--parent", "x\u{9b}", "--label", "Child",
        ],
        vec![
            "tree", "set", "demo", "--id", "root", "--parent", "x\u{85}", "--label", "Root",
        ],
        vec!["tree", "remove", "demo", "x\u{9b}"],
    ];
    for args in cases {
        let (code, _, err) = common::run("diagram", &args, "", &[("HERMES_HOME", h)], &home);
        assert_ne!(code, 0, "{args:?}");
        assert!(err.contains("control character"), "{err:?}");
        assert!(!err.chars().any(char::is_control), "{err:?}");
        assert_eq!(fs::read(&path).unwrap(), baseline, "{args:?}");
    }

    let (code, out, err) = common::run(
        "diagram",
        &["tree", "rows", "demo"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_eq!((code, err.as_str()), (0, ""));
    let rows: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(rows[0]["id"], "root");
    assert_eq!(rows[0]["parent"], Value::Null);

    let mut doc: Value = serde_json::from_slice(&baseline).unwrap();
    doc["body"]["nodes"] = serde_json::json!([
        {"id":"x\u{85}","parent":null,"label":"Root"},
        {"id":"x\u{9b}","parent":"x\u{85}","label":"Child"}
    ]);
    let invalid_id_bytes = serde_json::to_vec(&doc).unwrap();
    fs::write(&path, &invalid_id_bytes).unwrap();
    for args in [
        vec!["tree", "rows", "demo"],
        vec![
            "tree", "add", "demo", "--id", "next", "--parent", "x\u{85}", "--label", "Next",
        ],
    ] {
        let (code, _, err) = common::run("diagram", &args, "", &[("HERMES_HOME", h)], &home);
        assert_ne!(code, 0);
        assert!(err.contains("control character"), "{err:?}");
        assert!(!err.chars().any(char::is_control), "{err:?}");
        assert_eq!(fs::read(&path).unwrap(), invalid_id_bytes);
    }

    doc["body"]["nodes"] = serde_json::json!([
        {"id":"root","parent":null,"label":"Root"},
        {"id":"child","parent":"x\u{85}","label":"Child"}
    ]);
    let invalid_parent_bytes = serde_json::to_vec(&doc).unwrap();
    fs::write(&path, &invalid_parent_bytes).unwrap();
    let (code, _, err) = common::run(
        "diagram",
        &["tree", "render", "demo"],
        "",
        &[("HERMES_HOME", h)],
        &home,
    );
    assert_ne!(code, 0);
    assert!(
        err.contains("parent id") && err.contains("control character"),
        "{err:?}"
    );
    assert!(!err.chars().any(char::is_control), "{err:?}");
    assert_eq!(fs::read(&path).unwrap(), invalid_parent_bytes);
}
