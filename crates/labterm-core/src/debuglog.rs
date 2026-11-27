//! Debug log files: the app's own `tracing` output on disk, one file per
//! launch, so the in-app viewer can show this run and the ones before it
//! (the run that crashed, the sign-in that failed yesterday).
//!
//! Every line is redacted before it is written and again when it is read:
//! keys, tokens, passwords and OAuth parameters never reach the file, the
//! screen, the clipboard or a share target, even from a debug build.
//!
//! The shells plug [`FileWriter`] into their `tracing_subscriber::fmt`
//! layer (`with_writer(Mutex::new(writer))`), so this module needs nothing
//! but `std`.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, OnceLock};

use regex_lite::Regex;
use time::OffsetDateTime;

use crate::error::{CoreError, Result};

const PREFIX: &str = "labterm-";
const EXT: &str = ".log";
/// A launch that logs more than this goes on in a new file.
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
/// Files kept, the current one included. Older ones are deleted at start.
pub const KEEP_FILES: usize = 10;
/// The viewer reads at most this much from the end of a file.
pub const MAX_READ_BYTES: u64 = 2 * 1024 * 1024;

static DIR: OnceLock<PathBuf> = OnceLock::new();
static CURRENT: Mutex<Option<String>> = Mutex::new(None);

/// Where this process writes its debug logs, once [`FileWriter::open`] ran.
pub fn dir() -> Option<&'static Path> {
    DIR.get().map(PathBuf::as_path)
}

/// One debug log file, for the viewer's file picker.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFile {
    /// File name, which is also what [`read`] takes.
    pub name: String,
    /// Bytes on disk.
    pub size: u64,
    /// Last write, Unix seconds.
    pub modified: i64,
    /// The file this process is writing now.
    pub current: bool,
}

/// Writes redacted lines into the current launch's file, starting a new
/// file when one grows past [`MAX_FILE_BYTES`].
pub struct FileWriter {
    dir: PathBuf,
    stem: String,
    part: u32,
    file: File,
    written: u64,
    pending: Vec<u8>,
}

impl FileWriter {
    /// Start this launch's file in `dir` (created if missing) and delete
    /// the oldest files beyond [`KEEP_FILES`]. Also makes `dir` the one
    /// [`list`] and [`read`] look in.
    pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        let now = OffsetDateTime::now_utc();
        let stem = format!(
            "{PREFIX}{:04}{:02}{:02}-{:02}{:02}{:02}",
            now.year(),
            u8::from(now.month()),
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        );
        let (file, part) = create(&dir, &stem, 1)?;
        let _ = DIR.set(dir.clone());
        let w = Self {
            dir,
            stem,
            part,
            file,
            written: 0,
            pending: Vec::new(),
        };
        prune(&w.dir, KEEP_FILES);
        Ok(w)
    }

    fn write_line(&mut self, line: &[u8]) -> io::Result<()> {
        if self.written >= MAX_FILE_BYTES {
            let (file, part) = create(&self.dir, &self.stem, self.part + 1)?;
            self.file = file;
            self.part = part;
            self.written = 0;
            prune(&self.dir, KEEP_FILES);
        }
        let text = String::from_utf8_lossy(line);
        let mut out = redact(text.trim_end_matches(['\r', '\n']));
        out.push('\n');
        self.file.write_all(out.as_bytes())?;
        self.written += out.len() as u64;
        Ok(())
    }
}

impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);
        while let Some(nl) = self.pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=nl).collect();
            self.write_line(&line)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

/// `stem.log`, or `stem-N.log` from part 2 on; skips names already taken
/// (a second instance started in the same second).
fn create(dir: &Path, stem: &str, mut part: u32) -> io::Result<(File, u32)> {
    loop {
        let name = file_name(stem, part);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(&name))
        {
            Ok(f) => {
                *CURRENT.lock().unwrap_or_else(|p| p.into_inner()) = Some(name);
                return Ok((f, part));
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && part < 1000 => part += 1,
            Err(e) => return Err(e),
        }
    }
}

fn file_name(stem: &str, part: u32) -> String {
    if part <= 1 {
        format!("{stem}{EXT}")
    } else {
        format!("{stem}-{part}{EXT}")
    }
}

