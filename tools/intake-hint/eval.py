#!/usr/bin/env python3
"""intake-hint の発話分類を `foundation decide` に通し、フラグごとの誤検出・見逃しを数える。

標準ライブラリだけで動く。使い方は README.md を参照する。
結果ファイルには発話本文を書かない（id と回答だけ）。
"""

import argparse
import json
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
QUESTIONS_DIR = HERE / "questions"
PREDICATES = ("proposal", "new_task", "discuss", "standing_rule")
REPLY_VALUES = ("approve", "revise", "decline", "none")
SOURCES = ("synthetic", "real")
UNCERTAIN_BAND = (0.3, 0.7)


class EvalError(Exception):
    pass


def repo_root(path: Path):
    """path が属する git 作業ツリーの根を返す。なければ None。"""
    for parent in [path.resolve(), *path.resolve().parents]:
        if (parent / ".git").exists():
            return parent
    return None


def load_questions(name: str) -> list:
    path = Path(name)
    if not path.is_file():
        path = QUESTIONS_DIR / f"{name}.json"
    if not path.is_file():
        raise EvalError(f"questions not found: {name}")
    questions = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(questions, list) or not questions:
        raise EvalError(f"questions must be a non-empty array: {path}")
    return questions


def validate_record(rec, where: str) -> None:
    if not isinstance(rec, dict):
        raise EvalError(f"{where}: must be an object")
    for key in ("id", "source", "user_message", "labels"):
        if key not in rec:
            raise EvalError(f"{where}: `{key}` is required")
    if rec["source"] not in SOURCES:
        raise EvalError(f"{where}: source must be one of {SOURCES}")
    if not isinstance(rec["user_message"], str) or not rec["user_message"].strip():
        raise EvalError(f"{where}: user_message must be a non-empty string")
    ai = rec.get("previous_ai_message")
    if ai is not None and not isinstance(ai, str):
        raise EvalError(f"{where}: previous_ai_message must be a string or null")
    labels = rec["labels"]
    if not isinstance(labels, dict):
        raise EvalError(f"{where}: labels must be an object")
    for flag in PREDICATES:
        if not isinstance(labels.get(flag), bool):
            raise EvalError(f"{where}: labels.{flag} must be a boolean")
    if labels.get("reply") not in REPLY_VALUES:
        raise EvalError(f"{where}: labels.reply must be one of {REPLY_VALUES}")


def load_dataset(path_str: str) -> list:
    path = Path(path_str)
    records = []
    seen = set()
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        where = f"{path}:{n}"
        try:
            rec = json.loads(line)
        except json.JSONDecodeError as e:
            raise EvalError(f"{where}: invalid JSON: {e}")
        validate_record(rec, where)
        if rec["id"] in seen:
            raise EvalError(f"{where}: duplicate id `{rec['id']}`")
        seen.add(rec["id"])
        records.append(rec)
    if not records:
        raise EvalError(f"{path}: no records")
    root = repo_root(path)
    if root is not None and any(r["source"] == "real" for r in records):
        raise EvalError(
            f"{path}: source=real の発話は git 作業ツリー（{root}）の中に置けない。"
            "リポジトリの外の非公開の場所に置く。"
        )
    return records


def build_spec(rec: dict, questions: list) -> dict:
    return {
        "input": {
            "previous_ai_message": rec.get("previous_ai_message"),
            "user_message": rec["user_message"],
        },
        "questions": questions,
    }


def run_decide(foundation: str, spec: dict, timeout: int) -> dict:
    return json.loads(decide_raw(foundation, spec, timeout))


def decide_raw(foundation: str, spec: dict, timeout: int, dry_run: bool = False) -> str:
    cmd = [foundation, "decide", "--timeout", str(timeout)]
    if dry_run:
        cmd.append("--dry-run")
    proc = subprocess.run(
        cmd,
        input=json.dumps(spec, ensure_ascii=False),
        capture_output=True,
        text=True,
        timeout=timeout + 30,
    )
    if proc.returncode != 0:
        raise EvalError(f"decide exit {proc.returncode}: {proc.stderr.strip()[:200]}")
    return proc.stdout


def query_all(records, variants, foundation, timeout, jobs):
    """(record, variant) ごとに decide を呼ぶ。結果行（本文なし）のリストを返す。"""
    tasks = [(r, v, q) for v, q in variants for r in records]

    def one(task):
        rec, variant, questions = task
        row = {"id": rec["id"], "source": rec["source"], "variant": variant}
        try:
            row["answers"] = run_decide(foundation, build_spec(rec, questions), timeout)["answers"]
        except (EvalError, KeyError, json.JSONDecodeError, subprocess.TimeoutExpired) as e:
            row["error"] = str(e)[:200]
        return row

    with ThreadPoolExecutor(max_workers=max(1, jobs)) as pool:
        return list(pool.map(one, tasks))


