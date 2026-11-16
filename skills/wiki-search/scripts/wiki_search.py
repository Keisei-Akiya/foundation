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


def _select_question_result(response):
    if not isinstance(response, dict):
        raise ValueError("invalid Jev response: expected a JSON object")
    probabilities = response.get("probabilities")
    legend = response.get("legend")
    if not isinstance(probabilities, dict) or not probabilities:
        raise ValueError("invalid Jev response: probabilities must be a nonempty dictionary")
    if not isinstance(legend, dict) or not legend:
        raise ValueError("invalid Jev response: legend must be a nonempty dictionary")
    # foundation decide returns maps by question index; accept a direct level map too.
    if "0" in probabilities and isinstance(probabilities["0"], dict):
        probabilities = probabilities["0"]
    if "0" in legend and isinstance(legend["0"], dict):
        legend = legend["0"]
    if not isinstance(probabilities, dict) or not probabilities:
        raise ValueError("invalid Jev response: probabilities must be a nonempty dictionary")
    for key, value in probabilities.items():
        if not isinstance(key, str) or not key.isdecimal():
            raise ValueError("invalid Jev response: probability keys must be integer indices")
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or not 0 <= value <= 1:
            raise ValueError("invalid Jev response: probabilities must be between 0 and 1")
    if not isinstance(legend, dict):
        raise ValueError("invalid Jev response: legend must be a dictionary")
    labels = {}
    for key, label in legend.items():
        if not isinstance(key, str) or not key.isdecimal():
            raise ValueError("invalid Jev response: legend keys must be integer indices")
        if isinstance(label, dict):
            label = label.get("label")
        if isinstance(label, str):
            labels[key] = label.lower()
    by_label = {label: probabilities[key] for key, label in labels.items() if key in probabilities}
    if "low" not in by_label or "high" not in by_label:
        raise ValueError("invalid Jev response: legend must identify low and high probabilities")
    return by_label["low"] + by_label["high"]


def score_batch(question, pages):
    spec = {
        "input": question,
        "questions": [
            {
                "name": str(index),
                "type": "score",
                "instructions": (
                    f"このページを選ぶべきか採点する。\n"
                    f"タイトル: {page['title']}\n要約: {page['summary']}"
                ),
                "levels": LEVELS,
            }
            for index, page in enumerate(pages)
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
    scores = []
    for index in range(len(pages)):
        item = dict(response)
        probs = response.get("probabilities")
        legends = response.get("legend")
        if isinstance(probs, dict) and isinstance(legends, dict):
            key = str(index)
            if key in probs and isinstance(probs[key], dict):
                item["probabilities"] = {"0": probs[key]}
                item["legend"] = {"0": legends.get(key)} if key in legends else legends
            elif index > 0:
                raise ValueError(f"invalid Jev response: missing result for page {index}")
        scores.append(_select_question_result(item))
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
