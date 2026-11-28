import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SKILL = ROOT / "skills" / "decision-review"
RUN_SCRIPT = SKILL / "run.py"
SPEC_SCRIPT = SKILL / "spec.py"


def run_spec(kind: str, source: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "-I", str(SPEC_SCRIPT), kind, str(source)],
        check=False,
        capture_output=True,
        text=True,
    )


def make_fake_foundation(directory: Path) -> Path:
    executable = directory / "foundation"
    executable.write_text(
        "#!/usr/bin/env python3\n"
        "import json, os, pathlib, sys\n"
        "pathlib.Path(os.environ['FAKE_CAPTURE']).write_text(json.dumps({'args':sys.argv[1:], 'spec':sys.stdin.read()}))\n"
        "sys.stdout.write(os.environ.get('FAKE_STDOUT', ''))\n"
        "sys.stderr.write(os.environ.get('FAKE_STDERR', ''))\n"
        "sys.exit(int(os.environ.get('FAKE_EXIT', '0')))\n",
        encoding="utf-8",
    )
    executable.chmod(0o755)
    return executable


def run_review(
    tmp_path: Path,
    provider: str,
    response: object,
    *,
    exit_code: int = 0,
    stdout: str | None = None,
) -> tuple[subprocess.CompletedProcess[str], dict[str, object]]:
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)
    make_fake_foundation(bin_dir)
    source = tmp_path / "input.txt"
    source.write_text("diff and relevant source", encoding="utf-8")
    capture = tmp_path / "capture.json"
    env = {
        **os.environ,
        "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}",
        "FAKE_CAPTURE": str(capture),
        "FAKE_STDOUT": json.dumps(response) if stdout is None else stdout,
        "FAKE_EXIT": str(exit_code),
    }
    result = subprocess.run(
        [sys.executable, "-I", str(RUN_SCRIPT), provider, str(source)],
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )
    return result, json.loads(capture.read_text(encoding="utf-8"))


def questions() -> list[dict[str, Any]]:
    return json.loads((SKILL / "questions.json").read_text(encoding="utf-8"))["dev"]


def provider_response(provider: str, names: list[str]) -> dict[str, Any]:
    if provider in {"typesafe", "microsoft"}:
        return {"answers": {name: {"noul": 0.01 * (index + 1)} for index, name in enumerate(names)}}
    return {
        "answers": [
            {"name": name, "type": "predicate", "probability": 0.01 * (index + 1)}
            for index, name in enumerate(names)
        ]
    }


def test_questions_only_contain_dev_and_build_spec(tmp_path: Path) -> None:
    question_sets = json.loads((SKILL / "questions.json").read_text(encoding="utf-8"))
    source = tmp_path / "input.txt"
    source.write_text("diff and relevant source", encoding="utf-8")

    assert set(question_sets) == {"dev"}
    names = {question["name"] for question in question_sets["dev"]}
    assert names == {
        "effects_mixed_with_decisions",
        "implicit_or_invalid_states",
        "expected_failure_hidden",
        "branching_not_exhaustive",
        "function_has_multiple_responsibilities",
        "shared_mutable_state",
    }
    assert "純粋" in next(
        question["instructions"]
        for question in question_sets["dev"]
        if question["name"] == "shared_mutable_state"
    )

    result = run_spec("dev", source)

    assert result.returncode == 0
    assert json.loads(result.stdout) == {
        "input": "diff and relevant source",
        "questions": question_sets["dev"],
    }
    assert all(question["type"] == "predicate" for question in question_sets["dev"])


def test_review_prints_all_raw_probabilities_for_each_provider(tmp_path: Path) -> None:
    names = [question["name"] for question in questions()]
    for provider in ("typesafe", "microsoft", "openai", "openai-codex"):
        result, captured = run_review(tmp_path / provider, provider, provider_response(provider, names))

        assert result.returncode == 0, result.stderr
        assert result.stdout.splitlines() == [f"{name}: {0.01 * (index + 1)}" for index, name in enumerate(names)]
        assert captured["args"] == ["decide", "--provider", provider, "--spec", "-"]
        assert json.loads(captured["spec"]) == {
            "input": "diff and relevant source",
            "questions": questions(),
        }


