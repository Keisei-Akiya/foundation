//! tests/common の共有ヘルパーの振る舞い。
//! 同名の `home` を並列に呼んでも、別のディレクトリが返ることを確かめる。
mod common;

use std::{collections::HashSet, thread};

#[test]
fn home_returns_distinct_dirs_for_the_same_name_across_threads() {
    let paths: Vec<_> = (0..16)
        .map(|_| thread::spawn(|| common::home("same", "name")))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect();
    let unique: HashSet<_> = paths.iter().collect();
    assert_eq!(unique.len(), paths.len(), "paths collided: {paths:?}");
    for p in &paths {
        assert!(p.join(".hermes").is_dir(), "{p:?} has no .hermes");
    }
}