fn is_log_name(name: &str) -> bool {
    name.starts_with(PREFIX)
        && name.ends_with(EXT)
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Sort key: names sort by start time, parts by number.
fn order_key(name: &str) -> (String, u32) {
    let stem = name.trim_end_matches(EXT);
    // labterm-YYYYMMDD-HHMMSS[-N]
    let base_len = PREFIX.len() + 15;
    if stem.len() > base_len {
        let (base, rest) = stem.split_at(base_len);
        if let Some(n) = rest.strip_prefix('-').and_then(|n| n.parse().ok()) {
            return (base.to_string(), n);
        }
    }
    (stem.to_string(), 1)
}

fn prune(dir: &Path, keep: usize) {
    let Ok(files) = list_in(dir) else { return };
    for f in files.iter().skip(keep).filter(|f| !f.current) {
        let _ = fs::remove_file(dir.join(&f.name));
    }
}

fn list_in(dir: &Path) -> io::Result<Vec<LogFile>> {
    let current = CURRENT.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if !is_log_name(&name) {
            continue;
        }
        let meta = entry.metadata()?;
        if !meta.is_file() {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);
        out.push(LogFile {
            current: current.as_deref() == Some(name.as_str()),
            name,
            size: meta.len(),
            modified,
        });
    }
    out.sort_by_key(|f| std::cmp::Reverse(order_key(&f.name)));
    Ok(out)
}

/// The debug log files, newest first. Empty before [`FileWriter::open`].
pub fn list() -> Result<Vec<LogFile>> {
    match dir() {
        Some(d) => Ok(list_in(d)?),
        None => Ok(Vec::new()),
    }
}

/// The last [`MAX_READ_BYTES`] of one file, as redacted lines.
pub fn read(name: &str) -> Result<Vec<String>> {
    let Some(d) = dir() else {
        return Err(CoreError::NotFound("debug log".into()));
    };
    read_in(d, name)
}

fn read_in(dir: &Path, name: &str) -> Result<Vec<String>> {
    // Only names `list` hands out: no paths, no other files in the folder.
    if !is_log_name(name) {
        return Err(CoreError::Invalid(format!("not a debug log: {name}")));
    }
    let mut f = File::open(dir.join(name)).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => CoreError::NotFound(format!("debug log {name}")),
        _ => CoreError::Io(e),
    })?;
    let len = f.metadata()?.len();
    let skip = len.saturating_sub(MAX_READ_BYTES);
    f.seek(SeekFrom::Start(skip))?;
    let mut buf = Vec::with_capacity((len - skip) as usize);
    f.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    let mut lines = text.lines();
    if skip > 0 {
        // Started mid-line.
        lines.next();
    }
    Ok(lines.map(redact).collect())
}

// Values that must never reach a log, even from a debug build. The mobile
// viewers mask logcat with these too (`debug_log_redact`).
static RULES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"-----BEGIN [A-Z ]*PRIVATE KEY-----.*", "<private key>"),
        (
            r"eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]*",
            "<jwt>",
        ),
        (r"(?i)\b(bearer|basic)\s+[A-Za-z0-9._~+/=-]+", "${1} <redacted>"),
        // OAuth parameters in URLs (redirects, authorize and token requests).
        (
            r#"(?i)([?&#](?:code|state|code_verifier|code_challenge|nonce|login_hint)=)[^&\s"']+"#,
            "${1}<redacted>",
        ),
        // Secret-looking fields in key=value, key: value and JSON form.
        (
            r#"(?i)\b(code_verifier|access_token|id_token|refresh_token|token|client_secret|secret|password|passphrase|setup_key|setupkey|private_key|privatekey|psk|otp|pin)("?\s*[=:]\s*"?)[^\s&"',;}]+"#,
            "${1}${2}<redacted>",
        ),
    ]
    .into_iter()
    .map(|(re, with)| (Regex::new(re).expect("redaction rule"), with))
    .collect()
});

// Base64 bodies of keys (PEM lines, blobs) and long tokens. Hex runs
// (SHA-256 sums, commit ids) are not secret and stay.
static LONG_BASE64: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z0-9+/]{60,}={0,2}").expect("redaction rule"));
// WireGuard keys and other 32-byte base64 values.
static KEY32: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Za-z0-9+/]{43}=").expect("redaction rule"));

