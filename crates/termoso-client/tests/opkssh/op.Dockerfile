# Test OpenID provider: the zitadel/oidc example server from openpubkey's
# fork, the same one opkssh runs its own integration tests against.
# Real OIDC (auth code + PKCE S256, nonce, RS256 ID tokens); users and the
# "native" public client are built in. Not for anything but tests.
FROM golang:1.27.4@sha256:205c7636d74e0ffbe3d007162df7d74f859f5a8c8322dfb9f907977686c126cf AS build
# openpubkey/oidc, branch "test"
ARG OIDC_COMMIT=0d18e99505b2aad1577ddf266aed74d8de4e369c
WORKDIR /src
RUN git init -q oidc && cd oidc \
    && git fetch -q --depth 1 https://github.com/openpubkey/oidc.git "$OIDC_COMMIT" \
    && git checkout -q FETCH_HEAD \
    && CGO_ENABLED=0 go build -o /exampleop ./example/server/dynamic

FROM debian:bookworm-slim@sha256:e67eeead736ff7bbe258ef088bff683635c561b1fcfefad0806c1df4ab49ce0e
COPY --from=build /exampleop /usr/local/bin/exampleop
# Issuer http://localhost:9998/; redirect URI http://localhost:3000/login-callback
# (the first of opkssh's default redirect URIs).
ENV PORT=9998 REDIRECT_PORT=3000 AUTH_CALLBACK_PATH=/login-callback
USER 65534
CMD ["/usr/local/bin/exampleop"]
