#!/usr/bin/env bash
# Build the portsage CLI and stage it where Tauri's externalBin bundler
# expects to find it: `src-tauri/binaries/portsage-cli-<HOST_TARGET_TRIPLE>`.
#
# Tauri strips the target-triple suffix when bundling so the binary lands
# inside `Portsage.app/Contents/MacOS/portsage-cli`. The Homebrew cask then
# exposes it on PATH as `portsage`.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Resolve the host target triple. We try several sources because the CI
# step has occasionally raced with `dtolnay/rust-toolchain` PATH setup,
# leaving `rustc` momentarily unresolvable inside `$(...)` (which doesn't
# trigger `set -e`, so a silent empty TARGET would slip through).
TARGET="${TAURI_ENV_TARGET_TRIPLE:-}"
if [ -z "$TARGET" ]; then
    TARGET="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"
fi
if [ -z "$TARGET" ]; then
    # `rustup show active-toolchain` prints e.g. "stable-aarch64-apple-darwin (default)".
    TARGET="$(rustup show active-toolchain 2>/dev/null \
        | awk '{print $1}' \
        | grep -oE '[a-z0-9_]+-[a-zA-Z0-9_]+-[a-zA-Z0-9_]+(-[a-zA-Z0-9_]+)?$' \
        || true)"
fi
if [ -z "$TARGET" ] && [ "$(uname -s)" = "Darwin" ]; then
    # Last-resort fallback for macOS hosts: the triple is fully determined by
    # the CPU arch. Avoid this on Linux where gnu vs musl ambiguity matters.
    arch="$(uname -m)"
    [ "$arch" = "arm64" ] && arch="aarch64"
    TARGET="${arch}-apple-darwin"
fi
if [ -z "$TARGET" ]; then
    echo "build-cli: could not determine host target triple" >&2
    echo "build-cli: rustc=$(command -v rustc || echo MISSING) rustup=$(command -v rustup || echo MISSING) uname=$(uname -sm)" >&2
    exit 1
fi

echo "build-cli: target = $TARGET"

cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" -p portsage-cli

OUT_DIR="$REPO_ROOT/src-tauri/binaries"
mkdir -p "$OUT_DIR"
cp "$REPO_ROOT/target/release/portsage-cli" "$OUT_DIR/portsage-cli-$TARGET"

echo "build-cli: staged $OUT_DIR/portsage-cli-$TARGET"
