#!/usr/bin/env python3
"""Upload an Android App Bundle to a Google Play track.

    play-upload.py PACKAGE TRACK BUNDLE.aab [--status completed|draft] [--notes TEXT]

The service account JSON comes from the PLAY_SERVICE_ACCOUNT_JSON environment variable.
Standard library plus the `openssl` CLI (for the RS256 token signature) only: the job running
this holds the Play credentials, so it runs no third-party action or package.
"""

import argparse
import base64
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

API = "https://androidpublisher.googleapis.com/androidpublisher/v3/applications"
UPLOAD = "https://androidpublisher.googleapis.com/upload/androidpublisher/v3/applications"
SCOPE = "https://www.googleapis.com/auth/androidpublisher"


def b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def access_token(account: dict) -> str:
    now = int(time.time())
    header = b64url(json.dumps({"alg": "RS256", "typ": "JWT"}).encode())
    claims = b64url(json.dumps({
        "iss": account["client_email"],
        "scope": SCOPE,
        "aud": account["token_uri"],
        "iat": now,
        "exp": now + 600,
    }).encode())
    signing_input = f"{header}.{claims}".encode()
    with tempfile.NamedTemporaryFile("w", suffix=".pem") as key:
        os.chmod(key.name, 0o600)
        key.write(account["private_key"])
        key.flush()
        sig = subprocess.run(
            ["openssl", "dgst", "-sha256", "-sign", key.name],
            input=signing_input, capture_output=True, check=True,
        ).stdout
    body = urllib.parse.urlencode({
        "grant_type": "urn:ietf:params:oauth:grant-type:jwt-bearer",
        "assertion": f"{header}.{claims}.{b64url(sig)}",
    }).encode()
    return call("POST", account["token_uri"], body, "application/x-www-form-urlencoded")["access_token"]


def call(method: str, url: str, body: bytes | None = None, ctype: str = "application/json",
         token: str | None = None) -> dict:
    req = urllib.request.Request(url, data=body, method=method)
    if body is not None:
        req.add_header("Content-Type", ctype)
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=600) as res:
            raw = res.read()
    except urllib.error.HTTPError as e:
        # Google's error bodies name the problem (draft app, version code used, no access).
        sys.exit(f"{method} {url.split('?')[0]}: HTTP {e.code}\n{e.read().decode(errors='replace')}")
    return json.loads(raw) if raw else {}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("package")
    ap.add_argument("track")
    ap.add_argument("bundle")
    # A draft app (never rolled out by hand) only accepts draft releases.
    ap.add_argument("--status", default="completed", choices=["completed", "draft"])
    ap.add_argument("--notes", default="")
    args = ap.parse_args()

    account = json.loads(os.environ["PLAY_SERVICE_ACCOUNT_JSON"])
    token = access_token(account)
    app = f"{API}/{args.package}"

    edit = call("POST", f"{app}/edits", b"{}", token=token)["id"]
    with open(args.bundle, "rb") as f:
        bundle = call("POST", f"{UPLOAD}/{args.package}/edits/{edit}/bundles?uploadType=media",
                      f.read(), "application/octet-stream", token)
    code = str(bundle["versionCode"])
    print(f"uploaded {args.bundle}: version code {code}")

    release = {"versionCodes": [code], "status": args.status}
    if args.notes:
        release["releaseNotes"] = [{"language": "en-US", "text": args.notes[:500]}]
    track = {"track": args.track, "releases": [release]}
    call("PUT", f"{app}/edits/{edit}/tracks/{args.track}", json.dumps(track).encode(), token=token)
    call("POST", f"{app}/edits/{edit}:commit", b"", token=token)
    print(f"{args.package}: version code {code} on track {args.track} ({args.status})")


if __name__ == "__main__":
    main()
