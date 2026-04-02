# Build e Release

## Prerequisiti

- Rust (rustup): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- Node.js >= 20
- pnpm: `npm i -g pnpm`
- uv (Python): `curl -LsSf https://astral.sh/uv/install.sh | sh`

## Sviluppo

```bash
pnpm install                  # installa dipendenze frontend
cd mcp && uv sync && cd ..    # installa dipendenze MCP
pnpm tauri dev                # avvia app in dev mode (hot reload)
```

## Build

```bash
pnpm tauri build
```

Output:
- `src-tauri/target/release/bundle/macos/Grimport.app`
- `src-tauri/target/release/bundle/dmg/Grimport_<version>_aarch64.dmg`

## Release di una nuova versione

### 1. Aggiorna la versione

Modifica la versione in due posti:
- `src-tauri/tauri.conf.json` -> `"version": "X.Y.Z"`
- `package.json` -> `"version": "X.Y.Z"`

### 2. Build

```bash
pnpm tauri build
```

### 3. Commit e tag

```bash
git add -A
git commit -m "Release vX.Y.Z"
git tag vX.Y.Z
git push origin main --tags
```

### 4. Crea GitHub Release

```bash
gh release create vX.Y.Z \
  src-tauri/target/release/bundle/dmg/Grimport_X.Y.Z_aarch64.dmg \
  --title "vX.Y.Z" \
  --notes "Descrizione delle modifiche"
```

### 5. Aggiorna Homebrew cask

```bash
# Calcola SHA256 del DMG dalla release
curl -sL "https://github.com/essedev/grimport/releases/download/vX.Y.Z/Grimport_X.Y.Z_aarch64.dmg" | shasum -a 256

# Aggiorna il cask
cd /tmp
git clone https://github.com/essedev/homebrew-grimport.git
cd homebrew-grimport
```

In `Casks/grimport.rb` aggiorna:
- `version "X.Y.Z"`
- `sha256 "<nuovo-sha>"`

```bash
git add -A
git commit -m "Update grimport to vX.Y.Z"
git push origin main
```

### 6. Verifica

```bash
brew update
brew upgrade grimport
```

## Struttura repo

### essedev/grimport (questo repo)
Codice sorgente dell'app. Le release contengono il DMG come asset.

### essedev/homebrew-grimport
Homebrew tap. Contiene solo `Casks/grimport.rb` che punta al DMG nella release.

```
homebrew-grimport/
  Casks/
    grimport.rb     # version, sha256, url della release
```

### Installazione da Homebrew

```bash
brew tap essedev/grimport      # clona il tap localmente (una volta sola)
brew install grimport           # scarica il DMG e installa l'app
brew upgrade grimport           # aggiorna a nuova versione
```
