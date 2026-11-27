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
Host nas-01
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
    """Scrolls down, then back up, until `label` shows (slow swipes, so nothing flings past)."""
    w, h = screen_size()
    for down in (True, False):
        for _ in range(tries):
            if has(label, exact):
                return
            a, b = int(h * 0.7), int(h * 0.45)
            swipe(w // 2, a if down else b, w // 2, b if down else a, ms=900)
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
    typed = next(n.text for n in nodes() if n.cls.endswith("EditText"))
    if typed.split() != IMPORT.split():
        raise RuntimeError(f"import text came out as {typed!r}")
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
    wait("Duplicate")


@step("android-key-panel")
def key_panel():
    drive.back()
    time.sleep(0.5)
    tap("More keys")
    wait("Customize")


@step("android-hidden-input")
def hidden_input():
    tap("Password")
    wait("Hidden input")


@step("android-compose")
def compose():
    tap("Cancel")
    if has("Hide keys"):
        tap("Hide keys")
    # Compose sits at the far end of the key strip.
    w, _ = screen_size()
    y = wait("Paste")[0].center[1]
    for _ in range(6):
        if has("Compose"):
            break
        swipe(int(w * 0.85), y, int(w * 0.15), y)
    tap("Compose")
    time.sleep(0.8)
    text_field(-1)
    type_text("Refactor the auth module: extract token validation into a helper and add unit tests for expired and malformed tokens.")
    wait("Paste only")


@step("android-gesture-overlay")
def gesture_overlay():
    """A finger held mid-drag on the terminal: DOWN and MOVE now, UP in the next step."""
    drive.back()
    open_session()
    w, h = screen_size()
    x, y = w // 2, int(h * 0.35)
    sh(f"input motionevent DOWN {x} {y}")
    time.sleep(0.9)  # long-press first, then the drag sends arrows
    for dx in range(40, 361, 40):
        sh(f"input motionevent MOVE {x + dx} {y}")
    time.sleep(0.5)


@step("android-connections")
def connections():
    w, h = screen_size()
    sh(f"input motionevent UP {w // 2 + 360} {int(h * 0.35)}")
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
    row_y = wait("web-01")[0].center[1]
    menu = min(find("Host actions"), key=lambda n: abs(n.center[1] - row_y))
    tap_xy(*menu.center)
    scroll_to("Edit")
    tap("Edit")
    # The agent picker sits under the Mosh row; picker labels are not in the dump.
    scroll_to("Mosh")
    w, h = screen_size()
    y = wait("Mosh")[0].center[1]
    if y > h * 0.6:
        swipe(w // 2, int(h * 0.7), w // 2, int(h * 0.45), ms=900)
        y = wait("Mosh")[0].center[1]
    below = [n for n in nodes() if n.clickable and n.center[1] > y + 80]
    tap_xy(*min(below, key=lambda n: n.center[1]).center)
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
        # Saving opens the new key; the picture is the list.
        if has("Fingerprint"):
            tap("Back")
            wait("Add")


@step("android-pf-rule")
def pf_rule():
    vault_overview()
    tap("Port forwarding")
    tap("Add") if has("Add") else tap("Create a rule")
    if has("Skip wizard"):
        tap("Skip wizard")
    if has("Local forwarding"):
        tap("Local forwarding")
    # The Host picker sits between Label and "Listen on this device"; its label is not in the dump.
    w, _ = screen_size()
    host_y = (wait("Label")[0].center[1] + wait("Listen on this device")[0].center[1]) // 2
    for _ in range(3):
        tap_xy(w // 2, host_y)
        time.sleep(1)
        if has("web-02"):
            tap("web-01", index=-1)
            break
    fill("Label", "Admin panel via web-01")
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
    scroll_to("Long-press for arrows")
    w, h = screen_size()
    swipe(w // 2, int(h * 0.7), w // 2, int(h * 0.45))


@step("android-linux-env")
def linux_env():
    back()
    tap("Linux environment")
    time.sleep(1.5)


@step("android-linux-shell")
def linux_shell():
    if not has("Open a shell in it"):
        tap("Ubuntu 24.04 LTS")
        time.sleep(2)
        if has("Install"):
            tap("Install")
        wait("Open a shell in it", timeout=420)
    tap("Open a shell in it")
    wait("Session actions", timeout=60)
    time.sleep(3)
    for cmd in ("head -n 4 /etc/os-release", "uname -srm", "apt --version", "df -h / | tail -n 1"):
        type_text(cmd + "\n")
        time.sleep(1.5)


@step("android-pin")
def pin():
    to_tabs()
    nav("Settings")
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
    listed = sh("cmd notification list")
    print("notifications:", listed.strip().replace("\n", " ; "), flush=True)
    for line in listed.splitlines():
        if "|android|" in line or "|com.android.systemui|" in line:
            print("snooze", line.strip(), sh(f"cmd notification snooze --for 3600000 '{line.strip()}'").strip(), flush=True)
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
