#!/usr/bin/env python3
"""Rename the code from Termoso to Labterm. Safe to rerun.

    python3 scripts/rename-to-labterm.py

Branches started before a rename run this once on their own tree, commit, and
then merge main: the rename is mechanical, so their remaining conflicts are real.

Pass 1 (crates and app ids): the client crates become labterm-*; Rust, Kotlin and
Swift type and package names; the app ids become dev.labdex.labterm (Android
debug: dev.labdex.labterm.debug); the iOS project, targets and folders.

Pass 2 (everywhere): every other "termoso" in code, docs, deploy files and paths,
in all its spellings, including the server and bridge crates and the TERMOSO_*
settings (the code still reads the old TERMOSO_* names as a fallback).

What stays, on purpose (Labterm keeps talking to Termoso servers and keeps
reading existing data), listed in KEEP_EVERYWHERE below:
  - what a server, a vault or the disk already holds: X-Termoso-* headers, crypto
    labels (termoso/v1/..., HKDF salt, OPAQUE server id, FIDO2 "ssh:termoso"),
    the backup magic, the "termoso-core/<version>" user agent, metric names, the
    audit chain format, the data dir, keychain and Android keystore names, saved
    setting ids (termoso-dark/light) and browser storage keys;
  - the termoso://sso callback upstream servers allow; other termoso:// links
    still open (lines marked rename:keep), new links use labterm://;
  - Termoso's own services and repo (termoso.dev, github.com/termoso, Termoso
    Cloud), its package repo secrets, packaging/ (upstream's store manifests), SQL
    migrations (their checksums are recorded) and signed test fixtures;
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
    r"packaging/fdroid/com\.termoso\.android\.yml",  # upstream's F-Droid manifest stays
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
    # build outputs named after a crate: libtermoso_mobile.so/.a, termoso_wasm_bg.wasm
    s = s.replace("libtermoso_mobile", "liblabterm_mobile").replace("termoso_wasm_bg", "labterm_wasm_bg")
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


# Pass 2. Text that keeps its Termoso spelling, protected before the rename runs.
KEEP_EVERYWHERE = [
    r"[^\n]*rename:keep[^\n]*",  # a line marked by hand
    r'"labterm", "termoso"',  # both link schemes, where a comment can't go (JSON)
    r"termoso:\\/\\/sso",  # the sign-in callback in a regex
    r"[Xx]-[Tt]ermoso-[A-Za-z-]+",  # HTTP headers
    r"termoso/v\d+",  # crypto labels and AAD paths
    r"TERMOSO-BACKUP",
    r'"termoso-core/\{\}"',
    r"termoso-hkdf-salt-v\d",
    r'SERVER_ID: &\[u8\] = b"termoso"',
    r"ssh:termoso",  # FIDO2 application of existing security keys
    r"termoso-audit(?:-checkpoint-v\d)?",
    r"termoso-job:",
    r'redis_prefix: "termoso:"',
    r"termoso:captcha",
    r"termoso:audit",  # Splunk sourcetype in SIEM configs
    r"termoso-team-export/\d",  # team export format id
    r"reykimedev/termoso\b",  # this repository, until it moves
    r'android:scheme="termoso" android:host="sso"',
    r"[Tt]ermosoCaptcha",
    r"\btermoso_[a-z0-9_]+_(?:total|seconds|connections|bytes|count)\b",  # metric names
    r'"termoso_live_connections"|"termoso_ws_connections"',
    r'target: "termoso_audit"|\btermoso_audit\b',
    r'setup_scaffolding!\("termoso"\)',  # UniFFI namespace (generated file names)
    r'const SERVICE: &str = "termoso"',  # desktop keychain service
    r'\.join\("termoso"\)',  # desktop data dir
    r'appendingPathComponent\("Termoso", isDirectory: true\)',  # iOS data dir
    r"com\.termoso\.ios[A-Za-z.]*",  # iOS keychain
    r'"termoso\.master(?:\.auth)?"',  # Android keystore aliases
    r'\?: "termoso(?:-debug)?"',  # signing key alias defaults
    r"termoso-(?:dark|light)\b",  # saved terminal theme ids
    r'"termoso\.(?:theme|language|activeVault|teamSteps\.hidden|suggestPausedUntil|unlocked|device|session|sso\.next|session_ended)"',
    r"termoso://sso[A-Za-z/]*",  # the callback upstream servers accept
    r"TERMOSO_REPO_(?:DEPLOY|GPG)_KEY",  # GitHub secrets of upstream's package repo
    r"(?:[\w.+-]+@)?(?:[a-z0-9-]+\.)*termoso\.dev\b[\w./?=&%#-]*",  # Termoso's own sites, with their paths
    r"(?:github\.com|githubusercontent\.com|ghcr\.io|hosted\.weblate\.org/(?:projects|widget))/termoso\b[\w./-]*",
    r"termoso/(?:homebrew-termoso|termoso-repo)\b",
    r"Termoso Cloud",
    r"(?:fork of|based on|upstream,?|credit to|Termoso as upstream:?) Termoso\b|Termoso (?:contributors|authors|project|as (?:the )?upstream)",
    r"packaging/[\w./-]*termoso[\w./-]*|dev\.termoso\.Termoso[\w.]*|com\.termoso\.android\.yml",
]
KEEP_EVERYWHERE_RE = re.compile("|".join(KEEP_EVERYWHERE))
# Never edited in pass 2 (paths still move unless listed in NO_MOVE).
NO_EDIT = (
    "packaging/", "LICENSE", "NOTICE", "CHANGELOG", "THIRD_PARTY",
)
NO_EDIT_PARTS = ("/migrations/", "/fixtures/")
NO_MOVE = ("packaging/",)
WORD = re.compile(r"termoso|Termoso|TERMOSO")
SPELL = {"termoso": "labterm", "Termoso": "Labterm", "TERMOSO": "LABTERM"}


def everywhere_moves():
    for path in git("ls-files").splitlines():
        if path.startswith(NO_MOVE) or not WORD.search(path):
            continue
        move(path, WORD.sub(lambda m: SPELL[m.group(0)], path))


def everywhere_edit():
    changed = 0
    for path in git("ls-files").splitlines():
        if path == SELF or path.startswith(NO_EDIT) or any(p in f"/{path}" for p in NO_EDIT_PARTS):
            continue
        if any(part in SKIP_DIRS for part in path.split("/")):
            continue
        if not os.path.basename(path).startswith("Dockerfile") and os.path.splitext(path)[1] not in TEXT_EXT | {".tpl", ".container", ".volume", ".network", ".example",
                                                       ".desktop", ".env", ".sql", ".conf", ".service", ".rb",
                                                       ".svg", ".in", ".cfg", ".ini", ".lock", ".tmpl", ".py", ".patch"}:
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

        # this fork builds its own images (ci.yml docker job), which read LABTERM_* settings
        new = re.sub(r"ghcr\.io/(?:termoso|rep0rtdev)/termoso-", "ghcr.io/reykimedev/labterm-", text)
        new = new.replace("github.com/rep0rtDev/termoso", "github.com/reykimedev/termoso")
        new = KEEP_EVERYWHERE_RE.sub(protect, new)
        new = new.replace("com.termoso.android", APP_ID).replace("com.termoso", "dev.labdex.labterm")
        new = WORD.sub(lambda m: SPELL[m.group(0)], new)
        new = re.sub("\x00(\\d+)\x00", lambda m: kept[int(m.group(1))], new)
        if new != text:
            open(full, "w", encoding="utf-8").write(new)
            changed += 1
    print(f"pass 2: edited {changed} files")


if __name__ == "__main__":
    if git("status", "--porcelain").strip() and "--dirty-ok" not in sys.argv:
        sys.exit("commit or stash first (or pass --dirty-ok)")
    moves()
    edit_tree()
    everywhere_moves()
    everywhere_edit()
    print("now: cargo update -w --offline, then build and test")
