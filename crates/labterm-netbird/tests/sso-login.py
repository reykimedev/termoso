#!/usr/bin/env python3
"""Plays the browser in the e2e sign-in test: signs in at the NetBird
server's embedded identity provider (Dex, local passwords) and follows the
redirect back to the app's localhost listener. Standard library only.

Usage: sso-login.py AUTH_URL EMAIL PASSWORD
"""
import http.cookiejar
import re
import sys
import urllib.parse
import urllib.request


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def main() -> None:
    auth_url, email, password = sys.argv[1:4]
    jar = http.cookiejar.CookieJar()
    browse = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
    page = browse.open(auth_url, timeout=20)
    html = page.read().decode()
    action = re.search(r'<form[^>]*method="post"[^>]*action="([^"]+)"', html)
    if not action:
        sys.exit(f"no login form at {page.geturl()}")
    login_url = urllib.parse.urljoin(page.geturl(), action.group(1).replace("&amp;", "&"))
    body = urllib.parse.urlencode({"login": email, "password": password}).encode()
    # Stop at the redirect to localhost to show where it goes, then follow it
    # like the browser would.
    post = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar), NoRedirect())
    try:
        post.open(login_url, data=body, timeout=20)
        sys.exit("login did not redirect (wrong password?)")
    except urllib.error.HTTPError as e:
        target = e.headers.get("Location", "")
    if not target.startswith("http://localhost:"):
        sys.exit(f"unexpected redirect: {target}")
    print(urllib.request.urlopen(target, timeout=60).read().decode())


if __name__ == "__main__":
    main()
