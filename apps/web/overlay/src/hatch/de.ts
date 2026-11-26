// German (Swiss spelling, no ß) for Hatch's own text: English source → translation, in the format of
// Labterm's ru table. Labterm's own UI text falls back to English until it has a German table.
export const de: Readonly<Record<string, string>> = {
  // phone navigation (also Labterm's own labels)
  Hosts: "Hosts",
  Keychain: "Schlüsselbund",
  "Known hosts": "Bekannte Hosts",
  Settings: "Einstellungen",
  More: "Mehr",

  // first start, lock screen
  "Set a lock password": "Sperrpasswort festlegen",
  "It encrypts this browser's copy of your vault and unlocks Labterm Web on this computer. It is not your Labterm account password and never leaves this browser.":
    "Es verschlüsselt die Kopie deines Tresors in diesem Browser und entsperrt Labterm Web auf diesem Computer. Es ist nicht das Passwort deines Labterm-Kontos und verlässt diesen Browser nie.",
  "Lock password": "Sperrpasswort",
  "Repeat the lock password": "Sperrpasswort wiederholen",
  "Use at least {count} characters": "Mindestens {count} Zeichen verwenden",
  "The passwords differ": "Die Passwörter stimmen nicht überein",
  "Setting up…": "Wird eingerichtet…",
  Continue: "Weiter",
  "Next: sign in to your Labterm account (the person icon at the top) to sync your hosts and keys.":
    "Als Nächstes: Beim Labterm-Konto anmelden (Personensymbol oben), um Hosts und Schlüssel zu synchronisieren.",
  "Labterm Web cannot start": "Labterm Web kann nicht starten",
  "Forgot it? Reset this browser and sign in to your Labterm account again.":
    "Vergessen? Diesen Browser zurücksetzen und erneut beim Labterm-Konto anmelden.",
  "Reset this browser": "Diesen Browser zurücksetzen",
  "Reset this browser?": "Diesen Browser zurücksetzen?",
  "Labterm Web forgets this browser's copy of your vault. Everything synced to your Labterm account comes back when you sign in again; entries kept only in this browser are lost.":
    "Labterm Web vergisst die Kopie deines Tresors in diesem Browser. Alles, was mit deinem Labterm-Konto synchronisiert ist, kommt bei der nächsten Anmeldung zurück; Einträge, die nur in diesem Browser liegen, gehen verloren.",
  Reset: "Zurücksetzen",
  Cancel: "Abbrechen",
  Close: "Schliessen",
  Lock: "Sperren",
  "Lock: close sessions and forget the vault key":
    "Sperren: Sitzungen schliessen und den Tresorschlüssel vergessen",
  "Wrong password": "Falsches Passwort",

  // build and integrity
  "Build and integrity": "Build und Integrität",
  "Signed release, every file checked": "Signiertes Release, jede Datei geprüft",
  "The service worker on the server differs from the signed release":
    "Der Service Worker auf dem Server weicht vom signierten Release ab",
  "Unsigned development build": "Unsignierter Entwicklungs-Build",
  "sw.js on the server is not the one in the signed manifest. The browser updates the service worker on its own, so Labterm Web can only warn. Lock now and compare the manifest hash with the release page before you unlock again.":
    "sw.js auf dem Server ist nicht die Datei aus dem signierten Manifest. Der Browser aktualisiert den Service Worker selbst, deshalb kann Labterm Web nur warnen. Jetzt sperren und den Manifest-Hash mit der Release-Seite vergleichen, bevor du wieder entsperrst.",
  "Every file matched the manifest signed for {version} before the app started.":
    "Jede Datei stimmte mit dem für {version} signierten Manifest überein, bevor die App startete.",
  "This build has no signed manifest. Use it for development only.":
    "Dieser Build hat kein signiertes Manifest. Nur für die Entwicklung verwenden.",
  Release: "Release",
  "Signed by": "Signiert von",
  Manifest: "Manifest",
  File: "Datei",
  Size: "Grösse",
  "To check a release yourself, build the same tag and compare the manifest hash.":
    "Um ein Release selbst zu prüfen, denselben Tag bauen und den Manifest-Hash vergleichen.",

  "This copy of Labterm Web changed on the server outside a signed release. Compare the manifest hash with the release page before you unlock.":
    "Diese Kopie von Labterm Web wurde auf dem Server ausserhalb eines signierten Releases geändert. Vergleiche den Manifest-Hash mit der Release-Seite, bevor du entsperrst.",

  // host keys (patch 0001)
  "Unknown host": "Unbekannter Host",
  "Host key has changed": "Host-Schlüssel hat sich geändert",
  "{target} answered with a different key than the one in your known hosts. Someone may be intercepting the connection, including the relay, or the server was reinstalled. Nothing was sent.":
    "{target} hat mit einem anderen Schlüssel geantwortet als dem in deinen bekannten Hosts. Jemand könnte die Verbindung abfangen, auch das Relay, oder der Server wurde neu installiert. Es wurde nichts gesendet.",
  "{target} is not in your known hosts. The web app never trusts a key on first use, because the relay could be the one answering.":
    "{target} ist nicht in deinen bekannten Hosts. Die Web-App vertraut einem Schlüssel nie beim ersten Kontakt, denn es könnte das Relay sein, das antwortet.",
  "Check the fingerprint on the server's console. If it is right, trust it in Labterm on your desktop or phone; it syncs here.":
    "Prüfe den Fingerabdruck an der Konsole des Servers. Stimmt er, vertraue ihm in Labterm auf dem Desktop oder Handy; er wird hierher synchronisiert.",
  "The host key differs from your known hosts. Nothing was sent.":
    "Der Host-Schlüssel weicht von deinen bekannten Hosts ab. Es wurde nichts gesendet.",
  "This host is not in your known hosts. Trust it from Labterm on a trusted device first.":
    "Dieser Host ist nicht in deinen bekannten Hosts. Vertraue ihm zuerst in Labterm auf einem vertrauenswürdigen Gerät.",
  "Paste the server's public key (ssh-keyscan or /etc/ssh/ssh_host_*.pub). Labterm Web never trusts a key on first use.":
    "Den öffentlichen Schlüssel des Servers einfügen (ssh-keyscan oder /etc/ssh/ssh_host_*.pub). Labterm Web vertraut einem Schlüssel nie beim ersten Kontakt.",

  // connecting
  "Cannot reach the relay at {url}": "Das Relay unter {url} ist nicht erreichbar",
  "The relay refused (HTTP {status})": "Das Relay hat abgelehnt (HTTP {status})",
  "This Labterm Web has no relay configured (config.json).":
    "Für dieses Labterm Web ist kein Relay konfiguriert (config.json).",
  "The web app connects to SSH hosts only (no local shell, serial or Mosh).":
    "Die Web-App verbindet sich nur mit SSH-Hosts (keine lokale Shell, kein Seriell, kein Mosh).",
  "No username": "Kein Benutzername",
  "Sign-in refused (tried {tried}; the server offers {offered})":
    "Anmeldung abgelehnt (versucht: {tried}; der Server bietet: {offered})",
  "Connection failed": "Verbindung fehlgeschlagen",

  // identity provider (Authentik)
  "This Labterm Web has no sign-in provider configured (config.json).":
    "Für dieses Labterm Web ist kein Anmeldeanbieter konfiguriert (config.json).",
  "Cannot read the sign-in provider's configuration (HTTP {status})":
    "Die Konfiguration des Anmeldeanbieters kann nicht gelesen werden (HTTP {status})",
  "The provider calls itself {issuer}, not {expected}":
    "Der Anbieter nennt sich {issuer}, nicht {expected}",
  "Sign-in failed: {reason}": "Anmeldung fehlgeschlagen: {reason}",
  "Sign-in window closed": "Anmeldefenster geschlossen",
  "Allow pop-ups for this page to sign in to your identity provider.":
    "Pop-ups für diese Seite erlauben, um dich beim Identitätsanbieter anzumelden.",
  "The provider sent no ID token": "Der Anbieter hat kein ID-Token gesendet",

  // Labterm account
  "Wrong email or password.": "E-Mail oder Passwort falsch.",
  "No sign-in in progress": "Keine Anmeldung im Gang",
  "No second factor pending": "Kein zweiter Faktor ausstehend",
  "No device approval pending": "Keine Gerätefreigabe ausstehend",
  "Unexpected answer from the server": "Unerwartete Antwort vom Server",
  "Sign in to your Labterm account first": "Zuerst beim Labterm-Konto anmelden",
  "Your Labterm session ended. Sign in again.":
    "Deine Labterm-Sitzung ist abgelaufen. Bitte erneut anmelden.",

  // vault
  "The vault is locked": "Der Tresor ist gesperrt",
  "Not found: {kind}": "Nicht gefunden: {kind}",
  "Vault not found": "Tresor nicht gefunden",
  "This vault is view-only": "Dieser Tresor ist schreibgeschützt",
  "Enter an address": "Adresse eingeben",
  "Enter a name": "Namen eingeben",
  "Use Move to vault to change the vault":
    "Zum Wechseln des Tresors «In Tresor verschieben» verwenden",
  "A group can't be inside itself": "Eine Gruppe kann nicht in sich selbst liegen",
  "Pick an OpenID key from the same vault": "Wähle einen OpenID-Schlüssel aus demselben Tresor",
  "Use a colour like #3a7 or #33aa77": "Verwende eine Farbe wie #3a7 oder #33aa77",
  "These tags are in different vaults": "Diese Tags liegen in verschiedenen Tresoren",
  "This tag is in another vault": "Dieses Tag liegt in einem anderen Tresor",
  "The web app generates Ed25519 keys only": "Die Web-App erzeugt nur Ed25519-Schlüssel",
  "Wrong passphrase": "Falsche Passphrase",

  // not in the browser
  "{what} is not available in the browser.": "{what} ist im Browser nicht verfügbar.",
  "A vault without a lock password": "Ein Tresor ohne Sperrpasswort",
  "Creating an account (use the Labterm app, then sign in here)":
    "Ein Konto erstellen (in der Labterm-App, danach hier anmelden)",
  "Single sign-on to Labterm": "Single Sign-on bei Labterm",
  "Reading key files from disk": "Schlüsseldateien von der Festplatte lesen",
  "Saving files to disk": "Dateien auf der Festplatte speichern",
  "Importing known_hosts": "known_hosts importieren",
  "Importing known_hosts (pin keys in the Labterm app or paste one on the host)":
    "known_hosts importieren (Schlüssel in der Labterm-App anheften oder beim Host einfügen)",
  SFTP: "SFTP",
  "Port forwarding": "Portweiterleitung",
  "Running snippets on several hosts": "Snippets auf mehreren Hosts ausführen",
};
