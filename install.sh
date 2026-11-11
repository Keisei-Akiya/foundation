#!/bin/sh
# foundation installer.
#
#   curl -fsSL https://raw.githubusercontent.com/Keisei-Akiya/foundation/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/Keisei-Akiya/foundation/main/install.sh | sh -s -- --version 0.0.1
#
# Downloads the prebuilt release from GitHub Releases, verifies its SHA256, then
# installs into <HERMES_HOME>. It never edits config.yaml, hook allowlists, or
# restarts Hermes.
set -eu

REPO="Keisei-Akiya/foundation"
TARGET="x86_64-unknown-linux-musl"
ASSET="foundation-$TARGET.tar.gz"

version="latest"
hermes_home="${HERMES_HOME:-${HOME:?HOME is not set}/.hermes}"

usage() {
  cat <<EOF
Usage: install.sh [--version X.Y.Z] [--hermes-home DIR]

Installs foundation (ecc-hermes-hooks binary, adapters, skills, docs).

  --version X.Y.Z    Release to install (default: latest).
  --hermes-home DIR  Target Hermes home (default: \$HERMES_HOME or ~/.hermes).

Installed paths:
  <HERMES_HOME>/bin/ecc-hermes-hooks
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
[ -x "$src/bin/ecc-hermes-hooks" ] || die "archive is missing bin/ecc-hermes-hooks"
[ -f "$src/VERSION" ] || die "archive is missing VERSION"
installed_version="$(cat "$src/VERSION")"

bin_dir="$hermes_home/bin"
share_dir="$hermes_home/foundation"
mkdir -p "$bin_dir" "$hermes_home"

install -m 0755 "$src/bin/ecc-hermes-hooks" "$bin_dir/ecc-hermes-hooks.new"
mv -f "$bin_dir/ecc-hermes-hooks.new" "$bin_dir/ecc-hermes-hooks"
rm -rf "$src/bin"

# Replace the shared tree in two renames so a failure leaves the old copy intact.
rm -rf "$share_dir.new"
mv "$src" "$share_dir.new"
rm -rf "$share_dir.old"
[ ! -e "$share_dir" ] || mv "$share_dir" "$share_dir.old"
mv "$share_dir.new" "$share_dir"
rm -rf "$share_dir.old"

cat <<EOF

Installed foundation $installed_version
  binary: $bin_dir/ecc-hermes-hooks
  files : $share_dir

No Hermes config, hook allowlist, or running gateway was changed.
To enable a hook, review $share_dir/adapters/hermes/hooks/INSTALL.md and copy
only the entry you want from hooks.example.yaml. Use this command path:
  $bin_dir/ecc-hermes-hooks
EOF
