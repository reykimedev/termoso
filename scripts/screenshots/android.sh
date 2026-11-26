#!/usr/bin/env bash
# Takes the Android pictures (android-*.png) on a running emulator.
#
#   scripts/screenshots/android.sh <apk> <out dir> [shot ...]
#
# Needs: adb with one emulator (a google_apis image, so adb can be root), the test SSH server
# from android/sshd listening on the host's port 2222, python3 with Pillow.
# .github/workflows/screenshots-android.yml does all of that on a GitHub runner.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
apk="${1:?apk}"; out="${2:?out dir}"; shift 2
pkg="${SHOTS_PACKAGE:-dev.labdex.labterm}"

adb wait-for-device
adb root >/dev/null; sleep 2; adb wait-for-device
# Example addresses go to the test server on the host (10.0.2.2:2222).
adb shell "iptables -t nat -A OUTPUT -p tcp -d 192.0.2.0/24 --dport 22 -j DNAT --to-destination 10.0.2.2:2222"
adb shell "iptables -t nat -A OUTPUT -p tcp -d 192.0.2.0/24 --dport 2222 -j DNAT --to-destination 10.0.2.2:2222"
# Same look every time: dark, 24-hour clock, no animations (shots.py sets the status bar).
adb shell "cmd uimode night yes"
adb shell "settings put global window_animation_scale 0; settings put global transition_animation_scale 0; settings put global animator_duration_scale 0"
adb shell "settings put system time_12_24 24"
# Text goes in through adb as a hardware keyboard: no soft keyboard covering the pictures, and
# no Back needed to hide one (Back in the terminal leaves it).
adb shell "settings put secure show_ime_with_hard_keyboard 0"
adb uninstall "$pkg" >/dev/null 2>&1 || true
adb install -r "$apk"

mkdir -p "$out/raw"
status=0
SHOTS_PACKAGE="$pkg" python3 "$here/android/shots.py" "$out/raw" "$@" || status=$?
# Phone captures are 1080x2400; the docs use half size.
python3 - "$out/raw" "$out" <<'PY'
import glob, os, sys
from PIL import Image
for f in glob.glob(os.path.join(sys.argv[1], "*.png")):
    im = Image.open(f)
    im.resize((im.width // 2, im.height // 2), Image.LANCZOS).save(os.path.join(sys.argv[2], os.path.basename(f)), optimize=True)
PY
exit $status
