#!/usr/bin/env python3
"""Run the decision-review dev questions and print every raw probability."""

import argparse
import json
import math
import shutil
import subprocess
import sys
from pathlib import Path


SKILL_DIR = Path(__file__).resolve().parent


def probability(value: object) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value) and 0 <= value <= 1


def extract_probabilities(
    provider: str, response: object, question_names: list[str]
) -> tuple[list[tuple[str, float]], list[str]]:
    if not isinstance(response, dict) or not isinstance(response.get("answers"), (dict, list)):
        raise ValueError("answers must be an object or array")

    answers = response["answers"]
    known_names = set(question_names)
    candidates: dict[str, object] = {}
    duplicates: set[str] = set()

    if provider in {"typesafe", "microsoft"}:
        if not isinstance(answers, dict):
            raise ValueError("this provider requires answers to be an object")
        for name, answer in answers.items():
            if name not in known_names:
                continue
            if name in candidates:
                duplicates.add(name)
                continue
            candidates[name] = answer.get("noul") if isinstance(answer, dict) else None
    else:
        if not isinstance(answers, list):
            raise ValueError("this provider requires answers to be an array")
        for answer in answers:
            if not isinstance(answer, dict) or not isinstance(answer.get("name"), str):
                continue
            name = answer["name"]
            if name not in known_names:
                continue
            if name in candidates:
                duplicates.add(name)
                continue
            candidates[name] = (
                answer.get("probability")
                if answer.get("type") == "predicate"
                else None
            )

    output: list[tuple[str, float]] = []
    issues: list[str] = []
    for name in question_names:
        if name in duplicates:
            issues.append(f"{name}: duplicate answer")
        elif name not in candidates:
            issues.append(f"{name}: missing answer")
        elif not probability(candidates[name]):
            issues.append(f"{name}: invalid or missing probability")
        else:
            output.append((name, candidates[name]))
    return output, issues


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("provider", choices=("typesafe", "microsoft", "openai", "openai-codex"))
    parser.add_argument("input_file", type=Path)
    args = parser.parse_args()

    try:
        input_text = args.input_file.read_text(encoding="utf-8")
        questions = json.loads((SKILL_DIR / "questions.json").read_text(encoding="utf-8"))["dev"]
    except (OSError, UnicodeError, json.JSONDecodeError, KeyError, TypeError) as error:
        print(f"未確認: 入力または質問セットを読み込めない: {error}", file=sys.stderr)
        return 1

    executable = shutil.which("foundation")
    if executable is None:
        print("未確認: foundation が PATH に見つからない", file=sys.stderr)
        return 1

    spec = json.dumps({"input": input_text, "questions": questions}, ensure_ascii=False)
    try:
        result = subprocess.run(
            [executable, "decide", "--provider", args.provider, "--spec", "-"],
            input=spec,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        print(f"未確認: foundation decide を起動できない: {error}", file=sys.stderr)
        return 1

    if result.stderr:
        print(result.stderr, file=sys.stderr, end="" if result.stderr.endswith("\n") else "\n")
    if result.returncode != 0:
        print(f"未確認: foundation decide が終了コード {result.returncode} を返した", file=sys.stderr)
        return result.returncode if result.returncode > 0 else 1

    try:
        response = json.loads(result.stdout)
        probabilities, issues = extract_probabilities(
            args.provider, response, [question["name"] for question in questions]
        )
    except (json.JSONDecodeError, ValueError, TypeError) as error:
        print(f"未確認: 応答が不正: {error}", file=sys.stderr)
        return 1

    for name, value in probabilities:
        print(f"{name}: {value}")
    if issues:
        print(f"未確認: 回答を確認できない質問: {', '.join(issues)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
