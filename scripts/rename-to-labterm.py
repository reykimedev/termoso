#!/usr/bin/env python3
"""Rename the client code from Termoso to Labterm. Safe to rerun.

    python3 scripts/rename-to-labterm.py

Branches started before the rename run this once on their own tree, commit, and
then merge main: the rename is mechanical, so their remaining conflicts are real.

What changes: the client crates (termoso-{core,client,crypto,proto,mobile,wasm,oidc-cert,
awslc-rsa} and termoso-desktop) become labterm-*; Rust, Kotlin and Swift type and
package names; the app ids become dev.labdex.labterm (Android debug:
dev.labdex.labterm.debug); the iOS project, targets and folders.

What stays, on purpose (Labterm must keep talking to Termoso servers and keep
reading existing data):
  - every string the server, the vault or the disk sees: HTTP headers, API paths,
    metrics, TERMOSO_* variables, crypto context strings, the backup magic, the
    "termoso-core/<version>" user agent, the data dir <data>/termoso and the
    keychain service "termoso" (desktop) / com.termoso.ios.* (iOS);
  - the termoso:// link scheme and the TermosoCaptcha bridge the server page calls;
  - the Termoso server and its relay: crates/termoso-server, crates/termoso-bridge,
    deploy/, packaging/ (they only follow the renamed crate names they depend on);
  - AGPL notices and credit to Termoso as upstream.
"""
import os
import re
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
APP_ID = "dev.labdex.labterm"
CRATES = ["core", "client", "crypto", "proto", "mobile", "wasm", "awslc-rsa", "oidc-cert"]

# Types and identifiers; TermosoCaptcha stays (the server's captcha page calls it).
IDENTS = {
    "TermosoCoreFFI": "LabtermCoreFFI",
    "TermosoCore": "LabtermCore",
    "TermosoApp": "LabtermApp",
    "TermosoSwitch": "LabtermSwitch",
    "TermosoApplication": "LabtermApplication",
    "TermosoUITests": "LabtermUITests",
    "TermosoTests": "LabtermTests",
    "TermosoTheme": "LabtermTheme",
    "TermosoRoot": "LabtermRoot",
    "TermosoTypography": "LabtermTypography",
    "TermosoIOSApp": "LabtermIOSApp",
    "TermosoException": "LabtermException",
    "termosoSwitchColors": "labtermSwitchColors",
    "termosoDark": "labtermDark",
    "termosoLight": "labtermLight",
}

# Text that must never change, protected before the rules run.
KEEP = [
    r'"termoso-core/\{\}"',  # user agent the server sees
    r"TermosoCaptcha",
    r"termosoCaptcha",
]

SKIP_DIRS = {".git", "node_modules", "target", "dist", "build", ".gradle", "pkg", "fuzz/target"}
TEXT_EXT = {
    ".rs", ".toml", ".ts", ".tsx", ".js", ".mjs", ".cjs", ".json", ".kt", ".kts", ".swift", ".yml",
    ".yaml", ".md", ".sh", ".xml", ".html", ".css", ".h", ".modulemap", ".pro", ".properties", ".txt",
    ".plist", ".entitlements", ".gitignore", "",
}
# Upstream's server side and store manifests: only crate references change there.
SERVER_ONLY = ("crates/termoso-server/", "crates/termoso-bridge/", "deploy/", "packaging/")
SELF = "scripts/rename-to-labterm.py"


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, check=True, capture_output=True, text=True).stdout


def move(src, dst):
    if os.path.exists(os.path.join(ROOT, src)) and not os.path.exists(os.path.join(ROOT, dst)):
        os.makedirs(os.path.dirname(os.path.join(ROOT, dst)) or ROOT, exist_ok=True)
        git("mv", src, dst)
        print(f"moved {src} -> {dst}")


def moves():
    for c in CRATES:
        move(f"crates/termoso-{c}", f"crates/labterm-{c}")
    kt = APP_ID.replace(".", "/")
    for sset in ("main", "test", "androidTest", "debug"):
        move(f"apps/android/app/src/{sset}/kotlin/com/termoso/android", f"apps/android/app/src/{sset}/kotlin/{kt}")
    for d in ("Termoso", "TermosoCore", "TermosoTests", "TermosoUITests"):
        move(f"apps/ios/{d}", f"apps/ios/{d.replace('Termoso', 'Labterm')}")
    # files named after a renamed type
    for path in git("ls-files").splitlines():
        base = os.path.basename(path)
        for old, new in IDENTS.items():
            if base.startswith(old + ".") or base == old:
                move(path, os.path.join(os.path.dirname(path), base.replace(old, new, 1)))
                break


