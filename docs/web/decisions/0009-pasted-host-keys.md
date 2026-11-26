# 0009 A host key may be pasted by hand

Amends [0005](0005-no-trust-on-first-use.md).

**Context:** 0005 lets a host key in only through Labterm's synced known hosts. Labterm's host
editor also lets the user paste a server key (from `ssh-keyscan` or the server's console) before
the first connection.

**Decision:** The browser keeps that field: a pasted public key is pinned and synced like any
other. What stays forbidden is pinning "the key the server shows now": in the browser that key
arrives through the relay, so trusting it would be trust on first use. The connection still
refuses an unknown or changed key before any credential is sent.

**Consequences:** A new host can be added from the browser alone, if the user can read its key
from somewhere other than the connection. The prompt for an unknown key offers only Close.
