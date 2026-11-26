#!/usr/bin/env node
// Signs <dist>/manifest.json with the release key (Ed25519), writing <dist>/manifest.sig.
//   HATCH_RELEASE_KEY_FILE=release.pem node tools/sign-manifest.mjs <dist>
// The key comes from a file (the release environment's secret); it is never printed. The
// signature is checked against tools/release-keys.json before it is written.
import { createPrivateKey, createPublicKey, sign } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const dist = process.argv[2];
const keyFile = process.env.HATCH_RELEASE_KEY_FILE;
if (!dist || !keyFile) {
  console.error("usage: HATCH_RELEASE_KEY_FILE=<pem> sign-manifest.mjs <dist>");
  process.exit(2);
}
const key = createPrivateKey(readFileSync(keyFile));
if (key.asymmetricKeyType !== "ed25519") throw new Error("the release key must be Ed25519");
const pub = createPublicKey(key).export({ format: "der", type: "spki" }).subarray(-32).toString("base64");
const here = dirname(fileURLToPath(import.meta.url));
const known = JSON.parse(readFileSync(join(here, "release-keys.json"), "utf8"));
const entry = known.find((k) => k.publicKey === pub);
if (!entry) throw new Error("this key is not in tools/release-keys.json; the loader would refuse the signature");
const manifest = readFileSync(join(dist, "manifest.json"));
writeFileSync(join(dist, "manifest.sig"), sign(null, manifest, key).toString("base64") + "\n");
console.log(`signed manifest.json with ${entry.id}`);
