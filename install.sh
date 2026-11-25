#!/bin/sh
# foundation installer.
#
#   curl -fsSL https://raw.githubusercontent.com/Keisei-Akiya/foundation/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/Keisei-Akiya/foundation/main/install.sh | sh -s -- --version 0.0.29
#
# Downloads the prebuilt release from GitHub Releases, verifies its SHA256, then
# installs into <HERMES_HOME>. Hermes CLI enables the native plugin and registers
# the installed skills directory; hook allowlists are untouched and Hermes is
# never restarted. The native plugin is always enabled.
set -eu

REPO="Keisei-Akiya/foundation"
TARGET="x86_64-unknown-linux-musl"
ASSET="foundation-$TARGET.tar.gz"

version="latest"
hermes_home="${HERMES_HOME:-${HOME:?HOME is not set}/.hermes}"

usage() {
  cat <<EOF
Usage: install.sh [--version X.Y.Z] [--hermes-home DIR]

Installs foundation (foundation binary, adapters, skills, docs).

  --version X.Y.Z    Release to install (default: latest).
  --hermes-home DIR  Target Hermes home (default: \$HERMES_HOME or ~/.hermes).

Installed paths:
  <HERMES_HOME>/bin/foundation
  <HERMES_HOME>/foundation/   (adapters, skills, docs, VERSION)

Environment:
  FOUNDATION_BASE_URL  Override the download base URL (testing; file:// allowed).
EOF
}

die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      [ $# -ge 2 ] || die "missing value for --version"
      version="${2#v}"; shift ;;
    --hermes-home)
      [ $# -ge 2 ] || die "missing value for --hermes-home"
      hermes_home="$2"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; die "unknown argument: $1" ;;
  esac
  shift
done

case "$version" in
  latest) ;;
  *[!0-9A-Za-z.+-]*|"") die "invalid version: $version" ;;
esac

[ "$(uname -s)" = "Linux" ] || die "unsupported OS: $(uname -s) (Linux x86_64 only)"
case "$(uname -m)" in
  x86_64|amd64) ;;
  *) die "unsupported architecture: $(uname -m) (Linux x86_64 only)" ;;
esac

for cmd in curl tar sha256sum install mktemp python3; do
  command -v "$cmd" >/dev/null 2>&1 || die "required command not found: $cmd"
done

if [ -n "${FOUNDATION_BASE_URL:-}" ]; then
  base_url="$FOUNDATION_BASE_URL"
elif [ "$version" = "latest" ]; then
  base_url="https://github.com/$REPO/releases/latest/download"
else
  base_url="https://github.com/$REPO/releases/download/v$version"
fi

tmp="$(mktemp -d)"
trap 'rm -rf -- "$tmp"' EXIT INT TERM

printf 'Downloading %s (%s)\n' "$ASSET" "$version"
curl -fsSL --proto '=https,file' --tlsv1.2 -o "$tmp/$ASSET" "$base_url/$ASSET" \
  || die "download failed: $base_url/$ASSET"
curl -fsSL --proto '=https,file' --tlsv1.2 -o "$tmp/$ASSET.sha256" "$base_url/$ASSET.sha256" \
  || die "download failed: $base_url/$ASSET.sha256"

# The checksum file names the asset; verify from inside the download directory.
(cd "$tmp" && sha256sum -c "$ASSET.sha256" >/dev/null 2>&1) \
  || die "SHA256 verification failed for $ASSET"
printf 'SHA256 verified.\n'

mkdir -p "$tmp/x"
tar -xzf "$tmp/$ASSET" -C "$tmp/x"
src="$tmp/x/foundation"
[ -x "$src/bin/foundation" ] || die "archive is missing bin/foundation"
[ -f "$src/VERSION" ] || die "archive is missing VERSION"
installed_version="$(cat "$src/VERSION")"

