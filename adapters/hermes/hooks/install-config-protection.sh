#!/usr/bin/env bash
set -euo pipefail

# Build the Rust hook and optionally install the binary under the selected Hermes home.
# This script never edits config.yaml, allowlists, or restarts Hermes.
usage() {
  cat <<'EOF'
Usage: install-config-protection.sh [--install] [--hermes-home DIR]

Builds target/release/ecc-hermes-hooks from this checkout and prints a Hermes
hooks: snippet. With --install, copies the binary to <HERMES_HOME>/bin/ but
still does not edit Hermes configuration or approve/run the hook.

HERMES_HOME is used when set; otherwise ~/.hermes is selected. Pass an explicit
home to test an isolated profile. Requires Rust/Cargo and network access only if
Cargo needs to fetch uncached crates.
EOF
}

install_binary=0
hermes_home="${HERMES_HOME:-${HOME:?HOME is not set}/.hermes}"
while (($#)); do
  case "$1" in
    --install) install_binary=1 ;;
    --hermes-home)
      shift
      (($#)) || { echo "missing value for --hermes-home" >&2; exit 2; }
      hermes_home=$1
      ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/../../.." && pwd -P)"
manifest="$repo_root/Cargo.toml"
[[ -f "$manifest" ]] || { echo "Cargo.toml not found at $manifest" >&2; exit 1; }

cargo build --release --manifest-path "$manifest" --bin ecc-hermes-hooks
built="$repo_root/target/release/ecc-hermes-hooks"
[[ -x "$built" ]] || { echo "build succeeded but executable is missing: $built" >&2; exit 1; }

if ((install_binary)); then
  destination="$hermes_home/bin/ecc-hermes-hooks"
  mkdir -p -- "$(dirname -- "$destination")"
  install -m 0755 -- "$built" "$destination"
else
  destination="$built"
fi

printf 'Built binary: %s\n' "$built"
if ((install_binary)); then printf 'Installed binary: %s\n' "$destination"; fi
printf '\nPaste only this entry under hooks: in the intended Hermes config.yaml, after reviewing it:\n'
printf '  pre_tool_call:\n'
shell_quote() {
  local value=$1
  printf "'"
  printf '%s' "${value//\'/\'\\\'\'}"
  printf "'"
}
shell_command="$(shell_quote "$destination") config-protection"
yaml_command=${shell_command//\'/\'\'}
printf "    - command: '%s'\n" "$yaml_command"
printf '      matcher: "^(write_file|edit_file|patch|apply_patch)$"\n'
printf '      timeout: 10\n'
printf '      fail_closed: false\n'
printf '\nNo Hermes config, hook allowlist, or running gateway was changed.\n'
