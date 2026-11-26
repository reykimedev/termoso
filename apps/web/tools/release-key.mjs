#!/usr/bin/env node
// Makes a new release signing key: writes the private key (PEM, mode 600) to <file> and prints
// the entry to add to tools/release-keys.json. Run it where the key will live (the release
// environment), not on a shared machine.
//   node tools/release-key.mjs <file> <id>
import { generateKeyPairSync } from "node:crypto";
import { writeFileSync } from "node:fs";

const [file, id] = process.argv.slice(2);
if (!file || !id) {
  console.error("usage: release-key.mjs <private-key-file> <id, e.g. hatch-release-2026>");
  process.exit(2);
}
const { publicKey, privateKey } = generateKeyPairSync("ed25519");
writeFileSync(file, privateKey.export({ format: "pem", type: "pkcs8" }), { mode: 0o600, flag: "wx" });
const raw = publicKey.export({ format: "der", type: "spki" }).subarray(-32).toString("base64");
console.log(JSON.stringify({ id, publicKey: raw }));
