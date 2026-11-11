#!/usr/bin/env bash
# 配布物に影響する変更があるのに Cargo.toml の version が上がっていなければ失敗する。
# 使い方: scripts/check-version-bump.sh <base-ref>   例: origin/main
set -euo pipefail

base="${1:?usage: check-version-bump.sh <base-ref>}"

# version を上げるべきパス。.agents/skills/release/SKILL.md の基準と同じ内容を保つ。
dist_paths=(src adapters skills install.sh scripts/package-release.sh Cargo.lock)

read_version() { sed -n 's/^version = "\(.*\)"$/\1/p' | head -n1; }

base_version="$(git show "$base:Cargo.toml" | read_version)"
head_version="$(read_version < Cargo.toml)"

changed="$(git diff --name-only "$base...HEAD" -- "${dist_paths[@]}")"

if [ -z "$changed" ]; then
  echo "配布物に影響する変更なし。version 更新は不要 ($head_version)"
  exit 0
fi

if [ "$base_version" != "$head_version" ]; then
  echo "version 更新済み: $base_version -> $head_version"
  exit 0
fi

echo "配布物に影響する変更があるが version が $head_version のまま:" >&2
echo "$changed" | sed 's/^/  /' >&2
echo "Cargo.toml と Cargo.lock の version を上げる。基準は .agents/skills/release/SKILL.md を参照。" >&2
exit 1
