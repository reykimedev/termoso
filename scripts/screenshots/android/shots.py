"""Takes the Android pictures (android-*.png) on a running emulator with the app installed.

    python3 shots.py <out dir> [name ...]

Steps run in order and build on each other (hosts imported first, then a session, ...).
A failed step prints what the screen showed and the run goes on; the exit code says if any
failed. Example data only: example.com names, 192.0.2.0/24 addresses (sent to the test SSH
server by the emulator's firewall), made-up keys."""

import os
import sys
import time
import traceback

import re

from drive import (NotFound, back, find, has, hide_keyboard, key, labels, long_press, nodes,
                   screen_size, sh, shot, swipe, tap, tap_xy, type_text, wait)

IMPORT_BUTTON = re.compile(r"^Import \d+ hosts?$")

PKG = os.environ.get("SHOTS_PACKAGE", "dev.labdex.labterm")
OUT = sys.argv[1]
ONLY = set(sys.argv[2:])
PASSWORD = "example-password"

IMPORT = """Host web-01
  HostName 192.0.2.11
  User deploy
Host web-02
  HostName 192.0.2.12
  User deploy
Host db-01
  HostName 192.0.2.21
  User postgres
Host k3s-1
  HostName 192.0.2.31
  User admin
Host bastion
  HostName 192.0.2.5
  User ops
Host nas
  HostName 192.0.2.40
  User admin
  Port 2222"""

failed = []


def step(name, keep=True):
    """Registers a picture; `keep=False` steps only set things up."""
    def deco(fn):
        STEPS.append((name, fn, keep))
        return fn
    return deco


STEPS = []


def nav(tab):
    tap(tab)
    time.sleep(0.8)


def home():
    sh(f"am start -n {PKG}/dev.labdex.labterm.MainActivity")
    time.sleep(1.5)


def fill(label, value):
    """Focuses the text field labelled `label` and types `value` into it."""
    tap(label)
    sh("input keyevent KEYCODE_MOVE_END")
    for _ in range(40):
        sh("input keyevent KEYCODE_DEL")
    type_text(value)


@step("setup", keep=False)
def setup():
    sh(f"pm grant {PKG} android.permission.POST_NOTIFICATIONS")
    home()
    if has("Continue without sync"):
        tap("Continue without sync")
    time.sleep(1.5)
    # Anything asking at first start (notifications, battery) is answered so it stays away.
    for label in ("Allow", "Not now", "Skip", "Later"):
        if has(label):
            tap(label)


@step("android-quick-import")
def quick_import():
    nav("Vaults")
    tap("Add")
    tap("Import hosts")
    time.sleep(1)
    fields = [n for n in nodes() if n.cls.endswith("EditText")]
    if fields:
        tap_xy(*fields[0].center)
    type_text(IMPORT)
    hide_keyboard()
    wait("Detected: ssh_config", exact=False)


