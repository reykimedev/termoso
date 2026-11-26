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

from drive import (NotFound, back, find, has, hide_keyboard, key, labels, long_press, nodes,
                   screen_size, sh, shot, tap, tap_xy, type_text, wait)

PKG = os.environ.get("SHOTS_PACKAGE", "dev.labdex.labterm")
OUT = sys.argv[1]
ONLY = set(sys.argv[2:])
PASSWORD = "example-password"

IMPORT = """Host web-01
  HostName 192.0.2.11
  User deploy
Host db-01
  HostName 192.0.2.21
  User postgres
deploy@web-02.example.com
ops@bastion.example.com
admin@192.0.2.31:2222"""

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


def open_local_vault():
    nav("Vaults")
    if has("Local vault"):
        tap("Local vault")


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
    open_local_vault()
    if not has("Hosts"):
        raise NotFound(f"no Hosts entry; screen shows: {labels()}")
    tap("Hosts")
    tap("Add")
    tap("Import hosts")
    time.sleep(1)
    # The paste field is the first editable field of the sheet.
    fields = [n for n in nodes() if n.cls.endswith("EditText")]
    if fields:
        tap_xy(*fields[0].center)
    type_text(IMPORT)
    hide_keyboard()
    wait("Import", exact=False)


@step("android-hosts")
def hosts():
    tap(__import__("re").compile(r"^Import \d+ hosts?$"))
    time.sleep(1.5)


@step("android-vault")
def vault():
    back()
    time.sleep(0.8)


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
    time.sleep(3)
    for cmd in ("uptime", "ls -la", "df -h /", "tail -n 5 logs/app.log"):
        type_text(cmd + "\n")
        time.sleep(1)
    hide_keyboard()


@step("android-connections")
def connections():
    back()
    nav("Connections")


def main():
    os.makedirs(OUT, exist_ok=True)
    w, h = screen_size()
    print(f"screen {w}x{h}")
    for name, fn, keep in STEPS:
        try:
            fn()
            if keep and (not ONLY or name in ONLY):
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