/// `line` with keys, tokens, passwords and OAuth parameters masked.
pub fn redact(line: &str) -> String {
    let mut out = line.to_string();
    for (re, with) in RULES.iter() {
        if re.is_match(&out) {
            out = re.replace_all(&out, *with).into_owned();
        }
    }
    if LONG_BASE64.is_match(&out) {
        out = LONG_BASE64
            .replace_all(&out, |c: &regex_lite::Captures<'_>| {
                let m = &c[0];
                if m.bytes().all(|b| b.is_ascii_hexdigit()) {
                    m.to_string()
                } else {
                    "<base64>".to_string()
                }
            })
            .into_owned();
    }
    if KEY32.is_match(&out) {
        out = KEY32.replace_all(&out, "<key>").into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_secrets() {
        let cases = [
            (
                "Authorization: Bearer abc.def-123",
                "Authorization: Bearer <redacted>",
            ),
            (
                "GET https://idp.example.com/cb?code=s3cr3t&state=xyz&other=1",
                "GET https://idp.example.com/cb?code=<redacted>&state=<redacted>&other=1",
            ),
            (
                r#"{"password":"hunter2","user":"mike"}"#,
                r#"{"password":"<redacted>","user":"mike"}"#,
            ),
            ("setup_key=ABCD-1234 ok", "setup_key=<redacted> ok"),
            ("pin: 1234", "pin: <redacted>"),
            (
                "token eyJhbGciOiJ.eyJzdWIiOiIx.c2lnbmF0dXJl here",
                "token <jwt> here",
            ),
            (
                "key AAAAC3NzaC1lZDI1NTE5AAAAIGZm0123456789abcdefghijklmnopqrstuvw= end",
                "key <base64> end",
            ),
            (
                "wg 0123456789abcdefghijklmnopqrstuvwxyzABCDEFG= end",
                "wg <key> end",
            ),
            ("-----BEGIN OPENSSH PRIVATE KEY----- abc", "<private key>"),
        ];
        for (input, want) in cases {
            assert_eq!(redact(input), want, "{input}");
        }
        // Plain debug lines stay readable.
        let plain = "ssh: connected to mike@host.example.com:22 in 412 ms";
        assert_eq!(redact(plain), plain);
        let fp = "host key SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU";
        assert_eq!(redact(fp), fp);
        let sum = "verify: SHA-256 matches (9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08)";
        assert_eq!(redact(sum), sum);
    }

    #[test]
    fn names_are_checked() {
        assert!(is_log_name("labterm-20261127-060224.log"));
        assert!(is_log_name("labterm-20261127-060224-2.log"));
        assert!(!is_log_name("../vault.db"));
        assert!(!is_log_name("labterm-../../x.log"));
        assert!(!is_log_name("labterm-a/b.log"));
        assert!(!is_log_name("vault.db"));
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("vault.db"), "secret").unwrap();
        assert!(matches!(
            read_in(dir.path(), "vault.db"),
            Err(CoreError::Invalid(_))
        ));
    }

    #[test]
    fn writes_redacted_lines_and_orders_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("labterm-20260101-000000.log"), "old\n").unwrap();
        fs::write(dir.path().join("labterm-20260101-000000-2.log"), "old 2\n").unwrap();
        fs::write(
            dir.path().join("labterm-20260101-000000-10.log"),
            "old 10\n",
        )
        .unwrap();
        fs::write(dir.path().join("notes.txt"), "not a log\n").unwrap();
        let mut w = FileWriter {
            dir: dir.path().to_path_buf(),
            stem: "labterm-20991231-235959".into(),
            part: 1,
            file: create(dir.path(), "labterm-20991231-235959", 1).unwrap().0,
            written: 0,
            pending: Vec::new(),
        };
        w.write_all(b"first password=hunter2\nsecond ").unwrap();
        w.write_all(b"half\n").unwrap();
        let files = list_in(dir.path()).unwrap();
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "labterm-20991231-235959.log",
                "labterm-20260101-000000-10.log",
                "labterm-20260101-000000-2.log",
                "labterm-20260101-000000.log",
            ]
        );
        assert_eq!(
            read_in(dir.path(), &files[0].name).unwrap(),
            ["first password=<redacted>", "second half"]
        );
        prune(dir.path(), 2);
        assert_eq!(list_in(dir.path()).unwrap().len(), 2);
        assert!(dir.path().join("notes.txt").exists());
    }

    #[test]
    fn rolls_over_when_full() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = FileWriter {
            dir: dir.path().to_path_buf(),
            stem: "labterm-20991231-235958".into(),
            part: 1,
            file: create(dir.path(), "labterm-20991231-235958", 1).unwrap().0,
            written: MAX_FILE_BYTES,
            pending: Vec::new(),
        };
        w.write_all(b"next\n").unwrap();
        assert!(dir.path().join("labterm-20991231-235958-2.log").exists());
        assert_eq!(
            read_in(dir.path(), "labterm-20991231-235958-2.log").unwrap(),
            ["next"]
        );
    }
}
