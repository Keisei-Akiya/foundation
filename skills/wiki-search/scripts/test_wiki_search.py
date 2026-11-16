import json
import os
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("wiki_search.py")
FAKE_FOUNDATION = r'''#!/usr/bin/env python3
import json, os, sys
spec = json.load(sys.stdin)
with open(os.environ["FAKE_CALLS"], "a", encoding="utf-8") as f:
    f.write(json.dumps(spec, ensure_ascii=False) + "\n")
responses = json.loads(os.environ["FAKE_RESPONSES"])
call = sum(1 for _ in open(os.environ["FAKE_CALLS"], encoding="utf-8")) - 1
print(json.dumps(responses[call]))
'''


def response(scores):
    # foundation decide の固定応答形式: 問い合わせ番号 -> 添字確率と legend。
    probs = {str(i): {"0": 1 - low - high, "1": low, "2": high}
             for i, (low, high) in enumerate(scores)}
    legend = {str(i): {"0": "none", "1": "low", "2": "high"}
              for i in range(len(scores))}
    return {"probabilities": probs, "legend": legend}


class WikiSearchCliTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.wiki = self.root / "wiki"
        (self.wiki / "concepts").mkdir(parents=True)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        fake = self.bin / "foundation"
        fake.write_text(FAKE_FOUNDATION, encoding="utf-8")
        fake.chmod(0o755)
        self.calls = self.root / "calls.jsonl"

    def add_page(self, slug, title, body, domains="[engineering, home]"):
        (self.wiki / "concepts" / f"{slug}.md").write_text(
            f"---\ntitle: {title}\ndomains: {domains}\n---\n# {title}\n{body}\n",
            encoding="utf-8",
        )

    def run_search(self, scores, *args, expect_ok=True):
        env = os.environ.copy()
        env["PATH"] = str(self.bin) + os.pathsep + env["PATH"]
        env["FAKE_CALLS"] = str(self.calls)
        env["FAKE_RESPONSES"] = json.dumps([response(s) for s in scores])
        proc = subprocess.run(
            ["python3", "-I", str(SCRIPT), "--wiki", str(self.wiki), *args,
             "この文脈を補った質問"],
            text=True, capture_output=True, env=env,
        )
        if expect_ok:
            self.assertEqual(proc.returncode, 0, proc.stderr)
        else:
            self.assertNotEqual(proc.returncode, 0)
        return proc

    def read_calls(self):
        return [json.loads(line) for line in self.calls.read_text(encoding="utf-8").splitlines()]

    def test_threshold_matches_are_one_line_each_sorted_by_score(self):
        self.add_page("alpha", "Alpha", "SECRET BODY A")
        self.add_page("beta", "Beta", "SECRET BODY B")
        self.add_page("gamma", "Gamma", "SECRET BODY C")
        proc = self.run_search([[(0.1, 0.4), (0.2, 0.6), (0.1, 0.1)]], "--threshold", "0.5")
        lines = proc.stdout.strip().splitlines()
        self.assertEqual(len(lines), 2)
        self.assertIn("0.80", lines[0])
        self.assertTrue(lines[0].endswith("concepts/beta.md\tBeta"))
        self.assertIn("0.50", lines[1])
        self.assertTrue(lines[1].endswith("concepts/alpha.md\tAlpha"))

    def test_no_page_over_threshold_returns_highest_single_page(self):
        self.add_page("alpha", "Alpha", "body")
        self.add_page("beta", "Beta", "body")
        proc = self.run_search([[(0.1, 0.1), (0.2, 0.2)]], "--threshold", "0.9")
        self.assertEqual(len(proc.stdout.strip().splitlines()), 1)
        self.assertTrue(proc.stdout.strip().endswith("concepts/beta.md\tBeta"))

    def test_runtime_threshold_and_max_results_are_applied(self):
        for i in range(4):
            self.add_page(f"p{i}", f"Page {i}", "body")
        proc = self.run_search([[(0.3, 0.5), (0.2, 0.5), (0.3, 0.3), (0.1, 0.2)]],
                               "--threshold", "0.6", "--max-results", "1")
        self.assertEqual(len(proc.stdout.strip().splitlines()), 1)
        self.assertTrue(proc.stdout.strip().endswith("concepts/p0.md\tPage 0"))

    def test_request_contains_question_and_titles_but_never_page_body(self):
        self.add_page("alpha", "Alpha title", "DO_NOT_SEND_BODY")
        (self.wiki / "index.md").write_text("# Index\n- [[alpha]] — Alpha summary\n", encoding="utf-8")
        self.run_search([[(0.1, 0.7)]])
        serialized = json.dumps(self.read_calls(), ensure_ascii=False)
        self.assertIn("この文脈を補った質問", serialized)
        self.assertIn("Alpha title", serialized)
        self.assertIn("Alpha summary", serialized)
        self.assertNotIn("DO_NOT_SEND_BODY", serialized)

    def test_batches_near_fifty_and_deduplicates_multidomain_pages(self):
        for i in range(51):
            self.add_page(f"p{i:02}", f"Page {i:02}", "body", "[engineering, home]")
        (self.wiki / "index.md").write_text("# Index\n" + "".join(
            f"- [[p{i:02}]] — Summary {i:02}\n" for i in range(51)), encoding="utf-8")
        proc = self.run_search([[(0.1, 0.4)] * 50, [(0.2, 0.5)]], "--max-results", "60")
        calls = self.read_calls()
        self.assertEqual(len(calls), 2)
        self.assertEqual([len(call["questions"]) for call in calls], [50, 1])
        self.assertEqual(len(proc.stdout.strip().splitlines()), 51)
        self.assertEqual(sum(len(call["questions"]) for call in calls), 51)

    def test_malformed_probability_responses_fail_clearly(self):
        self.add_page("alpha", "Alpha", "body")
        for invalid in [
            {"probabilities": [], "legend": {}},
            {"probabilities": {"0": {"low": 0.4}}, "legend": {"0": {"1": "low"}}},
            {"probabilities": {"0": {"1": 1.2}}, "legend": {"0": {"1": "low"}}},
        ]:
            env = os.environ.copy()
            env["PATH"] = str(self.bin) + os.pathsep + env["PATH"]
            env["FAKE_CALLS"] = str(self.calls)
            env["FAKE_RESPONSES"] = json.dumps([invalid])
            proc = subprocess.run(["python3", "-I", str(SCRIPT), "--wiki", str(self.wiki), "question"],
                                  text=True, capture_output=True, env=env)
            self.assertNotEqual(proc.returncode, 0)
            self.assertIn("invalid Jev response", proc.stderr)
            self.calls.unlink(missing_ok=True)

    def test_pages_need_no_added_frontmatter_fields(self):
        self.add_page("plain", "Plain existing title", "Existing content")
        proc = self.run_search([[(0.1, 0.6)]])
        self.assertEqual(len(proc.stdout.strip().splitlines()), 1)
        self.assertTrue(proc.stdout.strip().endswith("concepts/plain.md\tPlain existing title"))


if __name__ == "__main__":
    unittest.main()
