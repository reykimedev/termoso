# 0001 The UI is Termoso's desktop frontend

**Context:** The handbook's stack standard ([0001](https://github.com/bitsandbricks-ch/handbook/blob/main/decisions/0001-stack-standard.md))
is React, TypeScript, Vite and Mantine with `@bitsandbricks/ui`. A first mockup on that stack
didn't feel like Termoso, and people move between Hatch and the Termoso apps. Termoso's desktop
frontend (React, MUI, TanStack Query, xterm.js) talks to its backend only through Tauri `invoke`
and events, so it runs in a browser once those calls are served by a wasm backend: the mockup
needed 56 changed lines in 4 files.

**Decision:** Hatch ships Termoso's desktop frontend at a pinned release tag, with a small
patch set behind an `IS_WEB` flag and web-only screens in `web/overlay/`. It doesn't use Mantine
or `@bitsandbricks/ui`. The rest of the stack standard holds: React, TypeScript, Vite.

**Consequences:** Hatch looks and works like Termoso, and gets its screens and translations for
free. Each Termoso release can add IPC commands the web backend must serve or stub, so moving
the pin is its own reviewed PR. The patches stay small and are offered upstream. Hatch follows
Termoso's UI rules rather than the BitsAndBricks brand guide.
