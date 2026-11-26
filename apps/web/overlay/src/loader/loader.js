// Hatch loader: the only script index.html runs. It checks the release manifest's Ed25519
// signature against the keys built in below, keeps the release from going backwards, installs the
// service worker that checks every file, and only then starts the app, with SRI on the entry
// script and stylesheets (index.html carries the import map with the hashes of every module).
// Plain JavaScript on purpose: no bundler between this file and what the browser runs.
(function () {
  "use strict";
  /** Release signing keys: [{ id, publicKey (base64, 32 bytes) }]. Filled in by tools/manifest.mjs. */
  var KEYS = /*HATCH_RELEASE_KEYS*/ []; /*END*/

  function b64(bytes) {
    var s = "";
    for (var i = 0; i < bytes.length; i += 0x8000)
      s += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
    return btoa(s);
  }
  function unb64(s) {
    return Uint8Array.from(atob(s), function (c) {
      return c.charCodeAt(0);
    });
  }
  function sha256(bytes) {
    return crypto.subtle.digest("SHA-256", bytes).then(function (h) {
      return b64(new Uint8Array(h));
    });
  }
  function get(path, network) {
    // "?network" asks the service worker to fetch past its verified cache.
    return fetch(path + (network ? "?network" : ""), {
      cache: "no-store",
      credentials: "same-origin",
    });
  }

  /** Resolves with the id of the key that signed `data`, rejects when none did. */
  function verify(data, sig) {
    var i = 0;
    function next() {
      if (i >= KEYS.length)
        return Promise.reject(
          new Error("The release manifest is not signed by a Hatch release key."),
        );
      var k = KEYS[i++];
      return crypto.subtle
        .importKey("raw", unb64(k.publicKey), { name: "Ed25519" }, false, ["verify"])
        .then(function (key) {
          return crypto.subtle.verify({ name: "Ed25519" }, key, sig, data);
        })
        .then(function (ok) {
          return ok ? k.id : next();
        });
    }
    return next();
  }

  /** The manifest as served (cached by the service worker once it runs), checked. */
  function loadManifest(network) {
    return Promise.all([get("manifest.json", network), get("manifest.sig", network)]).then(
      function (r) {
        if (!r[0].ok) throw new Error("manifest.json is missing (HTTP " + r[0].status + ").");
        return Promise.all([r[0].arrayBuffer(), r[1].ok ? r[1].text() : null]).then(function (x) {
          var bytes = new Uint8Array(x[0]);
          var signed = KEYS.length > 0;
          var check = signed
            ? x[1]
              ? verify(bytes, unb64(x[1].trim()))
              : Promise.reject(new Error("manifest.sig is missing."))
            : Promise.resolve(null);
          return Promise.all([check, sha256(bytes)]).then(function (y) {
            return { m: JSON.parse(new TextDecoder().decode(bytes)), keyId: y[0], hash: y[1] };
          });
        });
      },
    );
  }

  /** Compares sw.js on the server with the signed one; a release in progress is not tampering. */
  function workerChanged(m) {
    if (!("serviceWorker" in navigator) || !m.files["sw.js"]) return Promise.resolve(false);
    return get("sw.js", true)
      .then(function (r) {
        return r.arrayBuffer();
      })
      .then(function (buf) {
        return sha256(new Uint8Array(buf));
      })
      .then(function (h) {
        if (h === m.files["sw.js"].sha256) return false;
        return loadManifest(true).then(
          function (n) {
            return !(
              n.m.serial >= m.serial &&
              n.m.files["sw.js"] &&
              n.m.files["sw.js"].sha256 === h
            );
          },
          function () {
            return true;
          },
        );
      })
      .catch(function () {
        return false;
      });
  }

  function fail(message) {
    var root = document.getElementById("root");
    root.textContent = "";
    var box = document.createElement("div");
    box.setAttribute("role", "alert");
    box.style.cssText =
      "max-width:520px;margin:15vh auto;padding:24px;font:15px/1.5 system-ui,sans-serif;border:2px solid #c62828;border-radius:12px;color:#c62828;background:#fff";
    var h = document.createElement("h1");
    h.style.cssText = "font-size:20px;margin:0 0 8px";
    h.textContent = "Hatch did not start";
    var p = document.createElement("p");
    p.textContent =
      message +
      " This copy of Hatch failed its integrity check, so none of it ran. Do not enter a password here; tell whoever runs this Hatch.";
    box.appendChild(h);
    box.appendChild(p);
    root.appendChild(box);
  }

  function start(c) {
    var m = c.m;
    var last = 0;
    try {
      last = Number(localStorage.getItem("hatch.serial") || 0);
    } catch (e) {
      /* storage blocked */
    }
    if (c.keyId && m.serial < last)
      throw new Error(
        "The server offers release " + m.version + ", older than one this browser already ran.",
      );
    try {
      if (c.keyId) localStorage.setItem("hatch.serial", String(m.serial));
    } catch (e) {
      /* storage blocked */
    }
    return workerChanged(m).then(function (changed) {
      window.__hatchIntegrity = {
        status: c.keyId ? "verified" : "unsigned",
        version: m.version,
        commit: m.commit,
        termoso: m.termoso,
        keyId: c.keyId,
        manifestHash: c.hash,
        files: Object.keys(m.files)
          .sort()
          .map(function (path) {
            return { path: path, size: m.files[path].size, sha256: m.files[path].sha256 };
          }),
        workerChanged: changed,
      };
      if ("serviceWorker" in navigator)
        navigator.serviceWorker.register("sw.js", { updateViaCache: "none" }).catch(function () {});
      m.entry.styles.forEach(function (path) {
        var l = document.createElement("link");
        l.rel = "stylesheet";
        l.href = path;
        l.integrity = "sha256-" + m.files[path].sha256;
        l.crossOrigin = "anonymous";
        document.head.appendChild(l);
      });
      var s = document.createElement("script");
      s.type = "module";
      s.src = m.entry.script;
      s.integrity = "sha256-" + m.files[m.entry.script].sha256;
      s.crossOrigin = "anonymous";
      s.onerror = function () {
        fail("The app's code did not match the signed manifest.");
      };
      document.head.appendChild(s);
    });
  }

  if (!window.crypto || !crypto.subtle) {
    fail("This browser cannot check signatures here (a secure https:// page is needed).");
    return;
  }
  loadManifest(false)
    .then(start)
    .catch(function (e) {
      fail(e && e.message ? e.message : String(e));
    });
})();