crate_alt = "|".join(re.escape(c) for c in CRATES)
CRATE_HYPHEN = re.compile(rf"\btermoso-({crate_alt}|desktop)\b")
CRATE_SNAKE = re.compile(rf"\btermoso_({'|'.join(c.replace('-', '_') for c in CRATES)}|desktop_lib|desktop)\b")
IDENT_RE = re.compile(r"\b(" + "|".join(sorted(IDENTS, key=len, reverse=True)) + r")\b")
KEEP_RE = re.compile("|".join(KEEP))


def rules(path, s):
    server_only = path.startswith(SERVER_ONLY)
    s = CRATE_HYPHEN.sub(lambda m: f"labterm-{m.group(1)}", s)
    s = CRATE_SNAKE.sub(lambda m: f"labterm_{m.group(1)}", s)
    if server_only:
        return s
    s = IDENT_RE.sub(lambda m: IDENTS[m.group(1)], s)
    # Kotlin/Android packages and app ids
    s = s.replace("com.termoso.android", APP_ID).replace("com/termoso/android", APP_ID.replace(".", "/"))
    s = s.replace("com.termoso.core", f"{APP_ID}.core")
    s = s.replace('"identifier": "com.termoso.desktop"', f'"identifier": "{APP_ID}"')
    if path.startswith("apps/ios/project.yml") or path.startswith("apps/ios/altstore/"):
        s = s.replace("bundleIdPrefix: com.termoso", "bundleIdPrefix: dev.labdex")
        s = re.sub(r"\bcom\.termoso\.ios\b", APP_ID, s)
        s = s.replace("com.termoso.altstore", f"{APP_ID}.altstore")
    # the iOS project, its targets and folders
    if path.startswith(("apps/ios/", ".github/workflows/", "scripts/", "docs/IOS")) or path == "README.md":
        s = re.sub(r"\bTermoso(\.xcodeproj|\.xcarchive|\.xcresult|\.app)\b", r"Labterm\1", s)
        s = re.sub(r"-scheme Termoso\b", "-scheme Labterm", s)
        s = re.sub(r"apps/ios/Termoso(Core|Tests|UITests)?\b", lambda m: f"apps/ios/Labterm{m.group(1) or ''}", s)
        s = s.replace('"$app/Termoso"', '"$app/Labterm"')
    if path == "apps/ios/project.yml":
        s = re.sub(r"(?m)^name: Termoso$", "name: Labterm", s)
        s = re.sub(r"(?m)^(\s+)Termoso:$", r"\1Labterm:", s)
        s = re.sub(r"(?m)^(\s+- (?:path|target): )Termoso\b", r"\1Labterm", s)
        s = re.sub(r"(?m)^(\s+path: )Termoso/", r"\1Labterm/", s)
        s = re.sub(r"(?m)^(\s+)Termoso: all$", r"\1Labterm: all", s)
        s = s.replace("CFBundleName: Termoso", "CFBundleName: Labterm")
    if path.startswith("apps/ios/"):
        s = re.sub(r"(?m)^@testable import Termoso$", "@testable import Labterm", s)
    s = s.replace("Theme.Termoso", "Theme.Labterm")
    return s


def edit_tree():
    changed = 0
    for path in git("ls-files").splitlines():
        if path == SELF or any(part in SKIP_DIRS for part in path.split("/")):
            continue
        ext = os.path.splitext(path)[1]
        if ext not in TEXT_EXT:
            continue
        full = os.path.join(ROOT, path)
        try:
            text = open(full, encoding="utf-8").read()
        except (UnicodeDecodeError, FileNotFoundError, IsADirectoryError):
            continue
        kept = []

        def protect(m):
            kept.append(m.group(0))
            return f"\x00{len(kept) - 1}\x00"

        new = rules(path, KEEP_RE.sub(protect, text))
        new = re.sub("\x00(\\d+)\x00", lambda m: kept[int(m.group(1))], new)
        if new != text:
            open(full, "w", encoding="utf-8").write(new)
            changed += 1
    print(f"edited {changed} files")


if __name__ == "__main__":
    if git("status", "--porcelain").strip() and "--dirty-ok" not in sys.argv:
        sys.exit("commit or stash first (or pass --dirty-ok)")
    moves()
    edit_tree()
    print("now: cargo update -w --offline, then build and test")
