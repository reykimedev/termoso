# The SSH server the Android pictures connect to: one container answers for every example
# host (the emulator sends 192.0.2.0/24 port 22 here). Example data only.
FROM alpine:3.22
RUN apk add --no-cache bash openssh openssh-sftp-server coreutils procps tzdata \
 && ssh-keygen -A \
 && adduser -D -s /bin/bash deploy \
 && echo "deploy:example-password" | chpasswd \
 && sed -i -e 's/^#\?PasswordAuthentication.*/PasswordAuthentication yes/' \
           -e 's/^#\?PrintMotd.*/PrintMotd yes/' /etc/ssh/sshd_config
COPY home/ /home/deploy/
COPY motd /etc/motd
RUN chown -R deploy:deploy /home/deploy && touch -d "2026-11-24 15:20" /home/deploy/* /home/deploy/*/*
ENV TZ=UTC
EXPOSE 22
CMD ["/usr/sbin/sshd", "-D", "-e"]
