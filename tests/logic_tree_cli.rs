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
