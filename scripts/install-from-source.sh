#!/usr/bin/env bash
# Builds SpecDeck from this checkout and installs it for the current user, on Debian or Ubuntu,
# including Ubuntu in WSL 2 on Windows (the window opens on the Windows desktop through WSLg).
# For machines where the release installers cannot be used.
#
# - Installs Tauri's system libraries with apt (asks for your sudo password, skipped when present)
# - Installs mise and the tools pinned in mise.toml (Rust, Node, pnpm, just, cmake), per user
# - Builds the app with the public sharing service built in, like the release builds
# - Adds a `specdeck` command to ~/.local/bin and a SpecDeck entry to the application menu
#   (in WSL, Windows shows it in the Start menu under "Ubuntu")
#
# Run it again after `git pull` to update.
# Usage: scripts/install-from-source.sh
# Environment: SPECDECK_SIGNALLING_URL  sharing service to build in (default: the public one)
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
signalling_url=${SPECDECK_SIGNALLING_URL:-wss://specdeck.hartj.es/signal}
bin_dir="$HOME/.local/bin"
mise="$bin_dir/mise"

if ! command -v apt-get > /dev/null; then
  echo "This script needs apt (Debian or Ubuntu). On other systems, see the Development section of the README." >&2
  exit 1
fi

# 1. System libraries. libclang-dev is for bindgen (libdatachannel's bindings); the rest is Tauri's list from CI.
packages=(build-essential pkg-config curl file libclang-dev libssl-dev
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf)
missing=()
for package in "${packages[@]}"; do
  dpkg-query -W -f='${Status}' "$package" 2> /dev/null | grep -q 'install ok installed' || missing+=("$package")
done
if [ ${#missing[@]} -gt 0 ]; then
  echo "==> Installing system packages: ${missing[*]}"
  sudo apt-get update
  sudo apt-get install -y "${missing[@]}"
fi

# 2. Tools pinned in mise.toml.
if [ ! -x "$mise" ]; then
  echo "==> Installing mise (https://mise.jdx.dev) to $bin_dir"
  curl -fsSL https://mise.run | sh
fi
cd "$repo"
"$mise" trust "$repo/mise.toml"
echo "==> Installing Rust, Node, pnpm, just and cmake (mise.toml)"
"$mise" install

# 3. Build. The built-in sharing service is read at compile time, so touch the file that reads it:
# otherwise cargo reuses a build made with a different SPECDECK_SIGNALLING_URL.
echo "==> Building SpecDeck (sharing service: $signalling_url); the first build takes several minutes"
"$mise" exec -- pnpm install --frozen-lockfile
touch app/src-tauri/src/services.rs
SPECDECK_SIGNALLING_URL="$signalling_url" "$mise" exec -- pnpm tauri build --no-bundle
app="$repo/target/release/specdeck-app"

# 4. Launcher and menu entry.
mkdir -p "$bin_dir" "$HOME/.local/share/applications"
env_line=""
# WebKitGTK's DMA-BUF renderer shows a blank window under WSLg.
if grep -qi microsoft /proc/version 2> /dev/null; then
  env_line="export WEBKIT_DISABLE_DMABUF_RENDERER=1"
fi
cat > "$bin_dir/specdeck" << EOF
#!/usr/bin/env bash
# Starts SpecDeck (installed by scripts/install-from-source.sh).
# Usage: specdeck [--background]
#   By default it stays attached to the terminal: output shows there, Ctrl+C quits.
#   --background (-b) detaches it and logs to ~/.cache/specdeck.log.
$env_line
case "\${1:-}" in
  --background | -b)
    shift
    mkdir -p "\$HOME/.cache"
    setsid nohup "$app" "\$@" > "\$HOME/.cache/specdeck.log" 2>&1 < /dev/null &
    echo "SpecDeck started in the background (log: ~/.cache/specdeck.log)"
    ;;
  --foreground | -f)
    shift
    exec "$app" "\$@"
    ;;
  *)
    exec "$app" "\$@"
    ;;
esac
EOF
chmod +x "$bin_dir/specdeck"

cat > "$HOME/.local/share/applications/specdeck.desktop" << EOF
[Desktop Entry]
Type=Application
Name=SpecDeck
Comment=Share a coding agent session with your team
Exec=$bin_dir/specdeck
Icon=$repo/app/src-tauri/icons/128x128.png
Categories=Development;
Terminal=false
EOF

echo
echo "SpecDeck is installed. Start it with: specdeck (or specdeck --background to free the terminal)"
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) echo "Add $bin_dir to your PATH first, or open a new terminal (Ubuntu adds it when the folder exists)." ;;
esac
if [ -n "$env_line" ]; then
  echo "From Windows (PowerShell or Windows Terminal): wsl -- bash -lc specdeck"
fi
