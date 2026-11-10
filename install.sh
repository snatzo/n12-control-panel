#!/bin/bash
#
# install.sh - build n12ctl/n12panel and install them for normal use.
#
#   * n12panel -> ~/Applications/n12panel.app   (double-click to open; also in
#                 Launchpad/Finder)
#   * n12ctl   -> /usr/local/bin/n12ctl         (or ~/bin if that is not writable)
#
# Why a wrapper inside the .app: the panel finds the n12-monterey checkout
# relative to its own location, which is meaningless once it lives in
# /Applications. So the app bundle's executable is a tiny script that records
# where n12-monterey was at install time in $N12_TOOLS, then runs the real
# binary (stored alongside it as n12panel-bin).

set -u

REPO="$(cd "$(dirname "$0")" && pwd)"
cd "$REPO"

# Make sure cargo is on PATH (rustup puts it in ~/.cargo/env).
command -v cargo >/dev/null 2>&1 || . "$HOME/.cargo/env" 2>/dev/null || true
if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo not found - install Rust from https://rustup.rs" >&2
    exit 1
fi

echo "building (release)..."
if ! cargo build --release; then
    echo "build failed" >&2
    exit 1
fi

# --- find n12-monterey (the panel needs it to change the sample rate) --------
TOOLS="${N12_TOOLS:-}"
if [ -z "$TOOLS" ]; then
    for c in ../n12-monterey/tools \
             "$HOME/Documents/OpenCode_Projects/n12-monterey/tools"; do
        if [ -x "$c/n12_on.sh" ]; then
            TOOLS="$(cd "$c" && pwd)"
            break
        fi
    done
fi
if [ -z "$TOOLS" ]; then
    echo "warning: could not find n12-monterey's tools." >&2
    echo "         set N12_TOOLS=/path/to/n12-monterey/tools and re-run." >&2
fi

# --- the .app ---------------------------------------------------------------
APP_DIR="$HOME/Applications"
[ -w /Applications ] && APP_DIR="/Applications"
APP="$APP_DIR/n12panel.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"

# The real binary, under a name the wrapper points at.
cp target/release/n12panel "$APP/Contents/MacOS/n12panel-bin"

# The wrapper: export where n12-monterey is, then exec the real binary.
cat > "$APP/Contents/MacOS/n12panel" <<WRAP
#!/bin/zsh
export N12_TOOLS="$TOOLS"
exec "\$(dirname "\$0")/n12panel-bin" "\$@"
WRAP
chmod +x "$APP/Contents/MacOS/n12panel"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>n12panel</string>
  <key>CFBundleDisplayName</key><string>n12 Control Panel</string>
  <key>CFBundleIdentifier</key><string>com.n12monterey.panel</string>
  <key>CFBundleExecutable</key><string>n12panel</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

echo "installed the app:  $APP"

# --- icon -------------------------------------------------------------------
# If a source PNG is present (assets/icon_1024.png), turn it into a .icns and
# drop it into the bundle. `sips` resizes and `iconutil` packs the iconset.
ICON_SRC="$REPO/assets/icon_1024.png"
if [ -f "$ICON_SRC" ]; then
    WORK="$(mktemp -d)"
    ICONSET="$WORK/AppIcon.iconset"
    mkdir -p "$ICONSET" "$APP/Contents/Resources"
    for s in 16 32 128 256 512; do
        sips -z "$s" "$s" "$ICON_SRC" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null 2>&1
        d=$((s * 2))
        sips -z "$d" "$d" "$ICON_SRC" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null 2>&1
    done
    if iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns" 2>/dev/null; then
        echo "installed the icon:  $APP/Contents/Resources/AppIcon.icns"
    fi
    rm -rf "$WORK"
fi

# --- the command-line tool --------------------------------------------------
BIN_DIR="/usr/local/bin"
if [ ! -w "$BIN_DIR" ]; then
    BIN_DIR="$HOME/bin"
fi
mkdir -p "$BIN_DIR"
cp target/release/n12ctl "$BIN_DIR/n12ctl"
echo "installed the CLI:  $BIN_DIR/n12ctl"
if [ "$BIN_DIR" = "$HOME/bin" ]; then
    case ":$PATH:" in
        *":$HOME/bin:"*) ;;
        *) echo "  (note: add  export PATH=\"\$HOME/bin:\$PATH\"  to ~/.zshrc to run 'n12ctl' by name)" ;;
    esac
fi

echo
echo "Done. Open the panel from Finder/Launchpad, or:"
echo "    open \"$APP\""
if [ -n "$TOOLS" ]; then
    echo "The panel will use n12-monterey at: $TOOLS"
fi
