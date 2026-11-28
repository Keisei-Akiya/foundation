"""レビュー対象と質問セットから `foundation decide` の spec を標準出力へ出す。

usage: python3 -I spec.py <dev> <input-file>
"""

import json
import sys
from pathlib import Path


def build_spec(kind: str, text: str, questions: dict) -> dict:
    return {"input": text, "questions": questions[kind]}


def main(argv: list[str]) -> int:
    questions = json.loads((Path(__file__).parent / "questions.json").read_text(encoding="utf-8"))
    if len(argv) != 3 or argv[1] not in questions:
        print(f"usage: spec.py <{'|'.join(questions)}> <input-file>", file=sys.stderr)
        return 2
    text = Path(argv[2]).read_text(encoding="utf-8")
    print(json.dumps(build_spec(argv[1], text, questions), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