def test_review_rejects_incomplete_answers_but_prints_valid_known_values(tmp_path: Path) -> None:
    names = [question["name"] for question in questions()]
    response = provider_response("typesafe", names[:-1])

    result, _ = run_review(tmp_path, "typesafe", response)

    assert result.returncode != 0
    assert result.stdout.splitlines() == [
        f"{name}: {0.01 * (index + 1)}" for index, name in enumerate(names[:-1])
    ]
    assert "未確認" in result.stderr
    assert f"{names[-1]}: missing answer" in result.stderr


def test_review_ignores_unknown_answer_fields(tmp_path: Path) -> None:
    names = [question["name"] for question in questions()]
    response = provider_response("typesafe", names)
    response["answers"]["provider_metadata"] = {"noul": 0.99}
    response["metadata"] = {"noul": 0.88}

    result, _ = run_review(tmp_path, "typesafe", response)

    assert result.returncode == 0, result.stderr
    assert result.stdout.splitlines() == [
        f"{name}: {0.01 * (index + 1)}" for index, name in enumerate(names)
    ]


def test_review_rejects_duplicate_known_answers_and_prints_other_values(tmp_path: Path) -> None:
    names = [question["name"] for question in questions()]
    response = provider_response("openai", names)
    response["answers"].append(response["answers"][0].copy())

    result, _ = run_review(tmp_path, "openai", response)

    assert result.returncode != 0
    assert result.stdout.splitlines() == [
        f"{name}: {0.01 * (index + 1)}" for index, name in enumerate(names[1:], 1)
    ]
    assert f"{names[0]}: duplicate answer" in result.stderr


def test_review_rejects_command_error_even_with_valid_output(tmp_path: Path) -> None:
    names = [question["name"] for question in questions()]

    result, _ = run_review(
        tmp_path,
        "typesafe",
        provider_response("typesafe", names),
        exit_code=1,
    )

    assert result.returncode == 1
    assert result.stdout == ""
    assert "未確認" in result.stderr


def test_review_rejects_invalid_probability_and_non_json(tmp_path: Path) -> None:
    names = [question["name"] for question in questions()]
    invalid = provider_response("typesafe", names)
    invalid["answers"][names[0]]["noul"] = 1.2

    invalid_result, _ = run_review(tmp_path / "invalid", "typesafe", invalid)
    malformed_result, _ = run_review(tmp_path / "malformed", "typesafe", {}, stdout="not json")

    assert invalid_result.returncode != 0
    assert invalid_result.stdout.splitlines() == [
        f"{name}: {0.01 * (index + 1)}" for index, name in enumerate(names[1:], 1)
    ]
    assert "未確認" in invalid_result.stderr
    assert f"{names[0]}: invalid or missing probability" in invalid_result.stderr
    assert malformed_result.returncode != 0
    assert malformed_result.stdout == ""
    assert "未確認" in malformed_result.stderr


def test_installed_script_runs_outside_repository(tmp_path: Path) -> None:
    hermes_home = tmp_path / "fake-hermes"
    installed_skill = hermes_home / "foundation" / "skills" / "decision-review"
    installed_skill.mkdir(parents=True)
    shutil.copy2(RUN_SCRIPT, installed_skill / "run.py")
    shutil.copy2(SKILL / "questions.json", installed_skill / "questions.json")
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    make_fake_foundation(bin_dir)
    source = tmp_path / "input.txt"
    source.write_text("installed invocation", encoding="utf-8")
    names = [question["name"] for question in questions()]
    env = {
        **os.environ,
        "HERMES_HOME": str(hermes_home),
        "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}",
        "FAKE_CAPTURE": str(tmp_path / "capture.json"),
        "FAKE_STDOUT": json.dumps(provider_response("typesafe", names)),
    }

    result = subprocess.run(
        [sys.executable, "-I", str(installed_skill / "run.py"), "typesafe", str(source)],
        cwd=tmp_path,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert len(result.stdout.splitlines()) == len(names)


def test_unknown_question_set_fails_without_emitting_a_spec(tmp_path: Path) -> None:
    source = tmp_path / "input.txt"
    source.write_text("input", encoding="utf-8")

    result = run_spec("unknown", source)

    assert result.returncode == 2
    assert result.stdout == ""
    assert "usage:" in result.stderr