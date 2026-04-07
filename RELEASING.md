# Build and Release

## Prerequisites

- Rust (rustup): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- Node.js >= 20
- pnpm: `npm i -g pnpm`
- uv (Python): `curl -LsSf https://astral.sh/uv/install.sh | sh`

## Development

```bash
pnpm install                  # install frontend dependencies
cd mcp && uv sync && cd ..    # install MCP dependencies
pnpm tauri dev                # start the app in dev mode (hot reload)
```

## Build

```bash
pnpm tauri build
```

Output:
- `src-tauri/target/release/bundle/macos/Portsage.app`
- `src-tauri/target/release/bundle/dmg/Portsage_<version>_aarch64.dmg`

## Releasing a new version

### 1. Bump the version

Update the version in two places:
- `src-tauri/tauri.conf.json` -> `"version": "X.Y.Z"`
- `package.json` -> `"version": "X.Y.Z"`

### 2. Build

```bash
pnpm tauri build
```

### 3. Commit and tag

```bash
git add -A
git commit -m "Release vX.Y.Z"
git tag vX.Y.Z
git push origin main --tags
```

### 4. Create the GitHub release

```bash
gh release create vX.Y.Z \
  src-tauri/target/release/bundle/dmg/Portsage_X.Y.Z_aarch64.dmg \
  --title "vX.Y.Z" \
  --notes "Description of the changes"
```

### 5. Update the Homebrew cask

```bash
# Compute the SHA256 of the released DMG
curl -sL "https://github.com/essedev/portsage/releases/download/vX.Y.Z/Portsage_X.Y.Z_aarch64.dmg" | shasum -a 256

# Update the cask
cd /tmp
git clone https://github.com/essedev/homebrew-portsage.git
cd homebrew-portsage
```

In `Casks/portsage.rb` update:
- `version "X.Y.Z"`
- `sha256 "<new-sha>"`

```bash
git add -A
git commit -m "Update portsage to vX.Y.Z"
git push origin main
```

### 6. Verify

```bash
brew update
brew upgrade portsage
```

## Repo structure

### essedev/portsage (this repo)
The app source code. Releases contain the DMG as an asset.

### essedev/homebrew-portsage
The Homebrew tap. Contains only `Casks/portsage.rb`, which points to the DMG in the release.

```
homebrew-portsage/
  Casks/
    portsage.rb     # version, sha256, release URL
```

### Installation via Homebrew

```bash
brew tap essedev/portsage      # clone the tap locally (one-off)
brew install portsage           # download the DMG and install the app
brew upgrade portsage           # upgrade to a new version
```
