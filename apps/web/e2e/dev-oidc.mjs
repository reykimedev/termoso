// A stand-in OpenID Connect issuer for local tests: discovery, keys, and an authorize/token pair
// that signs in anyone without asking. Never run it anywhere but a test machine.
//   node web/e2e/dev-oidc.mjs [port]   ->  issuer http://localhost:<port>/
import { createServer } from "node:http";
import { generateKeyPairSync, sign, randomUUID, createHash } from "node:crypto";

const port = Number(process.argv[2] ?? 9400);
const issuer = `http://localhost:${port}/`;
const { publicKey, privateKey } = generateKeyPairSync("ed25519");
const jwk = { ...publicKey.export({ format: "jwk" }), kid: "dev", alg: "EdDSA", use: "sig" };
const b64 = (b) => Buffer.from(b).toString("base64url");
const codes = new Map();

export function idToken(claims) {
  const now = Math.floor(Date.now() / 1000);
  const body = { iss: issuer, aud: "hatch", sub: "dev-user", preferred_username: "dev", email: "dev@example.com",
    groups: ["ssh-users"], iat: now, exp: now + 600, ...claims };
  const head = b64(JSON.stringify({ alg: "EdDSA", typ: "JWT", kid: "dev" })) + "." + b64(JSON.stringify(body));
  return head + "." + b64(sign(null, Buffer.from(head), privateKey));
}

const cors = { "access-control-allow-origin": "*", "access-control-allow-headers": "content-type" };
createServer(async (req, res) => {
  const url = new URL(req.url, issuer);
  const json = (v, code = 200) => { res.writeHead(code, { "content-type": "application/json", ...cors }); res.end(JSON.stringify(v)); };
  if (req.method === "OPTIONS") { res.writeHead(204, cors); return res.end(); }
  if (url.pathname === "/.well-known/openid-configuration")
    return json({ issuer, jwks_uri: issuer + "jwks", authorization_endpoint: issuer + "authorize",
      token_endpoint: issuer + "token", response_types_supported: ["code"], code_challenge_methods_supported: ["S256"],
      id_token_signing_alg_values_supported: ["EdDSA"] });
  if (url.pathname === "/jwks") return json({ keys: [jwk] });
  if (url.pathname === "/authorize") {
    const code = randomUUID();
    codes.set(code, { nonce: url.searchParams.get("nonce"), challenge: url.searchParams.get("code_challenge") });
    const back = new URL(url.searchParams.get("redirect_uri"));
    back.searchParams.set("code", code);
    back.searchParams.set("state", url.searchParams.get("state") ?? "");
    res.writeHead(302, { location: back.toString() });
    return res.end();
  }
  if (url.pathname === "/token" && req.method === "POST") {
    let body = ""; for await (const c of req) body += c;
    const p = new URLSearchParams(body);
    const c = codes.get(p.get("code")); codes.delete(p.get("code"));
    const verifier = p.get("code_verifier") ?? "";
    if (!c || createHash("sha256").update(verifier).digest("base64url") !== c.challenge) return json({ error: "invalid_grant" }, 400);
    return json({ id_token: idToken(c.nonce ? { nonce: c.nonce } : {}), access_token: "dev", token_type: "Bearer", expires_in: 600 });
  }
  if (url.pathname === "/dev/token") return json({ id_token: idToken({}) });
  json({ error: "not_found" }, 404);
}).listen(port, () => console.log(`dev issuer on ${issuer}`));
