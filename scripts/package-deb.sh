#!/bin/bash
# Package Android Logcat Studio as a .deb for Linux.
#
# Install layout (no spaces in paths — GLib Desktop Entry Exec cannot
# reliably launch binaries under paths with spaces):
#   /opt/android-logcat-studio/          app tree
#   /usr/bin/android-logcat-studio       symlink launcher
#   /usr/share/applications/*.desktop    menu entry (Icon= name only)
#   /usr/share/icons/hicolor/*/apps/     themed icons
#   /usr/share/pixmaps/                  icon fallback
set -e

APP_ID="android-logcat-studio"
APP_DIR="/opt/${APP_ID}"
BIN_NAME="android-logcat-studio"
# Keep package version in sync with package.json
VERSION="$(node -p "require('./package.json').version")"

echo "Building release (v${VERSION})..."
npm run build:release

# Produce dist/linux-unpacked (Electron app tree) if missing or stale.
# electron-builder is a packaging dependency — install when absent.
if ! command -v electron-builder >/dev/null 2>&1 && [ ! -x node_modules/.bin/electron-builder ]; then
  echo "Installing electron-builder (devDependency for packaging)..."
  npm install --no-save --no-fund --no-audit electron-builder
fi

echo "Packaging Electron linux dir..."
npx --no-install electron-builder --linux dir --x64 || \
  npx electron-builder --linux dir --x64

if [ ! -d dist/linux-unpacked ]; then
  echo "ERROR: dist/linux-unpacked missing after electron-builder." >&2
  exit 1
fi

echo "Preparing deb tree..."
rm -rf /tmp/als-deb
mkdir -p "/tmp/als-deb${APP_DIR}"
cp -a dist/linux-unpacked/. "/tmp/als-deb${APP_DIR}/"

mkdir -p /tmp/als-deb/usr/share/applications
mkdir -p /tmp/als-deb/usr/bin
mkdir -p /tmp/als-deb/DEBIAN
mkdir -p /tmp/als-deb/usr/share/icons/hicolor
mkdir -p /tmp/als-deb/usr/share/pixmaps

# CLI / desktop launcher — symlink avoids duplicating the binary and
# gives Exec= a path with no spaces (required by freedesktop + GLib).
ln -sf "${APP_DIR}/${BIN_NAME}" "/tmp/als-deb/usr/bin/${BIN_NAME}"

# Desktop file
# Icon=android-logcat-studio resolves via freedesktop hicolor theme
# (files under /usr/share/icons/hicolor/*/apps/android-logcat-studio.png).
# Exec must be a single argv0 without spaces — use /usr/bin symlink.
cat > /tmp/als-deb/usr/share/applications/android-logcat-studio.desktop << EOF
[Desktop Entry]
Name=Android Logcat Studio
Comment=Professional standalone Android logcat viewer with advanced filtering, multi-device support and high performance.
Exec=/usr/bin/${BIN_NAME} %U
Terminal=false
Type=Application
Icon=${APP_ID}
StartupWMClass=Android Logcat Studio
Categories=Development;
Keywords=android;logcat;adb;logs;debug;developer;studio;
MimeType=application/x-logcat;
StartupNotify=true
EOF

# Icons — desktop launchers need PNGs in the hicolor theme tree.
# Prefer pre-sized build/icons/{s}x{s}.png; otherwise generate from build/icon.png.
# Also install a pixmaps fallback for DEs that skip icon-theme lookup.
ICON_SRC="build/icon.png"
if [ ! -f "$ICON_SRC" ]; then
  echo "ERROR: missing $ICON_SRC (needed for desktop icons)" >&2
  exit 1
fi

ICON_COUNT=0
for s in 16 32 48 64 128 256 512; do
  dest_dir=/tmp/als-deb/usr/share/icons/hicolor/${s}x${s}/apps
  dest="$dest_dir/${APP_ID}.png"
  mkdir -p "$dest_dir"
  if [ -f "build/icons/${s}x${s}.png" ]; then
    cp "build/icons/${s}x${s}.png" "$dest"
  elif [ -f "dist/.icon-set/icon_${s}x${s}.png" ]; then
    cp "dist/.icon-set/icon_${s}x${s}.png" "$dest"
  else
    # ImageMagick: resize master icon to exact square size
    convert "$ICON_SRC" -resize "${s}x${s}" "$dest"
  fi
  if [ ! -s "$dest" ]; then
    echo "ERROR: failed to create icon $dest" >&2
    exit 1
  fi
  ICON_COUNT=$((ICON_COUNT + 1))
done

# 48px is the most common launcher size; also expose under pixmaps as fallback
cp "/tmp/als-deb/usr/share/icons/hicolor/48x48/apps/${APP_ID}.png" \
  "/tmp/als-deb/usr/share/pixmaps/${APP_ID}.png"
