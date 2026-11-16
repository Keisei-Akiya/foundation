import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("wiki_search.py")
sys.path.insert(0, str(SCRIPT.parent))
import wiki_search
FAKE_FOUNDATION = r'''#!/usr/bin/env python3
import json, os, sys
spec = json.load(sys.stdin)
with open(os.environ["FAKE_CALLS"], "a", encoding="utf-8") as f:
    f.write(json.dumps(spec, ensure_ascii=False) + "\n")
responses = json.loads(os.environ["FAKE_RESPONSES"])
call = sum(1 for _ in open(os.environ["FAKE_CALLS"], encoding="utf-8")) - 1
configured = responses[call]
if isinstance(configured, dict):
    print(json.dumps(configured))
else:
    answers = {}
    for question, score in zip(spec["questions"], configured):
        labels = [level["label"].lower() for level in question["levels"]]
        if labels == ["no", "yes"]:
            answers[question["name"]] = {
                "confidence": 0.68,
                "legend": {"0": "no", "1": "yes"},
                "probabilities": {"0": 1 - score, "1": score},
                "score": 1.0,
                "type": "score",
            }
        else:
            low, high = score
            answers[question["name"]] = {
                "confidence": 0.68,
                "legend": {"0": "none", "1": "low", "2": "high"},
                "probabilities": {"0": 1 - low - high, "1": low, "2": high},
                "score": 1.79,
                "type": "score",
            }
    print(json.dumps({"answers": answers}))
'''


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
        env["FAKE_RESPONSES"] = json.dumps(scores)
        proc = subprocess.run(
            ["python3", "-I", str(SCRIPT), "--wiki", str(self.wiki),
             "--log", str(self.root / "search.jsonl"), *args,
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
        self.assertEqual(self.read_calls()[0]["questions"][0]["name"], "page:concepts/alpha.md")

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

    def assert_invalid_response(self, response):
        self.add_page("alpha", "Alpha", "body")
        env = os.environ.copy()
        env["PATH"] = str(self.bin) + os.pathsep + env["PATH"]
        env["FAKE_CALLS"] = str(self.calls)
        env["FAKE_RESPONSES"] = json.dumps([response])
        proc = subprocess.run(["python3", "-I", str(SCRIPT), "--wiki", str(self.wiki), "question"],
                              text=True, capture_output=True, env=env)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("invalid Jev response", proc.stderr)
        self.calls.unlink(missing_ok=True)

    def test_response_without_answers_fails_clearly(self):
        self.assert_invalid_response({})

    def test_empty_probabilities_fail_clearly(self):
        answer = {"probabilities": {}, "legend": {"0": "none", "1": "low", "2": "high"}}
        self.assert_invalid_response({"answers": {"page:concepts/alpha.md": answer}})

    def test_non_integer_probability_keys_fail_clearly(self):
        answer = {"probabilities": {"low": 0.4}, "legend": {"0": "none", "1": "low", "2": "high"}}
        self.assert_invalid_response({"answers": {"page:concepts/alpha.md": answer}})

    def test_out_of_range_probabilities_fail_clearly(self):
        answer = {"probabilities": {"0": 0.1, "1": 1.2, "2": 0.2},
                  "legend": {"0": "none", "1": "low", "2": "high"}}
        self.assert_invalid_response({"answers": {"page:concepts/alpha.md": answer}})

    def test_legend_without_low_fails_clearly(self):
        answer = {"probabilities": {"0": 0.1, "1": 0.4, "2": 0.5},
                  "legend": {"0": "none", "2": "high"}}
        self.assert_invalid_response({"answers": {"page:concepts/alpha.md": answer}})

    def test_legend_without_high_fails_clearly(self):
        answer = {"probabilities": {"0": 0.1, "1": 0.4, "2": 0.5},
                  "legend": {"0": "none", "1": "low"}}
        self.assert_invalid_response({"answers": {"page:concepts/alpha.md": answer}})

    def test_missing_question_answer_fails_clearly(self):
        answer = {"probabilities": {"0": 0.1, "1": 0.4, "2": 0.5},
                  "legend": {"0": "none", "1": "low", "2": "high"}}
        self.assert_invalid_response({"answers": {}})

    def test_pages_need_no_added_frontmatter_fields(self):
        self.add_page("plain", "Plain existing title", "Existing content")
        proc = self.run_search([[(0.1, 0.6)]])
        self.assertEqual(len(proc.stdout.strip().splitlines()), 1)
        self.assertTrue(proc.stdout.strip().endswith("concepts/plain.md\tPlain existing title"))

    def add_schema(self, rows):
        table = "| Domain | Description |\n| --- | --- |\n" + "".join(
            f"| {name} | {description} |\n" for name, description in rows)
        (self.wiki / "SCHEMA.md").write_text("# Schema\n\n## Domains\n\n" + table,
                                             encoding="utf-8")

    def test_frontmatter_domain_lists_parse_yaml_block_items_without_following_fields(self):
        fixtures = {
            "single": ("domains:\n- engineering", ["engineering"]),
            "multiple": ("domains:\n  - 'engineering'\n  - \"home\"\ntags:\n  - management",
                         ["engineering", "home"]),
            "inline": ("domains: [engineering, 'home']", ["engineering", "home"]),
        }
        for slug, (fields, expected) in fixtures.items():
            with self.subTest(slug=slug):
                page = self.wiki / "concepts" / f"{slug}.md"
                page.write_text(f"---\ntitle: {slug}\n{fields}\n---\n", encoding="utf-8")
                self.assertEqual(wiki_search._frontmatter_domains(page), expected)

    def test_block_list_domains_select_matching_pages_and_schema_descriptions(self):
        self.add_schema([("engineering", "Engineering domain description"),
                         ("home", "Home domain description")])
        (self.wiki / "concepts" / "engineering.md").write_text(
            "---\ntitle: Engineering\ndomains:\n- engineering\ntags:\n- home\n---\nbody\n",
            encoding="utf-8")
        (self.wiki / "concepts" / "home.md").write_text(
            "---\ntitle: Home\ndomains:\n  - \"home\"\n---\nbody\n", encoding="utf-8")
        self.run_search([[0.9, 0.1], [(0.8, 0.1), (0.1, 0.8)]])
        calls = self.read_calls()
        self.assertEqual([q["name"] for q in calls[0]["questions"]],
                         ["domain:engineering", "domain:home"])
        self.assertIn("Engineering domain description", json.dumps(calls[0], ensure_ascii=False))
        self.assertEqual([q["name"] for q in calls[1]["questions"]],
                         ["page:concepts/engineering.md"])

    def test_domains_score_candidates_only_and_only_send_allowed_metadata(self):
        self.add_schema([
            ("engineering", "Engineering description"),
            ("home", "Home description"),
            ("management", "Management description"),
        ])
        self.add_page("a_eng", "Engineering page", "PRIVATE ENG BODY", "[engineering]")
        self.add_page("b_home", "Home page", "PRIVATE HOME BODY", "[home]")
        self.add_page("c_mgmt", "Management page", "PRIVATE MGMT BODY", "[management]")
        self.add_page("d_plain", "Unclassified page", "PRIVATE PLAIN BODY", "[]")
        plain_path = self.wiki / "concepts" / "d_plain.md"
        plain_path.write_text(plain_path.read_text(encoding="utf-8").replace("domains: []\n", ""),
                              encoding="utf-8")
        (self.wiki / "index.md").write_text(
            "# Index\n- [[a_eng]] — Engineering summary\n"
            "- [[b_home]] — Home summary\n- [[c_mgmt]] — Management summary\n"
            "- [[d_plain]] — Plain summary\n", encoding="utf-8")
        proc = self.run_search([[0.8, 0.6, 0.1], [(0.1, 0.5), (0.2, 0.4),
                                                 (0.4, 0.6)]],
                               "--domain-threshold", "0.5", "--max-domains", "2",
                               "--threshold", "0.6", "--max-results", "1")
        calls = self.read_calls()
        self.assertEqual(len(calls), 2)
        domain_questions = calls[0]["questions"]
        self.assertEqual([q["name"] for q in domain_questions],
                         ["domain:engineering", "domain:home", "domain:management"])
        self.assertTrue(all([level["label"].lower() for level in q["levels"]] == ["no", "yes"]
                            for q in domain_questions))
        self.assertEqual([q["name"] for q in calls[1]["questions"]],
                         ["page:concepts/a_eng.md", "page:concepts/b_home.md",
                          "page:concepts/d_plain.md"])
        self.assertEqual(len(proc.stdout.strip().splitlines()), 1)
        serialized = json.dumps(calls, ensure_ascii=False)
        self.assertIn("Engineering description", serialized)
        self.assertIn("Home description", serialized)
        self.assertIn("Engineering summary", serialized)
        self.assertNotIn("Management summary", serialized)
        self.assertNotIn("PRIVATE", serialized)
        self.assertNotIn("none", json.dumps(domain_questions, ensure_ascii=False).lower())

    def test_schema_domain_names_strip_markdown_emphasis(self):
        self.add_schema([
            ("**engineering**", "**Engineering** description"),
            ("`management`", "`Management` description"),
        ])
        self.add_page("a_eng", "Engineering page", "body", "[engineering]")
        self.add_page("b_mgmt", "Management page", "body", "[management]")
        self.run_search([[0.8, 0.7], [(0.2, 0.5), (0.1, 0.4)]])
        questions = self.read_calls()[0]["questions"]
        self.assertEqual([question["name"] for question in questions],
                         ["domain:engineering", "domain:management"])
        serialized = json.dumps(questions, ensure_ascii=False)
        self.assertIn("説明: Engineering description", serialized)
        self.assertIn("説明: Management description", serialized)

    def test_domain_fallback_and_runtime_domain_limits_are_independent(self):
        self.add_schema([("engineering", "Engineering"), ("home", "Home"),
                         ("management", "Management")])
        self.add_page("a_eng", "Engineering", "body", "[engineering]")
        self.add_page("b_home", "Home", "body", "[home]")
        self.add_page("c_plain", "Plain", "body", "")
        proc = self.run_search([[0.2, 0.7, 0.9], [(0.1, 0.8), (0.4, 0.3)]],
                               "--domain-threshold", "0.95", "--max-domains", "1",
                               "--threshold", "0.8", "--max-results", "5")
        calls = self.read_calls()
        self.assertEqual([q["name"] for q in calls[1]["questions"]],
                         ["page:concepts/b_home.md", "page:concepts/c_plain.md"])
        self.assertEqual(len(proc.stdout.strip().splitlines()), 1)
        self.assertTrue(proc.stdout.strip().endswith("concepts/b_home.md\tHome"))

    def test_jsonl_logs_all_scores_probabilities_usage_and_returned_pages_once(self):
        self.add_schema([("engineering", "Engineering"), ("home", "Home")])
        self.add_page("a_eng", "Engineering", "body", "[engineering]")
        self.add_page("b_home", "Home", "body", "[home]")
        log = self.root / "custom.jsonl"
        env = os.environ.copy()
        env["PATH"] = str(self.bin) + os.pathsep + env["PATH"]
        env["FAKE_CALLS"] = str(self.calls)
        env["FAKE_RESPONSES"] = json.dumps([
            {"answers": {
                "domain:engineering": {"legend": {"0":"no","1":"yes"}, "probabilities":{"0":0.2,"1":0.8}},
                "domain:home": {"legend": {"0":"no","1":"yes"}, "probabilities":{"0":0.7,"1":0.3}}},
             "usage": {"input_tokens": 12, "output_tokens": 3}},
            {"answers": {
                "page:concepts/a_eng.md": {"legend":{"0":"none","1":"low","2":"high"}, "probabilities":{"0":0.1,"1":0.2,"2":0.7}}},
             "usage": {"input_tokens": 20, "output_tokens": 4}}
        ])
        proc = subprocess.run(["python3", "-I", str(SCRIPT), "--wiki", str(self.wiki),
                              "--log", str(log), "--domain-threshold", "0.5", "--max-domains", "1",
                              "--threshold", "0.5", "question text"],
                             text=True, capture_output=True, env=env)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        lines = log.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), 1)
        record = json.loads(lines[0])
        self.assertEqual(record["question"], "question text")
        self.assertEqual(set(record["domain_probabilities"]), {"engineering", "home"})
        self.assertEqual(record["domain_probabilities"]["home"], 0.3)
        self.assertEqual(set(record["page_scores"]), {"concepts/a_eng.md", "concepts/b_home.md"})
        self.assertAlmostEqual(record["page_scores"]["concepts/a_eng.md"], 0.9)
        self.assertIsNone(record["page_scores"]["concepts/b_home.md"])
        calls = self.read_calls()
        self.assertEqual([q["name"] for q in calls[1]["questions"]],
                         ["page:concepts/a_eng.md"])
        self.assertEqual(record["returned_pages"], ["concepts/a_eng.md"])
        self.assertEqual(record["jev_usage"], [{"input_tokens": 12, "output_tokens": 3},
                                                {"input_tokens": 20, "output_tokens": 4}])
        self.assertIn("timestamp", record)
        self.assertEqual(len(proc.stdout.strip().splitlines()), 1)

    def test_jsonl_append_failure_does_not_prevent_results(self):
        self.add_page("alpha", "Alpha", "body")
        blocked = self.root / "blocked"
        blocked.write_text("not a directory", encoding="utf-8")
        proc = self.run_search([[(0.1, 0.7)]], "--log", str(blocked / "search.jsonl"))
        self.assertEqual(len(proc.stdout.strip().splitlines()), 1)
        self.assertIn("log", proc.stderr.lower())


if __name__ == "__main__":
    unittest.main()
