//! Stable identifiers for Foundation hooks.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookId {
    pub id: &'static str,
    pub subcommand: Option<&'static str>,
    pub aliases: &'static [&'static str],
    pub ecc_ids: &'static [&'static str],
}

pub const HOOK_IDS: &[HookId] = &[
    HookId {
        id: "pre:config-protection",
        subcommand: Some("config-protection"),
        aliases: &[],
        ecc_ids: &["pre:config-protection"],
    },
    HookId {
        id: "pre:bash:preflight",
        subcommand: Some("preflight"),
        aliases: &[],
        ecc_ids: &["pre:bash:dispatcher"],
    },
    HookId {
        id: "pre:hookify-runtime",
        subcommand: Some("hookify"),
        aliases: &[],
        ecc_ids: &["pre:hookify-runtime"],
    },
    HookId {
        id: "post:edit:observer",
        subcommand: Some("post-observer"),
        aliases: &[],
        ecc_ids: &["post:dispatcher:sync"],
    },
    HookId {
        id: "post:skill:track",
        subcommand: Some("lifecycle"),
        aliases: &[],
        ecc_ids: &["post:skill:track"],
    },
    HookId {
        id: "session:start",
        subcommand: Some("session-hooks"),
        aliases: &[],
        ecc_ids: &["session:start"],
    },
    HookId {
        id: "pre:governance-capture",
        subcommand: Some("governance-capture"),
        aliases: &[],
        ecc_ids: &["pre:governance-capture"],
    },
    HookId {
        id: "pre:observe:continuous-learning",
        subcommand: Some("observe"),
        aliases: &[],
        ecc_ids: &["pre:observe:continuous-learning"],
    },
    HookId {
        id: "prompt:submit",
        subcommand: Some("prompt-submit"),
        aliases: &[],
        ecc_ids: &[],
    },
    HookId {
        id: "pre:write:doc-file-warning",
        subcommand: Some("doc-warning"),
        aliases: &["doc-file-warning"],
        ecc_ids: &["pre:write:doc-file-warning"],
    },
    HookId {
        id: "pre:edit-write:suggest-compact",
        subcommand: Some("suggest-compact"),
        aliases: &[],
        ecc_ids: &["pre:edit-write:suggest-compact"],
    },
    HookId {
        id: "stop:hookify-runtime",
        subcommand: Some("stop-hookify"),
        aliases: &[],
        ecc_ids: &["stop:hookify-runtime"],
    },
    HookId {
        id: "stop:check-console-log",
        subcommand: Some("check-console-log"),
        aliases: &[],
        ecc_ids: &["stop:check-console-log"],
    },
    HookId {
        id: "stop:format-typecheck",
        subcommand: Some("format-typecheck"),
        aliases: &[],
        ecc_ids: &["stop:format-typecheck"],
    },
    HookId {
        id: "stop:session-end",
        subcommand: Some("session-end"),
        aliases: &[],
        ecc_ids: &["stop:session-end"],
    },
    HookId {
        id: "stop:evaluate-session",
        subcommand: Some("evaluate-session"),
        aliases: &[],
        ecc_ids: &["stop:evaluate-session"],
    },
    HookId {
        id: "stop:cost-tracker",
        subcommand: Some("cost-tracker"),
        aliases: &[],
        ecc_ids: &["stop:cost-tracker"],
    },
    HookId {
        id: "stop:desktop-notify",
        subcommand: Some("desktop-notify"),
        aliases: &[],
        ecc_ids: &["stop:desktop-notify"],
    },
    HookId {
        id: "mcp-health",
        subcommand: Some("mcp-health"),
        aliases: &[],
        ecc_ids: &["pre:mcp-health-check", "post:mcp-health-check"],
    },
    HookId {
        id: "pre:edit-write:gateguard",
        subcommand: Some("gateguard"),
        aliases: &[],
        ecc_ids: &["pre:edit-write:gateguard-fact-force"],
    },
    HookId {
        id: "post:dispatcher:async",
        subcommand: Some("post-async"),
        aliases: &[],
        ecc_ids: &["post:dispatcher:async"],
    },
    HookId {
        id: "pre:compact",
        subcommand: Some("compact-summary"),
        aliases: &[],
        ecc_ids: &["pre:compact"],
    },
    HookId {
        id: "session:end:marker",
        subcommand: None,
        aliases: &[],
        ecc_ids: &["session:end:marker"],
    },
];

