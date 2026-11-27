// Security keys in the browser. WebAuthn only signs for this site, so a key made here is an
// sk-ecdsa key whose application is this host name (its relying party id), not ssh-keygen's
// "ssh:". sshd (OpenSSH 8.3+) takes the browser's signature in its webauthn-sk format, and the
// desktop and Android apps sign with the same key over USB/NFC. Ed25519 has no WebAuthn format in
// OpenSSH, so keys made here are ECDSA P-256.
import { tr } from "@/i18n";
import type { PrfAnswer } from "./db";
import { fail } from "./errors";
import { keyCard, type SshKey } from "./model";
import { put, type Uuid } from "./state";
import { w } from "./wasm";

/** The pseudo-device the keychain lists for the browser's own security-key dialog. */
export const BROWSER_DEVICE = "webauthn://browser";

const TIMEOUT_MS = 120_000;

export const supported = () =>
  typeof window !== "undefined" && typeof window.PublicKeyCredential === "function";

export function devices() {
  if (!supported()) return [];
  return [
    {
      path: BROWSER_DEVICE,
      product: tr("This browser (security key or passkey)"),
      vendorId: 0,
      productId: 0,
      aaguid: null,
      // The browser asks for the PIN itself.
      pinSet: null,
      residentKeys: true,
      algorithms: ["ecdsa_p256"],
      versions: [],
    },
  ];
}

const hex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");

/** A WebAuthn error as the desktop's error, so the panel can show it. */
function failWith(e: unknown): never {
  const name = e instanceof DOMException ? e.name : "";
  if (name === "NotAllowedError") fail("fido2_denied", "Cancelled, or the security key timed out");
  if (name === "InvalidStateError")
    fail("fido2", "This security key already holds a key for this site");
  if (name === "SecurityError")
    fail("fido2", "This address cannot use security keys (it needs HTTPS and a host name)");
  fail("fido2", "Security key: {message}", { message: String(e) });
}

export async function generate(f: {
  vaultId: Uuid;
  label: string;
  resident: boolean;
  userVerification: boolean;
  comment: string;
  passphrase: string | null;
  rememberPassphrase: boolean;
}) {
  if (!supported()) fail("fido2_no_device", "This browser has no security key support");
  const rpId = location.hostname;
  const label = f.label.trim() || "Security key";
  let cred: PublicKeyCredential | null;
  try {
    cred = (await navigator.credentials.create({
      publicKey: {
        rp: { id: rpId, name: "Labterm" },
        user: { id: crypto.getRandomValues(new Uint8Array(16)), name: label, displayName: label },
        challenge: crypto.getRandomValues(new Uint8Array(32)),
        pubKeyCredParams: [{ type: "public-key", alg: -7 }],
        authenticatorSelection: {
          residentKey: f.resident ? "required" : "discouraged",
          requireResidentKey: f.resident,
          userVerification: f.userVerification ? "required" : "discouraged",
        },
        attestation: "none",
        timeout: TIMEOUT_MS,
      },
    })) as PublicKeyCredential | null;
  } catch (e) {
    failWith(e);
  }
  if (!cred) fail("fido2_denied", "Cancelled, or the security key timed out");
  const spki = (cred.response as AuthenticatorAttestationResponse).getPublicKey();
  if (!spki) fail("fido2", "The browser did not return the new key's public key");
  const rawId = new Uint8Array(cred.rawId);
  let g: { privateKey: string; publicKey: string; keyType: string };
  try {
    g = w.securityKeyFromWebauthn(
      new Uint8Array(spki),
      rawId,
      rpId,
      f.resident,
      f.userVerification,
      f.comment || label,
      f.passphrase,
    ) as typeof g;
  } catch (e) {
    fail("fido2", String(e));
  }
  const e = put<SshKey>("ssh_key", f.vaultId, null, {
    label,
    private_key: g.privateKey,
    public_key: g.publicKey,
    passphrase: f.rememberPassphrase ? f.passphrase : null,
    key_type: "sk-ecdsa",
    fido2_credential_id: hex(rawId),
  });
  return keyCard(e.id);
}

/** Whether a page at `host` may use relying party id `rpId` (the host itself or a parent domain). */
const allowedRpId = (host: string, rpId: string) => host === rpId || host.endsWith(`.${rpId}`);

/**
 * Signs for labterm-web-wasm (`prompt("webauthn", …)`): the SSH data is the WebAuthn challenge.
 * When the browser refuses without asking (no recent click, as Safari does after a slow connect),
 * `again()` shows a prompt whose button is the click it needs.
 */
export async function sign(
  req: {
    rpId: string;
    challenge: Uint8Array<ArrayBuffer>;
    credentialId: Uint8Array<ArrayBuffer>;
    userVerification: UserVerificationRequirement;
  },
  again: () => Promise<boolean>,
) {
  if (!supported()) return { error: tr("This browser has no security key support") };
  if (!allowedRpId(location.hostname, req.rpId))
    return {
      error: tr("This security key belongs to Labterm Web at {site}", { site: req.rpId }),
    };
  const options: CredentialRequestOptions = {
    publicKey: {
      challenge: req.challenge,
      rpId: req.rpId,
      allowCredentials: [{ type: "public-key", id: req.credentialId }],
      userVerification: req.userVerification,
      timeout: TIMEOUT_MS,
    },
  };
  let cred: PublicKeyCredential | null;
  try {
    cred = (await navigator.credentials.get(options)) as PublicKeyCredential | null;
  } catch (e) {
    if (!(e instanceof DOMException && e.name === "NotAllowedError") || !(await again()))
      return null;
    cred = (await navigator.credentials
      .get(options)
      .catch(() => null)) as PublicKeyCredential | null;
  }
  if (!cred) return null;
  const r = cred.response as AuthenticatorAssertionResponse;
  return {
    authenticatorData: new Uint8Array(r.authenticatorData),
    signature: new Uint8Array(r.signature),
    clientDataJSON: new Uint8Array(r.clientDataJSON),
  };
}

