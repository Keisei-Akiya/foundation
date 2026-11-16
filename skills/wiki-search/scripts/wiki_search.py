#!/usr/bin/env python3
"""Score wiki pages against a rewritten question using foundation decide."""

import argparse
import json
import math
import re
import subprocess
import sys
from pathlib import Path

BATCH_SIZE = 50
DEFAULT_THRESHOLD = 0.2
DEFAULT_MAX_RESULTS = 10
LEVELS = [{"label": "none"}, {"label": "low"}, {"label": "high"}]


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wiki", required=True, type=Path, help="wiki root directory")
    parser.add_argument("--threshold", type=float, default=DEFAULT_THRESHOLD,
                        help="minimum P(low)+P(high), default %(default)s")
    parser.add_argument("--max-results", type=int, default=DEFAULT_MAX_RESULTS,
                        help="maximum number of pages to print, default %(default)s")
    parser.add_argument("question", help="rewritten, context-complete question")
    args = parser.parse_args()
    if not 0 <= args.threshold <= 1:
        parser.error("--threshold must be between 0 and 1")
    if args.max_results < 1:
        parser.error("--max-results must be at least 1")
    return args


def _unquote(value):
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
        value = value[1:-1]
    return value


def _frontmatter_title(path):
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return None
    match = re.match(r"\A---\s*\n(.*?)\n---(?:\s*\n|\Z)", text, re.DOTALL)
    if match:
        title = re.search(r"(?m)^title:\s*(.*?)\s*$", match.group(1))
        if title and title.group(1).strip():
            return _unquote(title.group(1))
    heading = re.search(r"(?m)^#\s+(.+?)\s*$", text)
    return heading.group(1) if heading else None


def _index_summaries(wiki):
    summaries = {}
    index = wiki / "index.md"
    if not index.is_file():
        return summaries
    try:
        text = index.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return summaries
    for line in text.splitlines():
        match = re.match(r"\s*-\s+\[\[([^]|#]+)(?:\|[^]]*)?\]\]\s*(?:[—–-]\s*(.*))?$", line)
        if match:
            summaries[match.group(1).strip()] = (match.group(2) or "").strip()
    return summaries


def discover_pages(wiki):
    if not wiki.is_dir():
        raise ValueError(f"wiki directory does not exist: {wiki}")
    summaries = _index_summaries(wiki)
    pages = []
    for path in sorted(wiki.rglob("*.md")):
        relative = path.relative_to(wiki)
        if any(part.startswith(".") or part in {"raw", ".git"} for part in relative.parts):
            continue
        if relative.as_posix() in {"index.md", "SCHEMA.md", "log.md"}:
            continue
        title = _frontmatter_title(path)
        if not title:
            continue
        pages.append({
            "path": relative.as_posix(),
            "title": title,
            "summary": summaries.get(path.stem, ""),
        })
    return pages


def _select_question_result(answer):
    if not isinstance(answer, dict):
        raise ValueError("invalid Jev response: question answer must be an object")
    probabilities = answer.get("probabilities")
    legend = answer.get("legend")
    if not isinstance(probabilities, dict) or not probabilities:
        raise ValueError("invalid Jev response: probabilities must be a nonempty dictionary")
    if not isinstance(legend, dict) or not legend:
        raise ValueError("invalid Jev response: legend must be a nonempty dictionary")
    for key, value in probabilities.items():
        if not isinstance(key, str) or not key.isdecimal():
            raise ValueError("invalid Jev response: probability keys must be integer indices")
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or not 0 <= value <= 1:
            raise ValueError("invalid Jev response: probabilities must be between 0 and 1")
    by_label = {}
    for key, label in legend.items():
        if not isinstance(key, str) or not key.isdecimal() or not isinstance(label, str):
            raise ValueError("invalid Jev response: legend must map integer indices to labels")
        normalized = label.lower()
        if normalized in {"none", "low", "high"}:
            if key not in probabilities:
                raise ValueError(f"invalid Jev response: missing probability for legend label {normalized}")
            if normalized in by_label:
                raise ValueError(f"invalid Jev response: duplicate legend label {normalized}")
            by_label[normalized] = probabilities[key]
    if not all(label in by_label for label in ("none", "low", "high")):
        raise ValueError("invalid Jev response: legend must identify none, low, and high probabilities")
    return by_label["low"] + by_label["high"]


def score_batch(question, pages):
    spec = {
        "input": question,
        "questions": [
            {
                "name": f"page:{page['path']}",
                "type": "score",
                "instructions": (
                    f"このページを選ぶべきか採点する。\n"
                    f"タイトル: {page['title']}\n要約: {page['summary']}"
                ),
                "levels": LEVELS,
            }
            for page in pages
        ],
    }
    try:
        result = subprocess.run(
            ["foundation", "decide", "--spec", "-"], input=json.dumps(spec, ensure_ascii=False),
            text=True, capture_output=True, check=False,
        )
    except OSError as exc:
        raise ValueError(f"cannot run foundation decide: {exc}") from exc
    if result.returncode != 0:
        raise ValueError(f"foundation decide failed ({result.returncode}): {result.stderr.strip()}")
    try:
        response = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid Jev response: output is not JSON: {exc}") from exc
    answers = response.get("answers") if isinstance(response, dict) else None
    if not isinstance(answers, dict):
        raise ValueError("invalid Jev response: answers must be a dictionary")
    scores = []
    for page in pages:
        name = f"page:{page['path']}"
        if name not in answers:
            raise ValueError(f"invalid Jev response: missing answer for question {name}")
        scores.append(_select_question_result(answers[name]))
    return scores


def search(question, wiki, threshold, max_results):
    pages = discover_pages(wiki)
    if not pages:
        raise ValueError(f"no wiki pages found in {wiki}")
    scored = []
    for start in range(0, len(pages), BATCH_SIZE):
        batch = pages[start:start + BATCH_SIZE]
        scored.extend((score, page) for score, page in zip(score_batch(question, batch), batch))
    selected = [item for item in scored if item[0] >= threshold]
    if not selected:
        selected = [max(scored, key=lambda item: item[0])]
    selected.sort(key=lambda item: item[0], reverse=True)
    return selected[:max_results]


def main():
    args = parse_args()
    try:
        results = search(args.question, args.wiki, args.threshold, args.max_results)
    except (OSError, ValueError) as exc:
        print(f"wiki-search: {exc}", file=sys.stderr)
        return 1
    for score, page in results:
        print(f"{score:.2f}\t{page['path']}\t{page['title']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
