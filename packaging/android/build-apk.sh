#!/usr/bin/env bash
# Build the EffectCraft Android app (a debug APK you can install on a phone).
#
#   bash packaging/android/build-apk.sh
#
# Needs: Rust with the wasm32-unknown-unknown target, `wasm-bindgen-cli` 0.2.129, Node 20+,
# JDK 21 and the Android SDK (ANDROID_HOME / ANDROID_SDK_ROOT). The GitHub workflow
# `.github/workflows/android-apk.yml` sets all of that up and runs this script.
#
# What it does: builds the web app (`cargo xtask web`, which holds the phone layout of
# crates/ui-egui/src/mobile.rs), puts it into a Capacitor Android project and builds the APK.
# Output: packaging/android/out/EffectCraft-<version>-debug.apk
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
cd "$root"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
code="${ANDROID_VERSION_CODE:-1}"
target="${CARGO_TARGET_DIR:-target}"

echo "==> 1/5 web build ($version)"
cargo xtask web
rm -rf "$here/www"
cp -R "$target/web/dist" "$here/www"

# Inside the app: no service worker (every file is already on the phone) and one frame worker
# (a phone has little memory). The page reads these flags from its URL.
python3 - "$here/www/index.html" <<'PY'
import sys
p = sys.argv[1]
s = open(p, encoding="utf-8").read()
flag = """<script>
    if (!location.search) history.replaceState(null, "", location.pathname + "?nosw&frameworkers=1");
  </script>
"""
s = s.replace("<script>", flag + "  <script>", 1)
open(p, "w", encoding="utf-8").write(s)
PY

echo "==> 2/5 Capacitor project"
cd "$here"
npm install --no-audit --no-fund
if [ ! -d android ]; then
  npx cap add android
fi
npx cap sync android

echo "==> 3/5 Android settings and icons"
manifest="android/app/src/main/AndroidManifest.xml"
# More memory for the big wasm module, and a window that follows the phone's rotation.
if ! grep -q 'android:largeHeap' "$manifest"; then
  sed -i 's/<application/<application android:largeHeap="true"/' "$manifest"
fi
if ! grep -q 'screenOrientation' "$manifest"; then
  sed -i 's/<activity/<activity android:screenOrientation="fullSensor"/' "$manifest"
fi
gradle_file="android/app/build.gradle"
sed -i "s/versionCode [0-9]*/versionCode $code/; s/versionName \"[^\"]*\"/versionName \"$version\"/" "$gradle_file"
icon="$root/assets/app-icon/hicolor/512x512/apps/ai.storyteller.effectcraft.png"
res="android/app/src/main/res"
rm -rf "$res"/mipmap-anydpi-v26
if command -v convert >/dev/null 2>&1; then
  for d in mdpi:48 hdpi:72 xhdpi:96 xxhdpi:144 xxxhdpi:192; do
    dir="$res/mipmap-${d%%:*}"
    mkdir -p "$dir"
    for n in ic_launcher ic_launcher_round ic_launcher_foreground; do
      convert "$icon" -resize "${d##*:}x${d##*:}" "$dir/$n.png"
    done
  done
else
  echo "ImageMagick not found: the app keeps Capacitor's default icon"
fi

echo "==> 4/5 Gradle"
cd android
./gradlew --no-daemon assembleDebug

echo "==> 5/5 APK"
mkdir -p "$here/out"
cp app/build/outputs/apk/debug/app-debug.apk "$here/out/EffectCraft-$version-debug.apk"
ls -lh "$here/out"