def score(records, rows, threshold):
    """variant -> source -> 集計。rows は query_all の結果。"""
    by_id = {r["id"]: r for r in records}
    report = {}
    for row in rows:
        rec = by_id.get(row["id"])
        if rec is None:
            continue
        cell = report.setdefault(row["variant"], {}).setdefault(
            rec["source"], {"n": 0, "errors": 0, "flags": {}, "reply": {"correct": 0, "wrong": 0, "wrong_ids": []}}
        )
        cell["n"] += 1
        answers = row.get("answers")
        if answers is None:
            cell["errors"] += 1
            cell.setdefault("first_error", row.get("error", "no answers"))
            continue
        for flag in PREDICATES:
            ans = answers.get(flag)
            if ans is None:
                continue
            p = float(ans["noul"])
            want = rec["labels"][flag]
            got = p >= threshold
            f = cell["flags"].setdefault(
                flag, {"tp": 0, "fp": 0, "fn": 0, "tn": 0, "uncertain": 0, "fp_ids": [], "fn_ids": []}
            )
            if want and got:
                f["tp"] += 1
            elif want:
                f["fn"] += 1
                f["fn_ids"].append(rec["id"])
            elif got:
                f["fp"] += 1
                f["fp_ids"].append(rec["id"])
            else:
                f["tn"] += 1
            if UNCERTAIN_BAND[0] <= p <= UNCERTAIN_BAND[1]:
                f["uncertain"] += 1
        reply = answers.get("reply")
        if reply is not None:
            key = "correct" if reply.get("choice") == rec["labels"]["reply"] else "wrong"
            cell["reply"][key] += 1
            if key == "wrong":
                cell["reply"]["wrong_ids"].append(rec["id"])
    return report


def format_report(report, threshold) -> str:
    lines = [f"threshold={threshold}  uncertain={UNCERTAIN_BAND[0]}-{UNCERTAIN_BAND[1]}"]
    for variant in sorted(report):
        for source in SOURCES:
            cell = report[variant].get(source)
            if cell is None:
                continue
            lines.append("")
            lines.append(f"== {variant} / {source}  n={cell['n']} errors={cell['errors']} ==")
            if cell.get("first_error"):
                lines.append(f"first error: {cell['first_error']}")
            lines.append(f"{'flag':<14}{'TP':>4}{'FP':>4}{'FN':>4}{'TN':>4}{'uncertain':>10}")
            for flag in PREDICATES:
                f = cell["flags"].get(flag)
                if f is None:
                    continue
                lines.append(
                    f"{flag:<14}{f['tp']:>4}{f['fp']:>4}{f['fn']:>4}{f['tn']:>4}{f['uncertain']:>10}"
                )
            r = cell["reply"]
            if r["correct"] + r["wrong"]:
                lines.append(f"{'reply':<14}correct={r['correct']} wrong={r['wrong']}")
            for flag in PREDICATES:
                f = cell["flags"].get(flag)
                if f and (f["fp_ids"] or f["fn_ids"]):
                    lines.append(f"  {flag} FP: {' '.join(f['fp_ids']) or '-'}")
                    lines.append(f"  {flag} FN: {' '.join(f['fn_ids']) or '-'}")
            if r["wrong_ids"]:
                lines.append(f"  reply wrong: {' '.join(r['wrong_ids'])}")
    return "\n".join(lines)


def load_datasets(paths):
    records = []
    seen = set()
    for p in paths:
        for rec in load_dataset(p):
            if rec["id"] in seen:
                raise EvalError(f"duplicate id across datasets: {rec['id']}")
            seen.add(rec["id"])
            records.append(rec)
    return records


def cmd_validate(args):
    records = load_datasets(args.dataset)
    counts = {s: sum(1 for r in records if r["source"] == s) for s in SOURCES}
    print(f"ok: {len(records)} records {counts}")
    for name in args.variants.split(","):
        load_questions(name)
    return 0


def cmd_run(args):
    records = load_datasets(args.dataset)
    variants = [(v, load_questions(v)) for v in args.variants.split(",")]
    foundation = args.foundation or os.environ.get("FOUNDATION_BIN") or "foundation"
    if args.dry_run:
        out = decide_raw(foundation, build_spec(records[0], variants[0][1]), args.timeout, dry_run=True)
        print(out)
        return 0
    rows = query_all(records, variants, foundation, args.timeout, args.jobs)
    if args.out:
        Path(args.out).write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in rows), encoding="utf-8")
    print(format_report(score(records, rows, args.threshold), args.threshold))
    return 0


def cmd_report(args):
    records = load_datasets(args.dataset)
    rows = [json.loads(line) for line in Path(args.results).read_text(encoding="utf-8").splitlines() if line.strip()]
    print(format_report(score(records, rows, args.threshold), args.threshold))
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    def common(p):
        p.add_argument("--dataset", action="append", required=True, help="JSONL。複数指定できる")

    v = sub.add_parser("validate", help="発話セットと質問セットの形式を検査する（通信しない）")
    common(v)
    v.add_argument("--variants", default="baseline4,narrow4,five")
    v.set_defaults(fn=cmd_validate)

    r = sub.add_parser("run", help="decide に通して集計する")
    common(r)
    r.add_argument("--variants", default="narrow4,five", help="questions/ の名前かファイル。カンマ区切り")
    r.add_argument("--foundation", help="foundation バイナリ。省略時は FOUNDATION_BIN か PATH 上の foundation")
    r.add_argument("--threshold", type=float, default=0.5)
    r.add_argument("--jobs", type=int, default=4)
    r.add_argument("--timeout", type=int, default=60)
    r.add_argument("--out", help="結果（id と回答のみ）の保存先")
    r.add_argument("--dry-run", action="store_true", help="先頭1件の送信先と本文だけ表示する")
    r.set_defaults(fn=cmd_run)

    rp = sub.add_parser("report", help="保存済みの結果から再集計する（通信しない）")
    common(rp)
    rp.add_argument("--results", required=True)
    rp.add_argument("--threshold", type=float, default=0.5)
    rp.set_defaults(fn=cmd_report)

    args = ap.parse_args(argv)
    try:
        return args.fn(args)
    except EvalError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
