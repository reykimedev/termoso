"""Small adb driver for the Android pictures: find on-screen text through `uiautomator dump`,
tap it, type, take a screencap. Every failure prints what the screen shows, so the next run can
be fixed from the job log alone."""

import os
import re
import subprocess
import time
import xml.etree.ElementTree as ET

SERIAL = os.environ.get("ANDROID_SERIAL")


def adb(*args, text=True, check=False, timeout=60):
    cmd = ["adb"] + (["-s", SERIAL] if SERIAL else []) + list(args)
    r = subprocess.run(cmd, capture_output=True, text=text, timeout=timeout)
    if check and r.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd)}: {r.stderr}")
    return r.stdout


def sh(cmd):
    return adb("shell", cmd)


class Node:
    def __init__(self, el):
        self.text = el.get("text") or ""
        self.desc = el.get("content-desc") or ""
        self.cls = el.get("class") or ""
        self.res = el.get("resource-id") or ""
        self.clickable = el.get("clickable") == "true"
        self.focused = el.get("focused") == "true"
        x1, y1, x2, y2 = map(int, re.findall(r"\d+", el.get("bounds", "[0,0][0,0]")))
        self.box = (x1, y1, x2, y2)
        self.center = ((x1 + x2) // 2, (y1 + y2) // 2)

    def __repr__(self):
        return f"{self.text or self.desc!r}@{self.center}"


def nodes():
    for _ in range(6):
        out = adb("exec-out", "uiautomator", "dump", "/dev/tty")
        end = out.rfind("</hierarchy>")
        if end > 0:
            try:
                root = ET.fromstring(out[out.find("<?xml") if "<?xml" in out else out.find("<hierarchy"):end + 12])
                return [Node(el) for el in root.iter("node")]
            except ET.ParseError:
                pass
        time.sleep(0.5)
    return []


def labels():
    return [n.text or n.desc for n in nodes() if (n.text or n.desc)]


def find(label, exact=True, all_nodes=None):
    pat = label if isinstance(label, re.Pattern) else None
    out = []
    for n in all_nodes if all_nodes is not None else nodes():
        for s in (n.text, n.desc):
            if not s:
                continue
            if pat and pat.search(s) or (not pat and (s == label if exact else label.lower() in s.lower())):
                out.append(n)
                break
    return out


class NotFound(Exception):
    pass


def wait(label, timeout=20, exact=True, gone=False):
    end = time.time() + timeout
    while True:
        hits = find(label, exact)
        if bool(hits) != gone:
            return hits
        if time.time() > end:
            raise NotFound(f"{'still there' if gone else 'not found'}: {label!r}; screen shows: {labels()}")
        time.sleep(0.5)


def tap_xy(x, y):
    sh(f"input tap {x} {y}")
    time.sleep(0.6)


def tap(label, timeout=20, exact=True, index=0):
    hits = wait(label, timeout, exact)
    tap_xy(*hits[index].center)


def has(label, exact=True):
    return bool(find(label, exact))


def long_press(x, y, ms=900):
    sh(f"input swipe {x} {y} {x} {y} {ms}")
    time.sleep(0.6)


def type_text(s):
    """Types with the IME like a person (one `input text` per line, Enter between lines)."""
    for i, line in enumerate(s.split("\n")):
        if i:
            key("ENTER")
        if line:
            esc = line.replace("\\", "\\\\").replace("'", "'\\''").replace(" ", "%s")
            sh(f"input text '{esc}'")
    time.sleep(0.4)


def key(name):
    sh(f"input keyevent KEYCODE_{name}")
    time.sleep(0.3)


def back():
    key("BACK")
    time.sleep(0.4)


def hide_keyboard():
    if "mInputShown=true" in sh("dumpsys input_method | grep mInputShown"):
        back()


def screen_size():
    m = re.search(r"(\d+)x(\d+)", sh("wm size"))
    return int(m.group(1)), int(m.group(2))


def shot(path):
    time.sleep(0.8)
    data = adb("exec-out", "screencap", "-p", text=False)
    with open(path, "wb") as f:
        f.write(data)
