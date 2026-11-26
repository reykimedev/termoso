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

import drive
from drive import (NotFound, find, foreground, has, hide_keyboard, key, labels, long_press, nodes,
                   screen_size, sh, shot, swipe, switch_state, tap, tap_xy, type_text, wait)

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
    in_app()
    tap(tab)
    time.sleep(0.8)


def home():
    sh(f"am start -n {PKG}/dev.labdex.labterm.MainActivity")
    time.sleep(1.5)


def in_app():
    """Brings the app back if a step left it (a Back too many)."""
    if PKG not in foreground():
        home()


def back():
    drive.back()
    in_app()


def scroll_to(label, exact=True, tries=8):
    w, h = screen_size()
    for _ in range(tries):
        if has(label, exact):
            return
        swipe(w // 2, int(h * 0.75), w // 2, int(h * 0.4))
    wait(label, 2, exact)


def fill(label, value):
    """Focuses the text field labelled `label` and types `value` into it."""
    tap(label)
    sh("input keyevent KEYCODE_MOVE_END")
    for _ in range(40):
        sh("input keyevent KEYCODE_DEL")
    type_text(value)


def to_tabs():
    """Backs out of whatever is open until the bottom tabs show (a prompt sheet ignores Back)."""
    in_app()
    for _ in range(5):
        if has("Vaults") and has("Settings"):
            return
        if has("Close sheet"):
            tap("Close sheet")
        elif has("Cancel"):
            tap("Cancel")
        else:
            drive.back()
        in_app()


def tick(label):
    """Ticks the checkbox on the same row as `label` (the label itself is not clickable)."""
    all_nodes = nodes()
    y = find(label, True, all_nodes)[0].center[1]
    box = [n for n in all_nodes if n.checkable and abs(n.center[1] - y) < 90]
    if box and not box[0].checked:
        tap_xy(*box[0].center)


def answer_password():
    """Answers the password sheet if it shows, and keeps the password in the vault."""
    if wait("Save in vault", timeout=30):
        tick("Save in vault")
        text_field(0)
        type_text(PASSWORD)
        tap("Continue")


def open_session():
    """Opens the web-01 terminal from Connections when it is not in front."""
    if has("Session actions"):
        return
    to_tabs()
    nav("Connections")
    tap("web-01", index=-1)
    wait("Session actions", timeout=15)


def keyboard_off():
    """Hides the soft keyboard without Back (Back leaves the terminal)."""
    if "mInputShown=true" in sh("dumpsys input_method | grep mInputShown"):
        if has("Toggle keyboard"):
            tap("Toggle keyboard")
        else:
            hide_keyboard()


def host_list():
    to_tabs()
    nav("Vaults")
    if has("Keychain"):
        tap("Hosts")
    wait("Sort")


def vault_overview():
    to_tabs()
    nav("Vaults")
    for _ in range(3):
        if has("Keychain"):
            return
        tap("Back")
    wait("Keychain")


def text_field(index=0):
    fields = [n for n in nodes() if n.cls.endswith("EditText")]
    tap_xy(*fields[index].center)


@step("setup", keep=False)
def setup():
    sh(f"pm grant {PKG} android.permission.POST_NOTIFICATIONS")
    home()
    if has("Continue without sync"):
        tap("Continue without sync")
    time.sleep(1.5)
    for label in ("Allow", "Not now", "Skip", "Later"):
        if has(label):
            tap(label)
    # The pictures show Labterm's own colours, not the emulator wallpaper's.
    nav("Settings")
    if switch_state("Dynamic colors"):
        tap("Dynamic colors")
        time.sleep(1)


@step("android-quick-import")
def quick_import():
    host_list()
    tap("Add")
    tap("Import hosts")
    time.sleep(1)
    text_field(0)
    type_text(IMPORT)
    hide_keyboard()
    wait("Detected: ssh_config", exact=False)


@step("android-hosts")
def hosts():
    scroll_to(IMPORT_BUTTON)
    tap(IMPORT_BUTTON)
    wait("web-01", timeout=10)
    time.sleep(1)


@step("android-vault")
def vault():
    vault_overview()


@step("android-terminal")
def terminal():
    host_list()
    tap("web-01")
    if has("Connect"):
        tap("Connect")
    # Unknown host key: the test server's key is trusted here, once, like a person would.
    wait("Trust & save", timeout=40)
    tap("Trust & save")
    answer_password()
    time.sleep(4)
    open_session()
    for cmd in ("uptime", "ls -la", "df -h /", "tail -n 5 logs/app.log"):
        type_text(cmd + "\n")
        time.sleep(1.2)
    keyboard_off()


@step("android-session-menu")
def session_menu():
    open_session()
    tap("Session actions")
    wait("Close session")


@step("android-key-panel")
def key_panel():
    drive.back()
    time.sleep(0.5)
    tap("Show key panel")
    wait("Customize")


@step("android-hidden-input")
def hidden_input():
    tap("Password")
    wait("Hidden input")


@step("android-compose")
def compose():
    tap("Cancel")
    keyboard_off()
    if not has("Compose"):
        tap("More keys")
    tap("Compose")
    time.sleep(0.8)
    text_field(-1)
    type_text("Refactor the auth module: extract token validation into a helper and add unit tests for expired and malformed tokens.")
    wait("Paste only")


@step("android-connections")
def connections():
    to_tabs()
    nav("Connections")
    wait("Ways to connect", exact=False)


@step("android-sftp")
def sftp():
    nav("Connections")
    tap("Files")
    time.sleep(1)
    tap("web-01")
    time.sleep(2)
    if has("Save in vault"):
        answer_password()
    wait("logs", timeout=30)


@step("android-ai-agent")
def ai_agent():
    host_list()
    long_press(*wait("web-01")[0].center)
    tap("Edit", exact=False)
    scroll_to("AI coding agent", exact=False)
    tap("AI coding agent", exact=False)
    wait("Claude Code", exact=False)


@step("android-keys")
def keys():
    vault_overview()
    tap("Keychain")
    for label, algo in (("deploy", "Ed25519"), ("backup", "ECDSA"), ("ci-runner", "RSA")):
        tap("Add")
        tap("Generate key")
        fill("Label", label)
        hide_keyboard()
        if has("Algorithm"):
            tap("Algorithm")
            if has(algo, exact=False):
                tap(algo, exact=False)
        tap("Save")
        wait(label, timeout=30)


@step("android-pf-rule")
def pf_rule():
    vault_overview()
    tap("Port forwarding")
    tap("Add") if has("Add") else tap("Create a rule")
    if has("Local forwarding"):
        tap("Local forwarding")
    fill("Label", "Admin panel via web-01")
    hide_keyboard()
    if has("Host"):
        tap("Host")
        tap("web-01", index=-1)
    fill("Local port", "8080")
    fill("Destination host", "localhost")
    fill("Destination port", "8000")
    hide_keyboard()


@step("android-settings")
def settings():
    drive.back()
    if has("Discard", exact=False):
        tap("Discard", exact=False)
    to_tabs()
    nav("Settings")
    w, h = screen_size()
    for _ in range(3):
        swipe(w // 2, int(h * 0.3), w // 2, int(h * 0.8))
    wait("Appearance")


@step("android-gestures-settings")
def gestures():
    tap("Keyboard & gestures")
    scroll_to("Gestures", exact=False)


@step("android-linux-env")
def linux_env():
    back()
    tap("Linux environment")
    time.sleep(1.5)


@step("android-pin")
def pin():
    back()
    scroll_to("App PIN / pattern")
    tap("App PIN / pattern")
    time.sleep(1)
    if has("Set up"):
        tap("Set up")
    wait("Pattern")


@step("android-shade")
def shade():
    tap("Cancel")
    # Emulator notices (keyboard, serial console) are not Labterm's: snooze them.
    for line in sh("cmd notification list").splitlines():
        if line.startswith("0|android|") or "|com.android.systemui|" in line:
            sh(f"cmd notification snooze --for 3600000 '{line.strip()}'")
    sh("cmd statusbar expand-notifications")
    time.sleep(1.5)


def calm_status_bar():
    """System UI demo mode: 9:42, full battery and signal, no notification icons."""
    demo = "am broadcast -a com.android.systemui.demo -e command"
    sh(f"settings put global sysui_demo_allowed 1; {demo} enter; {demo} clock -e hhmm 0942; "
       f"{demo} battery -e level 100 -e plugged false; {demo} network -e wifi show -e level 4 -e fully true; "
       f"{demo} network -e mobile hide; {demo} notifications -e visible false")


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
