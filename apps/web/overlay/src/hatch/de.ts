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
  "All sessions": "Alle Sitzungen",
  Command: "Befehl",
  "Command window": "Befehlsfenster",
  "Host groups": "Hostgruppen",
  "Keys go straight to {count} sessions": "Tasten gehen direkt an {count} Sitzungen",
  "New tab group": "Neue Tab-Gruppe",
  "New tab group…": "Neue Tab-Gruppe…",
  "Send commands to “{name}”": "Befehle an «{name}» senden",
  "Send keys as I type": "Tasten sofort senden",
  "Send to": "Senden an",
  "Send to {count}": "An {count} senden",
  "Tab groups": "Tab-Gruppen",
  "This session": "Diese Sitzung",
  "Type a command, Enter sends it, Shift+Enter adds a line":
    "Befehl eingeben: Enter sendet, Shift+Enter fügt eine Zeile hinzu",
  "Visible sessions": "Sichtbare Sitzungen",
  "Connect all": "Alle verbinden",
  "Connect all ({n})": "Alle verbinden ({n})",
  "Connect {n} hosts?": "{n} Hosts verbinden?",
  "Opens a tab for every host in “{label}” and its sub-groups.":
    "Öffnet einen Tab für jeden Host in «{label}» und seinen Untergruppen.",

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
  "Joining VPN networks": "Das Beitreten zu VPN-Netzwerken",
  "This host connects through the VPN network {name}, which the browser cannot join yet. Use the desktop or Android app.":
    "Dieser Host verbindet über das VPN-Netzwerk {name}, dem der Browser noch nicht beitreten kann. Verwende die Desktop- oder Android-App.",
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

  // team access rules (acl.ts)
  "Your access to this host lets you connect, not open its settings.":
    "Dein Zugriff auf diesen Host erlaubt das Verbinden, aber nicht seine Einstellungen.",
  "Your access to this group lets you connect to its hosts, not open its settings.":
    "Dein Zugriff auf diese Gruppe erlaubt das Verbinden mit ihren Hosts, aber nicht ihre Einstellungen.",
  "Your team's access rules hide this host from you.":
    "Die Zugriffsregeln deines Teams verbergen diesen Host vor dir.",
  // access overview (policy viewer)
  "OpenID (opkssh)": "OpenID (opkssh)",
  Certificate: "Zertifikat",
  Key: "Schlüssel",
  Password: "Passwort",
  "Asks when connecting": "Fragt beim Verbinden",
  "Other hosts in its group use a VPN network; this one connects without one.":
    "Andere Hosts seiner Gruppe nutzen ein VPN-Netzwerk; dieser verbindet ohne.",
  "Logs in with a password while the rest of the vault uses keys.":
    "Meldet sich mit einem Passwort an, während der Rest des Tresors Schlüssel nutzt.",
  "Logs in with a password, but the team allows keys only.":
    "Meldet sich mit einem Passwort an, aber das Team erlaubt nur Schlüssel.",
  "A jump host is in another vault or was deleted, so connections fail.":
    "Ein Jump-Host liegt in einem anderen Tresor oder wurde gelöscht, darum schlagen Verbindungen fehl.",
  "The team requires a jump host, and this host has none.":
    "Das Team verlangt einen Jump-Host, und dieser Host hat keinen.",
  "Your own credentials replace the team's login on this device.":
    "Deine eigenen Zugangsdaten ersetzen auf diesem Gerät die Anmeldung des Teams.",
  "Its settings can't be read: {detail}":
    "Seine Einstellungen können nicht gelesen werden: {detail}",
  Hidden: "Verborgen",
  "Connect only": "Nur verbinden",
  Read: "Lesen",
  Edit: "Bearbeiten",
  "not set": "nicht gesetzt",
  "set on the host": "am Host gesetzt",
  "from group {label}": "aus Gruppe {label}",
  "from tag {label}": "aus Tag {label}",
  "from your credentials {label}": "aus deinen Zugangsdaten {label}",
  "Edit host": "Host bearbeiten",
  "Edit group {label}": "Gruppe {label} bearbeiten",
  "Edit tag {label}": "Tag {label} bearbeiten",
  "Edit my credentials": "Meine Zugangsdaten bearbeiten",
  "Fix: {problem}": "Beheben: {problem}",
  "This vault is locked": "Dieser Tresor ist gesperrt",
  "Unlock it to see how its hosts connect.":
    "Entsperre ihn, um zu sehen, wie seine Hosts verbinden.",
  "No hosts in this vault": "Keine Hosts in diesem Tresor",
  "Hosts you add show up here with their login, network and jump hosts.":
    "Hosts, die du hinzufügst, erscheinen hier mit Anmeldung, Netzwerk und Jump-Hosts.",
  "Nothing matches": "Keine Treffer",
  "Try another search.": "Versuche eine andere Suche.",
  "Access overview": "Zugriffsübersicht",
  "{vault} · hosts: {count} · need a look: {risks}":
    "{vault} · Hosts: {count} · zu prüfen: {risks}",
  "{vault} · hosts: {count}": "{vault} · Hosts: {count}",
  "All hosts": "Alle Hosts",
  "Needs a look": "Zu prüfen",
  "Search hosts": "Hosts suchen",
  "Worked out on this device from the decrypted vault. Access rules live on the server, which blocks changes below Edit; every member's device can still decrypt all items of this vault.":
    "Auf diesem Gerät aus dem entschlüsselten Tresor berechnet. Zugriffsregeln liegen auf dem Server, der Änderungen unterhalb von «Bearbeiten» blockiert; das Gerät jedes Mitglieds kann trotzdem alle Einträge dieses Tresors entschlüsseln.",
  "Worked out on this device. Only you can see the items of this vault.":
    "Auf diesem Gerät berechnet. Nur du siehst die Einträge dieses Tresors.",
  "Direct, no VPN network": "Direkt, ohne VPN-Netzwerk",
  None: "Keine",
  "(outside this vault)": "(ausserhalb dieses Tresors)",
  Login: "Anmeldung",
  "VPN network": "VPN-Netzwerk",
  "Jump hosts": "Jump-Hosts",
  "Actions for {label}": "Aktionen für {label}",
  Fix: "Beheben",
  "How it connects": "Wie er verbindet",
  Proxy: "Proxy",
  "Your login": "Deine Anmeldung",
  "“{label}” is paused: the host's address or route changed":
    "«{label}» ist pausiert: Adresse oder Route des Hosts hat sich geändert",
  "“{label}” is used on this device": "«{label}» wird auf diesem Gerät verwendet",
  "Who can see this host": "Wer diesen Host sieht",
  "Only people who manage access see the rules on hosts and groups; this shows vault roles.":
    "Nur wer den Zugriff verwaltet, sieht die Regeln auf Hosts und Gruppen; hier stehen die Tresorrollen.",
  "Team admin, never limited": "Team-Admin, nie eingeschränkt",
  "Vault role": "Tresorrolle",
  "Rule on this host": "Regel auf diesem Host",
  "Rule on a group above": "Regel auf einer übergeordneten Gruppe",
  "IdP groups may change this: {groups}": "IdP-Gruppen können das ändern: {groups}",
  "Members of IdP group {group}": "Mitglieder der IdP-Gruppe {group}",
  "How each host logs in, which VPN network and jump hosts it uses, who can see it, and setups worth a second look.":
    "Wie sich jeder Host anmeldet, welches VPN-Netzwerk und welche Jump-Hosts er nutzt, wer ihn sieht und welche Einstellungen eine Prüfung wert sind.",
  Open: "Öffnen",

  // debug log (Settings → About)
  "Debug log": "Debug-Protokoll",
  "What Labterm logged in this and earlier launches, with timings for sync, VPN networks, OpenID keys, updates and SSH connects. Secrets are removed before anything is written.":
    "Was Labterm in diesem und früheren Starts protokolliert hat, mit Zeiten für Synchronisation, VPN-Netzwerke, OpenID-Schlüssel, Updates und SSH-Verbindungen. Geheimnisse werden entfernt, bevor etwas geschrieben wird.",
  "Log file": "Protokolldatei",
  "This launch · {size}": "Dieser Start · {size}",
  "Nothing logged here yet.": "Hier wurde noch nichts protokolliert.",
  Show: "Anzeigen",
  Refresh: "Aktualisieren",
  Copy: "Kopieren",
  Copied: "Kopiert",

  // security keys in the browser (webauthn.ts)
  "This browser (security key or passkey)": "Dieser Browser (Sicherheitsschlüssel oder Passkey)",
  "The browser asks for the PIN and the touch. Keys made here are ECDSA keys for this Labterm Web address; they also work in the desktop and Android apps, but not with ssh on the command line.":
    "Der Browser fragt nach PIN und Berührung. Hier erstellte Schlüssel sind ECDSA-Schlüssel für diese Labterm-Web-Adresse; sie funktionieren auch in den Desktop- und Android-Apps, aber nicht mit ssh auf der Kommandozeile.",
  "Use security key": "Sicherheitsschlüssel verwenden",
  "Sign in with {key}. Your browser then asks for the security key.":
    "Anmelden mit {key}. Danach fragt dein Browser nach dem Sicherheitsschlüssel.",
  "Sign in with your security key. Your browser then asks for it.":
    "Mit deinem Sicherheitsschlüssel anmelden. Danach fragt dein Browser danach.",
  "Cancelled, or the security key timed out":
    "Abgebrochen, oder der Sicherheitsschlüssel hat nicht rechtzeitig geantwortet",
  "This security key already holds a key for this site":
    "Dieser Sicherheitsschlüssel enthält schon einen Schlüssel für diese Seite",
  "This address cannot use security keys (it needs HTTPS and a host name)":
    "Diese Adresse kann keine Sicherheitsschlüssel verwenden (sie braucht HTTPS und einen Hostnamen)",
  "Security key: {message}": "Sicherheitsschlüssel: {message}",
  "This browser has no security key support":
    "Dieser Browser unterstützt keine Sicherheitsschlüssel",
  "The browser did not return the new key's public key":
    "Der Browser hat den öffentlichen Schlüssel des neuen Schlüssels nicht zurückgegeben",
  "This security key belongs to Labterm Web at {site}":
    "Dieser Sicherheitsschlüssel gehört zu Labterm Web unter {site}",
  "Loading resident keys from a security key":
    "Residente Schlüssel von einem Sicherheitsschlüssel laden",
  "Touch your security key to continue": "Berühre deinen Sicherheitsschlüssel, um fortzufahren",
};
