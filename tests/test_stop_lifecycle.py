"""Safe, side-effect-free portions of ECC stop-hook adapters."""

import unittest

from adapters.hermes.hooks.stop_lifecycle import (
    changed_javascript_paths,
    event_for_ecc_stop,
    session_usage_totals,
    should_persist,
)


class StopLifecycleTests(unittest.TestCase):
    def test_maps_turn_and_session_boundaries_without_equating_them(self):
        self.assertEqual(event_for_ecc_stop(23), "on_session_end")
        self.assertEqual(event_for_ecc_stop(27), "on_session_finalize")
        self.assertIsNone(event_for_ecc_stop(20))

    def test_session_persistence_requires_explicit_opt_in(self):
        self.assertFalse(should_persist("on_session_end", {}))
        self.assertFalse(should_persist("on_session_finalize", {"enabled": True}))
        self.assertTrue(should_persist("on_session_end", {"enabled": True, "scope": "turn"}))
        self.assertTrue(should_persist("on_session_finalize", {"enabled": True, "scope": "session"}))
        self.assertFalse(should_persist("on_session_end", {"enabled": True, "scope": "session"}))

    def test_changed_javascript_paths_are_confined_and_deleted_paths_excluded(self):
        got = changed_javascript_paths(
            ["src/a.ts", "src/b.js", "src/removed.ts", "../outside.ts", "src/c.tsx"],
            repo_root="/repo",
            deleted={"src/removed.ts"},
        )
        self.assertEqual(got, ("src/a.ts", "src/b.js", "src/c.tsx"))

    def test_usage_aggregation_uses_only_explicit_nonnegative_integer_fields(self):
        got = session_usage_totals([
            {"input_tokens": 10, "output_tokens": 4, "cache_read_input_tokens": 2},
            {"input_tokens": 3, "output_tokens": 1, "cache_read_input_tokens": 1},
            {"input_tokens": -7, "output_tokens": "bad"},
        ])
        self.assertEqual(got, {
            "input_tokens": 13,
            "output_tokens": 5,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 3,
        })


if __name__ == "__main__":
    unittest.main()
