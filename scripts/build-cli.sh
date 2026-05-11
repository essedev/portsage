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

TARGET="${TAURI_ENV_TARGET_TRIPLE:-$(rustc -vV | sed -n 's/host: //p')}"
if [ -z "$TARGET" ]; then
    echo "build-cli: could not determine host target triple" >&2
    exit 1
fi

echo "build-cli: target = $TARGET"

cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" -p portsage-cli

OUT_DIR="$REPO_ROOT/src-tauri/binaries"
mkdir -p "$OUT_DIR"
cp "$REPO_ROOT/target/release/portsage-cli" "$OUT_DIR/portsage-cli-$TARGET"

echo "build-cli: staged $OUT_DIR/portsage-cli-$TARGET"
