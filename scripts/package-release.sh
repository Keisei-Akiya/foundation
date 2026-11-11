#!/usr/bin/env bash
set -euo pipefail

# Package a release tarball from an already built binary.
# Usage: package-release.sh <target-triple> <output-dir>
# Expects target/<target-triple>/release/foundation (or target/release/ when
# FOUNDATION_BIN is set to an explicit path).

usage() {
  echo "Usage: $0 <target-triple> <output-dir>" >&2
  exit 2
}

(($# == 2)) || usage
target=$1
out_dir=$2

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
bin="${FOUNDATION_BIN:-$repo_root/target/$target/release/foundation}"
[[ -x "$bin" ]] || { echo "binary not found: $bin" >&2; exit 1; }

version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$repo_root/Cargo.toml" | head -n1)"
[[ -n "$version" ]] || { echo "cannot read version from Cargo.toml" >&2; exit 1; }

stage="$(mktemp -d)"
trap 'rm -rf -- "$stage"' EXIT
root="$stage/foundation"
mkdir -p "$root/bin"

install -m 0755 -- "$bin" "$root/bin/foundation"
printf '%s\n' "$version" > "$root/VERSION"
for path in adapters skills docs README.md AGENTS.md; do
  cp -R -- "$repo_root/$path" "$root/$path"
done
# Keep placeholders out of the distributed tree.
find "$root" -name .gitkeep -type f -delete

mkdir -p -- "$out_dir"
asset="foundation-$target.tar.gz"
# Fixed ordering and ownership make the archive reproducible for the same inputs.
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime='2000-01-01 00:00:00Z' \
  -C "$stage" -czf "$out_dir/$asset" foundation
(cd -- "$out_dir" && sha256sum "$asset" > "$asset.sha256")

printf 'version: %s\nasset: %s/%s\n' "$version" "$out_dir" "$asset"