/// Find a hook by its stable Foundation ID.
#[allow(dead_code)] // Public lookup API; currently used by tests and downstream integrations.
pub fn find_by_id(id: &str) -> Option<&'static HookId> {
    HOOK_IDS.iter().find(|hook| hook.id == id)
}

/// Find a hook by its primary subcommand or one of its aliases.
pub fn find_by_subcommand(subcommand: &str) -> Option<&'static HookId> {
    HOOK_IDS
        .iter()
        .find(|hook| hook.subcommand == Some(subcommand) || hook.aliases.contains(&subcommand))
}

#[cfg(test)]
mod tests {
    use super::{find_by_id, find_by_subcommand, HOOK_IDS};
    use std::collections::HashSet;

    const MAIN_RS: &str = include_str!("main.rs");
    const INVENTORY: &str = include_str!("../adapters/hermes/hooks/INVENTORY.md");
    const NON_HOOK_COMMANDS: &[&str] = &["enable", "decide"];

    #[test]
    fn ids_and_command_names_are_unique() {
        let mut ids = HashSet::new();
        let mut commands = HashSet::new();
        for hook in HOOK_IDS {
            assert!(ids.insert(hook.id), "duplicate hook ID: {}", hook.id);
            for command in hook
                .subcommand
                .into_iter()
                .chain(hook.aliases.iter().copied())
            {
                assert!(
                    commands.insert(command),
                    "duplicate hook command: {command}"
                );
            }
        }
        for command in NON_HOOK_COMMANDS {
            assert!(
                !commands.contains(command),
                "non-hook command included: {command}"
            );
        }
    }

    #[test]
    fn lookup_functions_find_ids_and_aliases() {
        for hook in HOOK_IDS {
            assert_eq!(find_by_id(hook.id), Some(hook));
            for command in hook
                .subcommand
                .into_iter()
                .chain(hook.aliases.iter().copied())
            {
                assert_eq!(find_by_subcommand(command), Some(hook));
            }
        }
        assert!(find_by_id("missing").is_none());
        assert!(find_by_subcommand("missing").is_none());
    }

    #[test]
    fn command_match_arms_equal_hook_commands() {
        let match_body = MAIN_RS
            .split("match command.as_str() {")
            .nth(1)
            .expect("main.rs command match exists")
            .split("_ => unreachable!()")
            .next()
            .expect("command match has unreachable fallback");
        let actual: HashSet<&str> = match_body
            .split("=>")
            .flat_map(|arm| arm.split('"').skip(1).step_by(2))
            .collect();
        let expected: HashSet<&str> = HOOK_IDS
            .iter()
            .flat_map(|hook| {
                hook.subcommand
                    .into_iter()
                    .chain(hook.aliases.iter().copied())
            })
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn inventory_foundation_and_ecc_ids_are_consistent() {
        let primary_table = INVENTORY
            .split("## Wave 2 issues")
            .next()
            .expect("inventory primary table exists");
        let rows: Vec<Vec<&str>> = primary_table
            .lines()
            .filter(|line| line.starts_with('|'))
            .map(|line| line.trim_matches('|').split('|').map(str::trim).collect())
            .collect();
        let header = rows.first().expect("table header exists");
        let ecc_column = header
            .iter()
            .position(|cell| cell.contains("ECC metadata ID"))
            .unwrap();
        let foundation_column = header
            .iter()
            .position(|cell| *cell == "Foundation ID")
            .unwrap();
        let ecc_ids: HashSet<&str> = rows
            .iter()
            .skip(2)
            .flat_map(|row| row[ecc_column].split('`').skip(1).step_by(2))
            .collect();

        for row in rows.iter().skip(2) {
            let foundation_id = row[foundation_column]
                .split('`')
                .nth(1)
                .unwrap_or(row[foundation_column]);
            if foundation_id == "—" {
                continue;
            }
            assert!(
                find_by_id(foundation_id).is_some(),
                "unknown Foundation ID: {foundation_id}"
            );
        }
        for hook in HOOK_IDS.iter().filter(|hook| !hook.ecc_ids.is_empty()) {
            for ecc_id in hook.ecc_ids {
                assert!(
                    ecc_ids.contains(ecc_id),
                    "ECC ID missing from inventory: {ecc_id}"
                );
            }
        }
    }
}
