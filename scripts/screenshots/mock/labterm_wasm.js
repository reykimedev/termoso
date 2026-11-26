// Stand-in for the cabinet's crypto wasm (web/src/crypto/pkg) in screenshots: "ciphertext" in the
// example data is plain base64, so pages that decrypt (recordings, vault names) show example text.
export default async function init() {}
const b64 = (s) => btoa(unescape(encodeURIComponent(s)));
const unb64 = (s) => decodeURIComponent(escape(atob(s)));
export const protocol_version = () => "1";
export const open_vault_key = () => "example-vault-key";
export const sealed_key_is_self = () => true;
export const decrypt_labeled = (_key, _labels, ct) => unb64(ct);
export const encrypt_labeled = (_key, _labels, pt) => b64(pt);
export const decrypt_field = (_key, ct) => unb64(ct);
export const encrypt_field = (_key, pt) => b64(pt);
export const decrypt_entity = (_key, ct) => unb64(ct);
export const encrypt_entity = (_key, pt) => b64(pt);
export const public_key_of = () => "ZXhhbXBsZS1wdWJsaWMta2V5";
export const generate_key_pair = () => ({ public_key: "ZXhhbXBsZS1wdWJsaWMta2V5", private_key: "ZXhhbXBsZQ==" });
export const generate_vault_key = () => "example-vault-key";
export const seal_vault_key = () => "ZXhhbXBsZQ==";
export const seal_vault_key_self = () => "ZXhhbXBsZQ==";
export const recovery_words = () =>
  "orbit lantern meadow copper violet harbor anchor willow summit pepper cobalt raven".split(" ");
export const recovery_verifier = () => "ZXhhbXBsZQ==";
export const create_account_keys = () => ({});
export const unlock_account = () => "ZXhhbXBsZQ==";
export const unlock_with_recovery = () => "ZXhhbXBsZQ==";
export const rewrap_private_key = () => "ZXhhbXBsZQ==";
export const rotate_recovery = () => ({});
export class OpaqueLogin { constructor() { this.request = ""; } finish() { return { free() {} }; } }
export class OpaqueRegistration { constructor() { this.request = ""; } finish() { return { free() {} }; } }
