"""Rust ソースから `foundation decide` の spec を作り、応答を判定表で読む。

usage:
  python3 -I review.py spec <file.rs>... > spec.json
  python3 -I review.py judge <response.json>
"""

import json
import sys
from pathlib import Path

YES_ABOVE = 0.7
NO_BELOW = 0.3

QUESTIONS_PATH = Path(__file__).parent / "questions.json"


def build_spec(files: dict[str, str], questions: list[dict]) -> dict:
    return {"input": {"files": files}, "questions": questions}


def verdict(noul: float) -> str:
    if noul >= YES_ABOVE:
        return "yes"
    if noul <= NO_BELOW:
        return "no"
    return "unsettled"


def judge(noul_by_name: dict[str, float]) -> dict[str, str]:
    return {name: verdict(noul) for name, noul in noul_by_name.items()}


def read_noul(response: dict) -> dict[str, float]:
    return {name: float(answer["noul"]) for name, answer in response["answers"].items()}


def render(verdicts: dict[str, str]) -> str:
    lines = [f"{name}: {v}" for name, v in verdicts.items() if v != "no"]
    return "\n".join(lines) if lines else "指摘候補なし"


def main(argv: list[str]) -> int:
    if len(argv) >= 3 and argv[1] == "spec":
        questions = json.loads(QUESTIONS_PATH.read_text(encoding="utf-8"))
        files = {Path(p).name: Path(p).read_text(encoding="utf-8") for p in argv[2:]}
        print(json.dumps(build_spec(files, questions), ensure_ascii=False))
        return 0
    if len(argv) == 3 and argv[1] == "judge":
        response = json.loads(Path(argv[2]).read_text(encoding="utf-8"))
        print(render(judge(read_noul(response))))
        return 0
    print("usage: review.py spec <file.rs>... | judge <response.json>", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
