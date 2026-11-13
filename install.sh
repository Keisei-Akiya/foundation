#!/bin/sh
# foundation installer.
#
#   curl -fsSL https://raw.githubusercontent.com/Keisei-Akiya/foundation/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/Keisei-Akiya/foundation/main/install.sh | sh -s -- --version 0.0.12
#
# Downloads the prebuilt release from GitHub Releases, verifies its SHA256, then
# installs into <HERMES_HOME>. By default it never edits config.yaml or hook
# allowlists and never restarts Hermes. The native plugin is enabled by default;
# --enable-hooks remains opt-in.
set -eu

REPO="Keisei-Akiya/foundation"
TARGET="x86_64-unknown-linux-musl"
ASSET="foundation-$TARGET.tar.gz"

version="latest"
enable_hooks=0
enable_plugin=1
hermes_home="${HERMES_HOME:-${HOME:?HOME is not set}/.hermes}"

usage() {
  cat <<EOF
Usage: install.sh [--version X.Y.Z] [--hermes-home DIR] [--enable-plugin] [--enable-hooks]

Installs foundation (foundation binary, adapters, skills, docs).

  --version X.Y.Z    Release to install (default: latest).
  --enable-hooks     Also enable all hooks: back up config.yaml, add the hooks
                     (merged into an existing hooks section if any), and approve
                     those hooks in the allowlist (opt-in).
  --enable-plugin    Enable the native foundation plugin with Hermes CLI (default).
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
    --enable-hooks) enable_hooks=1 ;;
    --enable-plugin) enable_plugin=1 ;;
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

for cmd in curl tar sha256sum install mktemp; do
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

bin_dir="$hermes_home/bin"
share_dir="$hermes_home/foundation"
mkdir -p "$bin_dir" "$hermes_home"

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

# Place the native plugin where Hermes discovers it. This only adds a symlink;
# Hermes loads the plugin after `plugins.enabled` lists it, which install.sh
# never edits. A foreign plugins/foundation is left untouched.
plugin_link="$hermes_home/plugins/foundation"
plugin_target="../foundation/adapters/hermes/plugin"
plugin_note="plugin : $plugin_link"
if [ -L "$plugin_link" ] || [ ! -e "$plugin_link" ]; then
  mkdir -p "$hermes_home/plugins"
  ln -sfn "$plugin_target" "$plugin_link"
else
  plugin_note="plugin : skipped ($plugin_link already exists and is not a symlink)"
fi

cat <<EOF

Installed foundation $installed_version
  binary: $bin_dir/foundation
  files : $share_dir
  $plugin_note

EOF

if [ "$enable_plugin" = "1" ]; then
  command -v hermes >/dev/null 2>&1 || die "hermes CLI not found; install Hermes or enable the plugin with 'HERMES_HOME=$hermes_home hermes plugins enable foundation'"
  HERMES_HOME="$hermes_home" hermes plugins enable foundation \
    || die "enabling the native plugin failed (the install itself is complete)"
fi

if [ "$enable_hooks" = "1" ]; then
  # Opt-in only. Backs up config.yaml, appends a hooks block, approves exactly
  # these hook commands in the allowlist. Never restarts Hermes.
  "$bin_dir/foundation" enable --hermes-home "$hermes_home" --bin "$bin_dir/foundation" \
    || die "enabling hooks failed (the install itself is complete)"
else
  cat <<EOF
The native plugin is enabled. No config.yaml, hook allowlist, or running gateway was changed.
To enable all remaining shell hooks, re-run with --enable-hooks, or run:
  $bin_dir/foundation enable
To enable selectively, review $share_dir/adapters/hermes/hooks/INSTALL.md and copy
only the entry you want from hooks.example.yaml. Use this command path:
  $bin_dir/foundation
EOF
fi
