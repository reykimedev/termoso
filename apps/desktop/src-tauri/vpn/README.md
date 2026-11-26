# Embedded VPN client

`build.sh` puts the embedded NetBird client (`crates/labterm-netbird/go`, a Go
c-shared library) here before the desktop app is bundled:

- Linux: `liblabterm_netbird.so`, bundled as a resource (`tauri.linux.conf.json`).
- macOS: `liblabterm_netbird.dylib`, bundled into `Contents/Frameworks` and
  signed with the app (`tauri.macos.conf.json`).

The app loads it with `dlopen` the first time a host uses a VPN network, so the
Go runtime only starts when it is needed. Without it, such hosts fail with
"NetBird is not available in this build". Windows has no build yet.

For `npm run tauri dev`, run `./build.sh` once; or point
`LABTERM_VPN_LIBRARY` at a library built elsewhere.
