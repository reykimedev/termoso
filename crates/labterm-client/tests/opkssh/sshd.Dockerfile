# OpenSSH host set up the way opkssh's install script sets one up:
# AuthorizedKeysCommand /usr/local/bin/opkssh verify %u %k %t.
FROM golang:1.27.4@sha256:205c7636d74e0ffbe3d007162df7d74f859f5a8c8322dfb9f907977686c126cf AS build
# opkssh v0.20.0
ARG OPKSSH_COMMIT=9dd6bfd7658df7b861d0f37ff3f34cd2da9cf3ed
WORKDIR /src
RUN git init -q opkssh && cd opkssh \
    && git fetch -q --depth 1 https://github.com/openpubkey/opkssh.git "$OPKSSH_COMMIT" \
    && git checkout -q FETCH_HEAD \
    && CGO_ENABLED=0 go build -o /opksshbuild . \
    && cp scripts/install-linux.sh /install-linux.sh

FROM ubuntu:24.04@sha256:562111b88caa79c71391c0c4d3c2114a7d5c35915c20561fd893cb1e07929e6b
RUN apt-get update \
    && apt-get install -y --no-install-recommends openssh-server sudo wget ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /run/sshd \
    && useradd -m -s /bin/bash test
COPY --from=build /opksshbuild /install-linux.sh /tmp/
RUN bash /tmp/install-linux.sh --install-from=/tmp/opksshbuild --no-sshd-restart \
    && rm -f /tmp/opksshbuild /tmp/install-linux.sh \
    # Trust only the test provider, and let its test user in as "test".
    # "24h": the PK token is good for a day after the ID token's iat.
    && echo "http://localhost:9998/ native 24h" > /etc/opk/providers \
    && opkssh add test test-user@zitadel.ch http://localhost:9998/ \
    && sed -i 's/^#\?Port .*/Port 2222/' /etc/ssh/sshd_config \
    && sed -i 's/^#\?PasswordAuthentication .*/PasswordAuthentication no/' /etc/ssh/sshd_config \
    && ssh-keygen -A
EXPOSE 2222
CMD ["/usr/sbin/sshd", "-D", "-e"]
