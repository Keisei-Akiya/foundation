import json
import os
import shutil
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SKILL = ROOT / "skills" / "decision-review"
SPEC_SCRIPT = SKILL / "spec.py"


def run_spec(kind: str, source: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "-I", str(SPEC_SCRIPT), kind, str(source)],
        check=False,
        capture_output=True,
        text=True,
    )


def test_question_sets_build_foundation_decide_specs(tmp_path: Path) -> None:
    question_sets = json.loads((SKILL / "questions.json").read_text(encoding="utf-8"))
    source = tmp_path / "input.txt"
    source.write_text("diff and relevant source", encoding="utf-8")

    assert set(question_sets) == {"doc", "test", "fp"}
    assert {question["name"] for question in question_sets["doc"]} == {
        "hardcoded_version",
        "duplicate_statement",
    }
    assert {question["name"] for question in question_sets["test"]} == {
        "custom_class"
    }
    assert {question["name"] for question in question_sets["fp"]} == {
        "effects_mixed_with_decisions",
        "implicit_or_invalid_states",
        "expected_failure_hidden",
        "branching_not_exhaustive",
        "function_has_multiple_responsibilities",
        "shared_mutable_state",
    }
    assert "manual_collection_transformation" not in {
        question["name"] for question in question_sets["fp"]
    }
    assert "純粋" in next(
        question["instructions"]
        for question in question_sets["fp"]
        if question["name"] == "shared_mutable_state"
    )
    for kind, questions in question_sets.items():
        result = run_spec(kind, source)

        assert result.returncode == 0
        assert json.loads(result.stdout) == {
            "input": "diff and relevant source",
            "questions": questions,
        }
        assert all(question["type"] == "predicate" for question in questions)


def test_installed_spec_script_runs_from_outside_repository(tmp_path: Path) -> None:
    hermes_home = tmp_path / "fake-hermes"
    installed_skill = hermes_home / "foundation" / "skills" / "decision-review"
    installed_skill.mkdir(parents=True)
    shutil.copy2(SPEC_SCRIPT, installed_skill / "spec.py")
    shutil.copy2(SKILL / "questions.json", installed_skill / "questions.json")
    source = tmp_path / "input.txt"
    source.write_text("installed invocation", encoding="utf-8")

    result = subprocess.run(
        [
            sys.executable,
            "-I",
            str(installed_skill / "spec.py"),
            "fp",
            str(source),
        ],
        cwd=tmp_path,
        env={**os.environ, "HERMES_HOME": str(hermes_home)},
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    spec = json.loads(result.stdout)
    assert spec["input"] == "installed invocation"
    assert len(spec["questions"]) == 6


def test_fp_questions_are_language_neutral() -> None:
    question_sets = json.loads((SKILL / "questions.json").read_text(encoding="utf-8"))
    fp_questions = question_sets["fp"]

    assert len(fp_questions) >= 5
    assert not any("Rust" in question["instructions"] for question in fp_questions)
    assert len({question["name"] for question in fp_questions}) == len(fp_questions)


def test_unknown_question_set_fails_without_emitting_a_spec(tmp_path: Path) -> None:
    source = tmp_path / "input.txt"
    source.write_text("input", encoding="utf-8")

    result = run_spec("unknown", source)

    assert result.returncode == 2
    assert result.stdout == ""
    assert "usage:" in result.stderr
