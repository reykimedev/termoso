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
