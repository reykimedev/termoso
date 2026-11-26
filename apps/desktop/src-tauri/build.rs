use std::process::Command;

fn main() {
    // The commit a binary was built from, shown in Settings → About and in
    // `--version`, so a user can match the build to its release and source.
    // CI sets GITHUB_SHA; local builds ask git; a source tarball has neither.
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    // logs/HEAD moves on every commit and checkout; skip what a tarball lacks,
    // since cargo reruns the script on every build for a missing path.
    for p in ["../../../.git/HEAD", "../../../.git/logs/HEAD"] {
        if std::path::Path::new(p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }
    let commit = std::env::var("GITHUB_SHA")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=LABTERM_BUILD_COMMIT={commit}");
    tauri_build::build()
}