mkdir -p "$hermes_home"
hermes_home="$(cd "$hermes_home" && pwd -P)"
bin_dir="$hermes_home/bin"
share_dir="$hermes_home/foundation"
mkdir -p "$bin_dir"

install -m 0755 "$src/bin/foundation" "$bin_dir/foundation.new"
mv -f "$bin_dir/foundation.new" "$bin_dir/foundation"
rm -rf "$src/bin"

# Replace the shared tree in two renames so a failure leaves the old copy intact.
rm -rf "$share_dir.new"
mv "$src" "$share_dir.new"
rm -rf "$share_dir.old"
[ ! -e "$share_dir" ] || mv "$share_dir" "$share_dir.old"
mv "$share_dir.new" "$share_dir"
rm -rf "$share_dir.old"

# Place the native plugin where Hermes discovers it. A foreign
# plugins/foundation is left untouched; Hermes CLI activation follows below.
plugin_link="$hermes_home/plugins/foundation"
plugin_target="../foundation/adapters/hermes/plugin"
plugin_note="plugin : $plugin_link"
plugin_conflict=0
if [ -L "$plugin_link" ] || [ ! -e "$plugin_link" ]; then
  mkdir -p "$hermes_home/plugins"
  ln -sfn "$plugin_target" "$plugin_link"
else
  plugin_note="plugin : skipped ($plugin_link already exists and is not a symlink)"
  plugin_conflict=1
fi

cat <<EOF

Installed foundation $installed_version
  binary: $bin_dir/foundation
  files : $share_dir
  $plugin_note

EOF

[ "$plugin_conflict" -eq 0 ] || die "installation completed but $plugin_link already exists and is not a symlink; refusing to enable the native plugin"
command -v hermes >/dev/null 2>&1 || die "hermes CLI not found; installation completed but plugin could not be enabled"
skills_dir="$share_dir/skills"
if ! external_dirs="$(HERMES_HOME="$hermes_home" hermes config get skills.external_dirs --json 2>"$tmp/config-get.stderr")"; then
  [ ! -s "$tmp/config-get.stderr" ] || {
    printf 'install.sh: Hermes config get skills.external_dirs failed:\n' >&2
    cat "$tmp/config-get.stderr" >&2
  }
  die "installation completed but Hermes skills configuration could not be read"
fi
merged_external_dirs="$(printf '%s' "$external_dirs" | python3 -I -c 'import json, sys
try:
    entries = json.load(sys.stdin)
except json.JSONDecodeError as error:
    print(f"invalid JSON at line {error.lineno}, column {error.colno}", file=sys.stderr)
    raise SystemExit(1)
if not isinstance(entries, list) or not all(isinstance(entry, str) for entry in entries):
    print("expected skills.external_dirs to be a JSON list of strings", file=sys.stderr)
    raise SystemExit(1)
skills_dir = sys.argv[1]
if skills_dir not in entries:
    entries.append(skills_dir)
print(json.dumps(entries, ensure_ascii=False))' "$skills_dir")" || die "installation completed but Hermes skills configuration is invalid; see the parsing diagnostic above"
if ! HERMES_HOME="$hermes_home" hermes config set skills.external_dirs "$merged_external_dirs" >/dev/null 2>"$tmp/config-set.stderr"; then
  [ ! -s "$tmp/config-set.stderr" ] || {
    printf 'install.sh: Hermes config set skills.external_dirs failed:\n' >&2
    cat "$tmp/config-set.stderr" >&2
  }
  die "installation completed but the foundation skills directory could not be registered with Hermes"
fi
HERMES_HOME="$hermes_home" hermes plugins enable foundation \
  || die "enabling the native plugin failed (the install itself is complete)"

cat <<EOF
The native plugin is enabled and the foundation skills directory is registered with Hermes. The hook allowlist and running gateway were not changed. Restart the gateway to apply the plugin and skills changes.
To disable the plugin, run:
  HERMES_HOME=$hermes_home hermes plugins disable foundation
EOF
