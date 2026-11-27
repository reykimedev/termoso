# Screenshots

Retakes the pictures in [`docs/screenshots/`](../../docs/screenshots/) and the README's
[`.github/screens/`](../../.github/screens/). Nothing here talks to a real server or host: every
picture comes from a mocked backend with example data only (`example.com` names, `192.0.2.0/24`
addresses, made-up keys and fingerprints).

```sh
scripts/screenshots/take.sh            # desktop, Labterm Web and the web cabinet
scripts/screenshots/take.sh desktop    # or only some of them: desktop, web, cabinet
```

Then look at the result (`git diff --stat`, open the PNGs) and commit what changed.
Needs Node 22 and the Chromium that Playwright uses (`npx playwright install chromium` once).

## How it works

| Part | Shots | Backend |
| --- | --- | --- |
| Desktop | `desktop-*.png`, README `terminal`, `hosts`, `sftp` | Tauri's `mockIPC` answers every command from [`mock/`](mock/) |
| Labterm Web | `web-app-*.png` | the same mock, on the desktop frontend with `apps/web`'s patches and overlay |
| Web cabinet | `web-*.png` | Playwright answers every `/api/v1` call from [`mock/cabinet-api.mjs`](mock/cabinet-api.mjs) |
| Android | `android-*.png` | see `android.sh` |

- [`prepare.sh`](prepare.sh) copies `apps/desktop` (and for Labterm Web, applies `apps/web/patches`
  and `apps/web/overlay`) into `.work/`, and swaps the entry point for [`mock/entry.ts`](mock/entry.ts).
  `apps/` itself is never changed.
- [`mock/fixtures.ts`](mock/fixtures.ts) is the example vault: hosts, groups, keys, known hosts,
  snippets, rules, logs. [`mock/terminal.ts`](mock/terminal.ts) is a scripted shell, so terminal
  pictures look real: `web-02` shows an unknown host key, `db-01` asks for a password.
  A command the mock does not know is answered with `[]` or `null` and listed in the run's output
  as `unmocked`, which is the first place to look when a picture shows an empty list.
- [`cabinet.sh`](cabinet.sh) copies the server's web pages (`web/`) into `.work/cabinet` with a
  stand-in for their crypto wasm ([`mock/labterm_wasm.js`](mock/labterm_wasm.js): example
  "ciphertext" is base64, so recordings show example text). No server or database runs.
  `CABINET_SRC=<other checkout>/web` takes the pictures from another branch's `web/`.
- [`shots.mjs`](shots.mjs) opens each shot in a fresh page (1280×800, the cabinet 1440×900; dark, English, UTC, a fixed
  clock) and runs its steps from [`steps/`](steps/). One shot by name:
  `node shots.mjs http://localhost:5191/ out desktop-terminal` while `take.sh`'s server runs.

To add a picture: add its steps to `steps/desktop.mjs`, `steps/web.mjs` or `steps/cabinet.mjs` (named
like the file), any data it needs to `mock/fixtures.ts` or `mock/cabinet-api.mjs`, and a cell in [`docs/SCREENSHOTS.md`](../../docs/SCREENSHOTS.md).

## Android

The Android pictures need an emulator, so they are taken on a GitHub runner:
[`screenshots-android.yml`](../../.github/workflows/screenshots-android.yml) builds an x86_64
release APK (signed with a throwaway key), starts a Pixel 7 emulator (API 35) and the test SSH
server from [`android/sshd/`](android/sshd/), and runs [`android.sh`](android.sh). It runs by hand
(`workflow_dispatch`) and on pushes to `claude/**` that touch the Android scripts.

The pictures and the run log (`shots.log`) are pushed to the branch `<branch>-android-shots`,
nothing is committed to the branch that ran it. Look at them, then copy them over:

```sh
git fetch origin <branch>-android-shots
git archive FETCH_HEAD | tar -x -C /tmp/shots
cp /tmp/shots/android-*.png docs/screenshots/
```

- `android.sh` sends `192.0.2.0/24` ports 22 and 2222 to the test server (iptables in the
  emulator), switches to dark mode, turns animations off and installs the APK.
- [`android/shots.py`](android/shots.py) drives the app through `adb` and `uiautomator dump`
  ([`android/drive.py`](android/drive.py)), one step per picture, in order: hosts are imported
  first, then a session opens, and so on. A failed step saves `<name>.failed.png` and logs what the
  screen showed; the run goes on.
- The SSH server is Alpine with user `deploy` and password `example-password`. The app trusts its
  host key once, like a person would, inside the throwaway emulator.
- `android-linux-shell` downloads the Ubuntu root filesystem from Ubuntu's mirror.