@step("android-hosts")
def hosts():
    w, h = screen_size()
    for _ in range(4):
        if has(IMPORT_BUTTON):
            break
        swipe(w // 2, int(h * 0.85), w // 2, int(h * 0.45))
    tap(IMPORT_BUTTON)
    wait("web-01", timeout=10)
    time.sleep(1)


@step("android-vault")
def vault():
    tap("Back")
    wait("Keychain")


@step("android-terminal")
def terminal():
    tap("Hosts")
    tap("web-01")
    if has("Connect"):
        tap("Connect")
    # Unknown host key: the test server's key is trusted here, once, like a person would.
    wait("Trust & save", timeout=40)
    tap("Trust & save")
    wait("Password", timeout=30)
    type_text(PASSWORD)
    if has("Save in vault"):
        tap("Save in vault")
    key("ENTER")
    time.sleep(4)
    for cmd in ("uptime", "ls -la", "df -h /", "tail -n 5 logs/app.log"):
        type_text(cmd + "\n")
        time.sleep(1.2)
    hide_keyboard()


@step("android-session-menu")
def session_menu():
    tap("Session actions")
    wait("Close session")


@step("android-key-panel")
def key_panel():
    back()
    time.sleep(0.5)
    tap("Show key panel")
    wait("Customize")


@step("android-hidden-input")
def hidden_input():
    tap("Password")
    wait("Hidden input")


@step("android-compose")
def compose():
    back()
    hide_keyboard()
    if not has("Compose"):
        tap("More keys")
    tap("Compose")
    time.sleep(0.8)
    fields = [n for n in nodes() if n.cls.endswith("EditText")]
    if fields:
        tap_xy(*fields[-1].center)
    type_text("Refactor the auth module: extract token validation into a helper and add unit tests for expired and malformed tokens.")
    wait("Paste only")


@step("android-connections")
def connections():
    back()
    hide_keyboard()
    back()
    time.sleep(0.5)
    nav("Connections")
    wait("Ways to connect", exact=False)


@step("android-sftp")
def sftp():
    tap("Files")
    time.sleep(1)
    tap("web-01")
    wait("logs", timeout=30)


@step("android-ai-agent")
def ai_agent():
    home()
    nav("Vaults")
    hit = wait("web-01")[0]
    long_press(*hit.center)
    tap("Edit", exact=False)
    wait("web-01")
    w, h = screen_size()
    for _ in range(8):
        if has("AI coding agent", exact=False):
            break
        swipe(w // 2, int(h * 0.8), w // 2, int(h * 0.35))
    tap("AI coding agent", exact=False)
    wait("Claude Code", exact=False)


@step("android-keys")
def keys():
    back()
    back()
    home()
    nav("Vaults")
    tap("Back")
    tap("Keychain")
    for label, algo in (("deploy", "Ed25519"), ("backup", "ECDSA"), ("ci-runner", "RSA")):
        tap("Add")
        tap("Generate key")
        fill("Label", label)
        if has("Algorithm"):
            tap("Algorithm")
            if has(algo, exact=False):
                tap(algo, exact=False)
        tap("Save")
        wait(label, timeout=30)


@step("android-pf-rule")
def pf_rule():
    back()
    tap("Port forwarding", exact=False)
    tap("Add") if has("Add") else tap("Create a rule")
    if has("Local forwarding"):
        tap("Local forwarding")
    fill("Label", "Admin panel via web-01")
    if has("Host"):
        tap("Host")
        tap("web-01", index=-1)
    fill("Local port", "8080")
    fill("Destination host", "localhost")
    fill("Destination port", "8000")
    hide_keyboard()


@step("android-settings")
def settings():
    back()
    if has("Discard", exact=False):
        tap("Discard", exact=False)
    home()
    nav("Settings")
    wait("Appearance")


@step("android-gestures-settings")
def gestures():
    tap("Keyboard & gestures")
    wait("Gestures", exact=False)


@step("android-linux-env")
def linux_env():
    back()
    tap("Linux environment")
    time.sleep(1.5)


@step("android-pin")
def pin():
    back()
    w, h = screen_size()
    for _ in range(6):
        if has("App PIN / pattern"):
            break
        swipe(w // 2, int(h * 0.8), w // 2, int(h * 0.4))
    tap("App PIN / pattern")
    time.sleep(1)
    if has("Set up"):
        tap("Set up")
    wait("PIN")


@step("android-shade")
def shade():
    back()
    sh("cmd statusbar expand-notifications")
    time.sleep(1.5)


def calm_status_bar():
    """System UI demo mode: 9:42, full battery and signal, no notification icons."""
    demo = "am broadcast -a com.android.systemui.demo -e command"
    sh(f"settings put global sysui_demo_allowed 1; {demo} enter; {demo} clock -e hhmm 0942; "
       f"{demo} battery -e level 100 -e plugged false; {demo} network -e wifi show -e level 4 -e fully true; "
       f"{demo} network -e mobile show -e level 4 -e datatype none; {demo} notifications -e visible false")


def main():
    os.makedirs(OUT, exist_ok=True)
    w, h = screen_size()
    print(f"screen {w}x{h}")
    calm_status_bar()
    print(sh("am broadcast -a com.android.systemui.demo -e command clock -e hhmm 0942"), flush=True)
    for name, fn, keep in STEPS:
        try:
            fn()
            if keep and (not ONLY or name in ONLY):
                if name != "android-shade":
                    calm_status_bar()
                shot(os.path.join(OUT, f"{name}.png"))
                print(f"ok   {name}", flush=True)
        except Exception as e:  # noqa: BLE001 - report and go on
            failed.append(name)
            print(f"FAIL {name}: {e}", flush=True)
            traceback.print_exc()
            try:
                shot(os.path.join(OUT, f"{name}.failed.png"))
                print(f"     screen: {labels()}", flush=True)
            except Exception:  # noqa: BLE001
                pass
    if failed:
        print(f"{len(failed)} failed: {' '.join(failed)}")
        sys.exit(1)


main()