// ── unlocking this browser's copy with a passkey (decision 0014) ──────────────────────────────

const b64url = (b: Uint8Array) =>
  btoa(String.fromCharCode(...b))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
const unb64url = (s: string) =>
  Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0));
const unb64 = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));

/** The UV flag (bit 2) of WebAuthn authenticator data. */
const verified = (authData: ArrayBuffer) => ((new Uint8Array(authData)[32] ?? 0) & 0x04) !== 0;

interface PrfResults {
  prf?: { enabled?: boolean; results?: { first?: ArrayBuffer } };
}

/** Thrown when the browser or authenticator has no PRF: the panel says to use the password. */
function noPrf(): never {
  fail("fido2_no_prf", "This browser or key can't unlock Labterm Web. Use the lock password.");
}

/**
 * Asks any of `entries` for its PRF value (one touch). Non-resident credentials, no user
 * verification asked, so a YubiKey answers with a touch like on the desktop.
 */
export async function prfGet(
  entries: { credentialId: string; salt: string }[],
): Promise<PrfAnswer> {
  if (!supported()) fail("fido2_no_device", "This browser has no security key support");
  if (entries.length === 0) fail("fido2_wrong_device", "No passkey is set up in this browser");
  const evalByCredential: Record<string, { first: Uint8Array<ArrayBuffer> }> = {};
  for (const e of entries) evalByCredential[e.credentialId] = { first: unb64(e.salt) };
  let cred: PublicKeyCredential | null;
  try {
    cred = (await navigator.credentials.get({
      publicKey: {
        rpId: location.hostname,
        challenge: crypto.getRandomValues(new Uint8Array(32)),
        allowCredentials: entries.map((e) => ({
          type: "public-key" as const,
          id: unb64url(e.credentialId),
        })),
        userVerification: "discouraged",
        timeout: TIMEOUT_MS,
        extensions: { prf: { evalByCredential } },
      },
    })) as PublicKeyCredential | null;
  } catch (e) {
    if (e instanceof DOMException && e.name === "NotAllowedError")
      fail("cancelled", "Cancelled, or the security key timed out");
    failWith(e);
  }
  if (!cred) fail("cancelled", "Cancelled, or the security key timed out");
  const first = (cred.getClientExtensionResults() as PrfResults).prf?.results?.first;
  if (!first) noPrf();
  return {
    credentialId: b64url(new Uint8Array(cred.rawId)),
    prf: new Uint8Array(first),
    uv: verified((cred.response as AuthenticatorAssertionResponse).authenticatorData),
  };
}

/**
 * Makes a new unlock credential for this site, then reads its PRF value for `salt` with a second
 * touch. Not at create: a key with a PIN verifies the user there, and `hmac-secret` answers
 * differently with and without verification, so the value must come from the same kind of
 * request as unlock. Nothing is saved when PRF is missing.
 */
export async function prfCreate(name: string, salt: string, exclude: string[]): Promise<PrfAnswer> {
  if (!supported()) fail("fido2_no_device", "This browser has no security key support");
  let cred: PublicKeyCredential | null;
  try {
    cred = (await navigator.credentials.create({
      publicKey: {
        rp: { id: location.hostname, name: "Labterm" },
        // A user id per enrolment: a passkey manager replaces a passkey with the same user id,
        // which would leave the older wrapper unopenable.
        user: { id: crypto.getRandomValues(new Uint8Array(16)), name, displayName: name },
        challenge: crypto.getRandomValues(new Uint8Array(32)),
        pubKeyCredParams: [
          { type: "public-key", alg: -7 },
          { type: "public-key", alg: -8 },
          { type: "public-key", alg: -257 },
        ],
        excludeCredentials: exclude.map((id) => ({
          type: "public-key" as const,
          id: unb64url(id),
        })),
        authenticatorSelection: { residentKey: "discouraged", userVerification: "discouraged" },
        attestation: "none",
        timeout: TIMEOUT_MS,
        extensions: { prf: {} },
      },
    })) as PublicKeyCredential | null;
  } catch (e) {
    if (e instanceof DOMException && e.name === "NotAllowedError")
      fail("cancelled", "Cancelled, or the security key timed out");
    if (e instanceof DOMException && e.name === "InvalidStateError")
      fail("fido2", "This key already unlocks Labterm Web in this browser");
    failWith(e);
  }
  if (!cred) fail("cancelled", "Cancelled, or the security key timed out");
  const ext = (cred.getClientExtensionResults() as PrfResults).prf;
  if (!ext?.enabled) noPrf();
  return prfGet([{ credentialId: b64url(new Uint8Array(cred.rawId)), salt }]);
}