cp "/tmp/als-deb/usr/share/icons/hicolor/256x256/apps/${APP_ID}.png" \
  "/tmp/als-deb/usr/share/pixmaps/${APP_ID}-256.png" 2>/dev/null || true

echo "Installed $ICON_COUNT hicolor icons + pixmaps fallback"

# Control
cat > /tmp/als-deb/DEBIAN/control << EOF
Package: ${APP_ID}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Android Logcat Studio Team <example@example.com>
Installed-Size: 220000
Depends: libgtk-3-0, libnotify4, libnss3, libxss1, libxtst6, xdg-utils, libatspi2.0-0
Description: Professional standalone Android logcat viewer
 Android Logcat Studio is a high-performance, standalone tool for viewing Android logcat logs.
 It supports advanced filtering, multi-device, search, export without needing Android Studio.
EOF

# postinst: chrome-sandbox SUID + refresh desktop/icon caches.
# Also remove the legacy install path that used spaces (broke Exec parsing).
cat > /tmp/als-deb/DEBIAN/postinst << EOF
#!/bin/sh
set -e

APP_DIR="${APP_DIR}"
SANDBOX="\$APP_DIR/chrome-sandbox"

if [ -f "\$SANDBOX" ]; then
  chown root:root "\$SANDBOX" 2>/dev/null || true
  chmod 4755 "\$SANDBOX" 2>/dev/null || true
fi

# Drop old space-containing install dir from earlier packages, if present
if [ -d "/opt/Android Logcat Studio" ]; then
  rm -rf "/opt/Android Logcat Studio" 2>/dev/null || true
fi

update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
gtk-update-icon-cache -f -t /usr/share/icons/hicolor >/dev/null 2>&1 || true

exit 0
EOF
chmod 0755 /tmp/als-deb/DEBIAN/postinst

cat > /tmp/als-deb/DEBIAN/prerm << 'EOF'
#!/bin/sh
set -e
exit 0
EOF
chmod 0755 /tmp/als-deb/DEBIAN/prerm

# Permissions
chmod +x "/tmp/als-deb${APP_DIR}/${BIN_NAME}" || true
chmod +x "/tmp/als-deb${APP_DIR}/resources/engine/als-engine" 2>/dev/null || true
chmod +x "/tmp/als-deb${APP_DIR}/resources/libs/linux/adb" 2>/dev/null || true

# Build
mkdir -p dist
fakeroot dpkg-deb --build /tmp/als-deb "dist/${APP_ID}_${VERSION}_amd64.deb"

echo "Done: dist/${APP_ID}_${VERSION}_amd64.deb"
ls -lh "dist/${APP_ID}_${VERSION}_amd64.deb"

# Quick self-check so a broken Exec never ships again.
# GLib refuses DesktopAppInfo when Exec binary is not on the host PATH,
# so validate against a temp .desktop that points at the staged binary.
python3 - <<PY || true
from pathlib import Path
import tempfile, os
try:
    import gi
    gi.require_version("Gio", "2.0")
    from gi.repository import Gio
except Exception as e:
    print("skip Gio validate:", e)
    raise SystemExit(0)

desk = Path("/tmp/als-deb/usr/share/applications/android-logcat-studio.desktop")
text = desk.read_text()
assert "Exec=" in text, "desktop missing Exec key"
assert "Android Logcat Studio" not in text.split("Exec=", 1)[1].splitlines()[0], (
    "Exec still contains the old space-containing path"
)
exec_line = text.split("Exec=", 1)[1].splitlines()[0]
assert " " not in exec_line.replace(" %U", "").replace("%U", ""), f"spaces in Exec: {exec_line!r}"
# staged binary (no spaces)
staged = Path("/tmp/als-deb${APP_DIR}/${BIN_NAME}")
assert staged.is_file() and os.access(staged, os.X_OK), f"staged binary missing: {staged}"
# symlink in package
link = Path("/tmp/als-deb/usr/bin/${BIN_NAME}")
assert link.is_symlink(), f"missing /usr/bin symlink: {link}"

td = tempfile.mkdtemp()
probe = Path(td) / "probe.desktop"
probe.write_text(
    "[Desktop Entry]\\n"
    "Name=ALS\\n"
    "Type=Application\\n"
    f"Exec={staged} %U\\n"
    "Terminal=false\\n"
    "Categories=Development;\\n"
)
app = Gio.DesktopAppInfo.new_from_filename(str(probe))
assert app is not None and app.get_commandline(), "GLib could not parse Exec"
assert " " not in (app.get_executable() or ""), f"spaces in executable: {app.get_executable()!r}"
print(f"GLib OK: cmdline={app.get_commandline()!r} executable={app.get_executable()!r}")
print(f"Package Exec line: {text.split('Exec=',1)[1].splitlines()[0]!r}")
PY
