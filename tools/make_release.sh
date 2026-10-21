#!/bin/sh
# Assemble and archive a release from already-built artefacts.
#
# Usage:
#   tools/make_release.sh <os-arch-tag> [version]
#     <os-arch-tag>  e.g. linux-x86_64 (goes into the archive and zip name)
#     [version]      the release version; read from the workspace Cargo.toml
#                    when omitted
#
# Expects a build to already exist under $CARGO_TARGET_DIR (default "target"),
# made by:
#   cargo nice-plug bundle -p sehrnoise-plugin --release
#   cargo build -p sehrnoise-standalone --release
# that is:
#   $CARGO_TARGET_DIR/bundled/sehrnoise-plugin.clap       (a directory on
#       macOS, a single file on Linux and Windows)
#   $CARGO_TARGET_DIR/bundled/sehrnoise-plugin.vst3       (a directory)
#   $CARGO_TARGET_DIR/release/sehrnoise-standalone[.exe]
#
# Produces:
#   $CARGO_TARGET_DIR/releasepkg/sehrnoise-<version>-<tag>.zip
# containing one directory, sehrnoise-<version>-<tag>/:
#   sehrnoise-plugin.clap/     the CLAP bundle (as cargo nice-plug produced it)
#   sehrnoise-plugin.vst3/     the VST3 bundle
#   sehrnoise-standalone       the standalone application
#   README.md LICENSE PROVENANCE.md CLEAN_ROOM.md
#
# The script assembles only; it never builds, so CI can run it on a machine
# without a Rust toolchain (see .github/workflows/ci.yml).
#
# Archiver fallback chain for the three CI runners: `zip` (ubuntu, macOS),
# then `tar.exe` (the runner's bsdtar, on Windows), then `bsdtar`.
set -eu

TAG="$(printf '%s' "$1" | tr -c 'a-zA-Z0-9._-' '-')"
shift

VER=""
if [ "$#" -ge 1 ]; then
    VER="$1"
else
    VER="$(grep -m1 'version =' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')"
fi

TARGET="${CARGO_TARGET_DIR:-target}"
BUNDLED="$TARGET/bundled"
STAGE="$TARGET/releasepkg/sehrnoise-$VER-$TAG"
ARCHIVE="$TARGET/releasepkg/sehrnoise-$VER-$TAG.zip"

NAME="$STAGE/sehrnoise-standalone"
if [ -f "$TARGET/release/sehrnoise-standalone.exe" ]; then
    NAME="$TARGET/release/sehrnoise-standalone.exe"
elif [ -f "$TARGET/release/sehrnoise-standalone" ]; then
    NAME="$TARGET/release/sehrnoise-standalone"
else
    echo "make_release: no binary at $TARGET/release/sehrnoise-standalone[.exe]" >&2
    exit 1
fi

for doc in README.md LICENSE PROVENANCE.md CLEAN_ROOM.md; do
    [ -f "$doc" ] || { echo "make_release: missing $doc (run from the repository root)" >&2; exit 1; }
done

if [ ! -f "$BUNDLED/sehrnoise-plugin.clap" ] && [ ! -d "$BUNDLED/sehrnoise-plugin.clap" ]; then
    echo "make_release: no CLAP bundle at $BUNDLED/sehrnoise-plugin.clap" >&2
    exit 1
fi
[ -d "$BUNDLED/sehrnoise-plugin.vst3" ] || { echo "make_release: no VST3 bundle at $BUNDLED/sehrnoise-plugin.vst3" >&2; exit 1; }

rm -rf "$STAGE" "$ARCHIVE"
mkdir -p "$STAGE"
if [ -d "$BUNDLED/sehrnoise-plugin.clap" ]; then
    cp -R "$BUNDLED/sehrnoise-plugin.clap" "$BUNDLED/sehrnoise-plugin.vst3" "$STAGE/"
else
    cp -R "$BUNDLED/sehrnoise-plugin.vst3" "$STAGE/"
    cp "$BUNDLED/sehrnoise-plugin.clap" "$STAGE/"
fi
cp "$NAME" "$STAGE/"
cp README.md LICENSE PROVENANCE.md CLEAN_ROOM.md "$STAGE/"

# the staging directory is the current directory of the archive, so that the
# zip unpacks as one folder
cd "$TARGET/releasepkg"

if command -v zip >/dev/null 2>&1; then
    zip -rq "$ARCHIVE" "sehrnoise-$VER-$TAG"
elif command -v tar.exe >/dev/null 2>&1; then
    # windows-latest: the runner's System32 bsdtar makes zips with -a
    tar.exe -a -cf "$ARCHIVE" "sehrnoise-$VER-$TAG"
elif command -v bsdtar >/dev/null 2>&1; then
    bsdtar -a -cf "$ARCHIVE" "sehrnoise-$VER-$TAG"
else
    echo "make_release: no archiver found (zip, tar.exe, bsdtar)" >&2
    exit 1
fi

echo "built $ARCHIVE"
