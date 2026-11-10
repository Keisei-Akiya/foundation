from __future__ import annotations

import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))
from adapters.hermes.hooks import hookify_runtime as runtime  # noqa: E402


def rule(root: Path, slug: str, frontmatter: str, message: str = "policy") -> None:
    directory = root / ".claude"
    directory.mkdir(exist_ok=True)
    (directory / f"hookify.{slug}.local.md").write_text(
        f"---\n{frontmatter}\n---\n{message}\n", encoding="utf-8"
    )


class HookifyRuntimeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name) / "project"
        self.root.mkdir()

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def test_enabled_bash_block_uses_hermes_pre_tool_directive(self) -> None:
        rule(self.root, "publish", "name: publish\nenabled: true\nevent: bash\naction: block\npattern: npm\\s+publish", "approval required")
        self.assertEqual(runtime.evaluate("terminal", {"command": "npm PUBLISH"}, self.root), {
            "action": "block", "message": "Hookify rule publish: approval required"
        })

    def test_bash_alias_matches_sample_tool_name(self) -> None:
        rule(self.root, "bash", "event: bash\naction: block\npattern: npm\\s+publish")
        self.assertEqual(runtime.evaluate("bash", {"command": "npm publish"}, self.root)["action"], "block")

    def test_malformed_rule_fails_open_without_blocking(self) -> None:
        rule(self.root, "broken", "event: bash\\naction: block\\npattern: [")
        self.assertIsNone(runtime.evaluate("bash", {"command": "anything"}, self.root))

    def test_disabled_invalid_and_unsupported_events_fail_open(self) -> None:
        rule(self.root, "off", "enabled: false\nevent: bash\naction: block\npattern: npm")
        rule(self.root, "prompt", "event: prompt\naction: block\npattern: .*", "not a Hermes pre-tool event")
        self.assertIsNone(runtime.evaluate("terminal", {"command": "npm"}, self.root))

    def test_warn_only_rule_is_not_faked_as_an_argument_modification(self) -> None:
        rule(self.root, "warn", "event: bash\naction: warn\npattern: git\\s+push")
        self.assertIsNone(runtime.evaluate("terminal", {"command": "git push"}, self.root))

    def test_path_rules_inspect_file_path_and_content(self) -> None:
        rule(self.root, "secret", "event: file\naction: block\npattern: env$")
        result = runtime.evaluate("write_file", {"path": ".env", "content": "x"}, self.root)
        self.assertEqual(result and result["action"], "block")

    def test_rule_symlink_outside_project_is_ignored(self) -> None:
        outside = Path(self.temp_dir.name) / "outside-hookify.md"
        outside.write_text("---\nevent: bash\naction: block\npattern: .*\n---\n", encoding="utf-8")
        (self.root / ".claude").mkdir()
        (self.root / ".claude" / "hookify.escape.local.md").symlink_to(outside)
        rules, diagnostics = runtime.load_rules(self.root)
        self.assertEqual(rules, [])
        self.assertTrue(any("unsafe rule path" in item for item in diagnostics))

    def test_symlinked_claude_directory_is_rejected(self) -> None:
        outside = Path(self.temp_dir.name) / "outside"
        outside.mkdir()
        (outside / "hookify.escape.local.md").write_text(
            "---\nevent: bash\naction: block\npattern: .*\n---\npolicy\n", encoding="utf-8"
        )
        (self.root / ".claude").symlink_to(outside, target_is_directory=True)
        rules, diagnostics = runtime.load_rules(self.root)
        self.assertEqual(rules, [])
        self.assertTrue(any("unsafe rule directory" in item for item in diagnostics))

    def test_ambiguous_repeated_alternation_regex_is_rejected(self) -> None:
        rule(self.root, "hazard", "event: bash\naction: block\npattern: (a|aa)+$")
        rules, diagnostics = runtime.load_rules(self.root)
        self.assertEqual(rules, [])
        self.assertTrue(any("unsafe regex" in item for item in diagnostics))

    def test_backreferences_and_lookbehind_are_rejected(self) -> None:
        for slug, pattern in (("backref", r"(a)\1"), ("lookbehind", "(?<=a)b")):
            rule(self.root, slug, f"event: bash\naction: block\npattern: {pattern}")
        rules, diagnostics = runtime.load_rules(self.root)
        self.assertEqual(rules, [])
        self.assertEqual(sum("unsafe regex" in item for item in diagnostics), 2)

    def test_oversized_pattern_and_input_fail_open(self) -> None:
        rule(self.root, "large", "event: bash\naction: block\npattern: " + "a" * 300)
        self.assertIsNone(runtime.evaluate("terminal", {"command": "a"}, self.root))
        self.assertFalse(runtime._matches("a", "a" * (runtime.MAX_INPUT_CHARS + 1)))

    def test_regex_timeout_fails_open(self) -> None:
        started = time.monotonic()
        self.assertFalse(runtime._matches("a*" * 10 + "b", "a" * 65_535 + "!"))
        self.assertLess(time.monotonic() - started, 2.0)


if __name__ == "__main__":
    unittest.main()
