// End of the identity provider's pop-up: hand the redirect URL (code and state) to the Hatch tab
// that opened it, on the same origin only, then close. The tab checks state and PKCE.
(function () {
  if (window.opener && window.opener !== window) {
    window.opener.postMessage({ type: "hatch-oidc", url: location.href }, location.origin);
    window.close();
  }
})();
