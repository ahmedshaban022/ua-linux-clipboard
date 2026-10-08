#!/bin/sh
# UA Clipboard installer (draft): builds from source and installs the
# binaries into ~/.local/bin, then offers first-run setup.
set -eu

cd "$(dirname "$0")"

echo "==> Building UA Clipboard (release)"
cargo build --release

BIN_DIR="${HOME}/.local/bin"
mkdir -p "$BIN_DIR"
for bin in ua-clipboard-daemon ua-clipboard-panel ua-clipboard; do
    install -m 0755 "target/release/$bin" "$BIN_DIR/$bin"
    echo "    installed $BIN_DIR/$bin"
done

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "==> NOTE: add $BIN_DIR to your PATH" ;;
esac

echo "==> Running first-run setup (autostart + Super+V)"
if "$BIN_DIR/ua-clipboard" setup; then
    echo "==> Done. Start the daemon:  ua-clipboard-daemon"
    echo "    Open the panel:          ua-clipboard-panel   (or Super+V)"
    echo "    Diagnose anytime:        ua-clipboard doctor"
else
    echo "==> Setup incomplete — start the daemon first, then re-run:"
    echo "    ua-clipboard-daemon &  &&  ua-clipboard setup"
fi
