// SMTP sink for local tests: accepts any mail and appends it to a file (no TLS, no auth).
//   node web/e2e/smtp-sink.mjs 1025 /tmp/mail.log
import { createServer } from "node:net";
import { appendFileSync } from "node:fs";

const [port = 1025, file = "mail.log"] = process.argv.slice(2);
createServer((s) => {
  let data = false, buf = "", mail = "";
  const say = (l) => s.write(l + "\r\n");
  say("220 sink");
  s.on("data", (d) => {
    buf += d.toString();
    let i;
    while ((i = buf.indexOf("\r\n")) >= 0) {
      const line = buf.slice(0, i); buf = buf.slice(i + 2);
      if (data) {
        if (line === ".") { data = false; appendFileSync(file, mail + "\n=====\n"); mail = ""; say("250 ok"); }
        else mail += line + "\n";
        continue;
      }
      const cmd = line.slice(0, 4).toUpperCase();
      if (cmd === "EHLO" || cmd === "HELO") say("250 sink");
      else if (cmd === "DATA") { data = true; say("354 go"); }
      else if (cmd === "QUIT") { say("221 bye"); s.end(); }
      else say("250 ok");
    }
  });
}).listen(Number(port), "127.0.0.1");
