#!/usr/bin/env python3
"""Check or safely auto-fix files using an isolated Git-index snapshot."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path


def run(*args: str, cwd: Path, env: dict[str, str] | None = None) -> None:
    result = subprocess.run(args, cwd=cwd, env=env)
    if result.returncode:
        raise SystemExit(result.returncode)


def git(root: Path, *args: str, input: bytes | None = None) -> bytes:
    return subprocess.run(
        ["git", "-C", str(root), *args], check=True, input=input, stdout=subprocess.PIPE
    ).stdout


def staged_paths(root: Path) -> list[str]:
    names = git(root, "diff", "--cached", "--name-only", "-z", "--diff-filter=ACMR")
    return [os.fsdecode(name) for name in names.split(b"\0") if name]


def staged_python(root: Path) -> list[str]:
    return [path for path in staged_paths(root) if path.endswith(".py")]


def staged_rust(root: Path) -> list[str]:
    return [path for path in staged_paths(root) if path.endswith(".rs")]


def rustfmt(snapshot: Path, rust_files: list[str], *options: str) -> None:
    if rust_files:
        run(
            "rustfmt",
            "--edition",
            "2021",
            "--config",
            "skip_children=true",
            *options,
            *rust_files,
            cwd=snapshot,
        )


def apply_snapshot_to_index(
    root: Path, snapshot: Path, allowed_paths: set[str]
) -> None:
    """Replace only changed index blobs; never git-add working-tree files."""
    entries = git(root, "ls-files", "--stage", "-z").split(b"\0")
    updates = bytearray()
    for entry in entries:
        if not entry:
            continue
        metadata, raw_path = entry.split(b"\t", 1)
        mode, old_blob, stage = metadata.split(b" ")
        if stage != b"0":
            raise SystemExit("cannot auto-fix an index with unresolved conflicts")
        path = os.fsdecode(raw_path)
        if path not in allowed_paths:
            continue
        file_path = snapshot / path
        if not file_path.is_file():
            continue
        new_blob = git(root, "hash-object", "-w", "--", str(file_path)).strip()
        if new_blob != old_blob:
            updates.extend(mode + b" " + new_blob + b"\t" + raw_path + b"\0")
    if updates:
        git(root, "update-index", "-z", "--index-info", input=bytes(updates))


def verify(
    snapshot: Path, python_files: list[str], rust_files: list[str], mode: str = "all"
) -> None:
    cargo_env = os.environ.copy()
    cargo_env["CARGO_TARGET_DIR"] = str(Path(__file__).resolve().parents[1] / "target")
    if mode in {"lint", "all"}:
        run(
            "cargo",
            "clippy",
            "--all-targets",
            "--all-features",
            "--locked",
            "--",
            "-D",
            "warnings",
            cwd=snapshot,
            env=cargo_env,
        )
    if mode in {"lint", "all"} and python_files:
        run(
            "uv",
            "run",
            "--locked",
            "ruff",
            "check",
            *python_files,
            cwd=snapshot,
        )
    if mode in {"format", "all"}:
        run("cargo", "fmt", "--all", "--", "--check", cwd=snapshot, env=cargo_env)
        if python_files:
            run(
                "uv",
                "run",
                "--locked",
                "ruff",
                "format",
                "--check",
                *python_files,
                cwd=snapshot,
            )


def clean_worktree_paths(root: Path, paths: set[str]) -> set[str]:
    """Capture staged paths whose worktree content matches the original index."""
    clean = set()
    for path in paths:
        result = subprocess.run(
            ["git", "-C", str(root), "diff", "--quiet", "--", path], check=False
        )
        if result.returncode == 0:
            clean.add(path)
        elif result.returncode != 1:
            raise SystemExit(f"could not inspect worktree state for {path!r}")
    return clean


def apply_clean_worktree_fixes(
    root: Path, snapshot: Path, clean_paths: set[str]
) -> None:
    """Mirror fixes only to files that matched the index before fixing."""
    for path in clean_paths:
        fixed = snapshot / path
        if fixed.is_file():
            destination = root / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(fixed.read_bytes())


def main() -> int:
    if len(sys.argv) != 2 or sys.argv[1] not in {"lint", "format", "fix"}:
        raise SystemExit("usage: check-staged.py lint|format|fix")
    mode = sys.argv[1]
    root = Path(__file__).resolve().parents[1]
    staged = staged_paths(root)
    python_files = [path for path in staged if path.endswith(".py")]
    rust_files = [path for path in staged if path.endswith(".rs")]

    clean_paths: set[str] = set()
    with tempfile.TemporaryDirectory(prefix="foundation-index-") as temp:
        snapshot = Path(temp)
        run(
            "git",
            "-C",
            str(root),
            "checkout-index",
            "--all",
            "--prefix",
            f"{snapshot}/",
            cwd=root,
        )
        if mode == "fix":
            clean_paths = clean_worktree_paths(root, set(staged))
            rustfmt(snapshot, rust_files)
            if python_files:
                run(
                    "uv",
                    "run",
                    "--locked",
                    "ruff",
                    "check",
                    "--fix",
                    *python_files,
                    cwd=snapshot,
                )
                run(
                    "uv",
                    "run",
                    "--locked",
                    "ruff",
                    "format",
                    *python_files,
                    cwd=snapshot,
                )
        verify(snapshot, python_files, rust_files, "all" if mode == "fix" else mode)
        if mode == "fix":
            apply_snapshot_to_index(root, snapshot, set(staged))
            apply_clean_worktree_fixes(root, snapshot, clean_paths)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
