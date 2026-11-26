# 0005 Host keys only from Labterm's known hosts

**Context:** The relay sits between the browser and every host. If the browser accepted an
unknown host key, the relay (or anyone who controls it) could answer in the host's place and
collect a password or a signature.

**Decision:** The browser accepts a host key only if it is in the user's Labterm known hosts,
synced from their other devices. An unknown or changed key stops the connection before any
credential is sent, with no way to accept it in the browser.

**Consequences:** A new host needs its key added from a trusted device first. An export of host
keys from an inventory (for example labdex's agent reports) into the Labterm trust store would
make this easier; it stays a source of facts, never a certificate authority.
