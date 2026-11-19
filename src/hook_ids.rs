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
        ecc_ids: &["prompt:hookify-runtime"],
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
    const NON_HOOK_COMMANDS: &[&str] = &["decide", "version"];

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
        let table_lines: Vec<&str> = primary_table
            .lines()
            .filter(|line| line.starts_with('|'))
            .collect();
        let header: Vec<&str> = table_lines[0][1..table_lines[0].len() - 1]
            .split('|')
            .map(str::trim)
            .collect();
        let ecc_column = header
            .iter()
            .position(|cell| cell.contains("ECC metadata ID"))
            .expect("ECC metadata ID column exists");
        let foundation_column = header
            .iter()
            .position(|cell| *cell == "Foundation ID")
            .expect("Foundation ID column exists");
        let mut inventory_mapping = Vec::new();

        for (line_number, line) in primary_table.lines().enumerate() {
            if !line.starts_with('|') {
                continue;
            }
            assert!(
                !line.starts_with("||") && line.ends_with('|'),
                "malformed primary-table row at line {}: {line}",
                line_number + 1
            );
            let cells: Vec<&str> = line[1..line.len() - 1].split('|').map(str::trim).collect();
            assert_eq!(
                cells.len(),
                header.len(),
                "primary-table row has shifted columns at line {}: {line}",
                line_number + 1
            );
            if cells
                .iter()
                .all(|cell| cell.chars().all(|ch| ch == '-' || ch == ':'))
            {
                continue;
            }
            if cells[0].contains("ECC metadata ID") {
                continue;
            }

            let foundation_id = cells[foundation_column]
                .strip_prefix('`')
                .and_then(|value| value.strip_suffix('`'))
                .unwrap_or(cells[foundation_column]);
            let ecc_ids: Vec<&str> = cells[ecc_column].split('`').skip(1).step_by(2).collect();
            if foundation_id == "—" {
                assert!(
                    ecc_ids.iter().all(|ecc_id| {
                        HOOK_IDS.iter().all(|hook| !hook.ecc_ids.contains(ecc_id))
                    }),
                    "unmapped ECC registration unexpectedly has a Foundation ID: {line}"
                );
            } else {
                assert!(
                    find_by_id(foundation_id).is_some(),
                    "unknown Foundation ID: {foundation_id}"
                );
                inventory_mapping.extend(ecc_ids.into_iter().map(|ecc_id| (foundation_id, ecc_id)));
            }
        }

        let mut expected_mapping: Vec<(&str, &str)> = HOOK_IDS
            .iter()
            .filter(|hook| !hook.ecc_ids.is_empty())
            .flat_map(|hook| hook.ecc_ids.iter().map(|ecc_id| (hook.id, *ecc_id)))
            .collect();
        inventory_mapping.sort();
        expected_mapping.sort();
        assert_eq!(
            inventory_mapping, expected_mapping,
            "inventory ECC-to-Foundation mappings must exactly match HOOK_IDS.ecc_ids"
        );
    }
}
