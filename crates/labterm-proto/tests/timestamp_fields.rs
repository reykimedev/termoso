//! Every wire type is serde, and `time` has its own serde form (enabled by
//! other crates in the build), so a timestamp field without our `with`
//! attribute would compile and silently change the vault and sync format.

use std::fs;
use std::path::Path;

#[test]
fn every_timestamp_field_uses_the_chrono_compatible_form() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut missing = Vec::new();
    for entry in fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().unwrap() == "timestamp.rs" {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let code = line.split("//").next().unwrap();
            if !code.contains("OffsetDateTime") || code.trim_start().starts_with("use ") {
                continue;
            }
            // Look through the attributes and doc comments above the field.
            let has_with = lines[..i]
                .iter()
                .rev()
                .take_while(|l| {
                    let t = l.trim_start();
                    t.starts_with("#[") || t.starts_with("///")
                })
                .any(|l| l.contains("timestamp::serde"));
            if !has_with {
                missing.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "add #[serde(with = ...)]:\n{}",
        missing.join("\n")
    );
}

#[test]
fn sync_change_keeps_its_json() {
    use labterm_proto::sync::EntityChange;

    // As chrono 0.4.45 wrote it, without the optional field.
    let old = r#"{"id":"6f1c2a3e-0000-4000-8000-000000000001","kind":"host","vault_id":"6f1c2a3e-0000-4000-8000-000000000002","key_version":1,"data":"x","updated_at":"2026-11-18T11:06:40.120Z"}"#;
    let change: EntityChange = serde_json::from_str(old).unwrap();
    assert!(change.secret_rotated_at.is_none());
    let json = serde_json::to_value(&change).unwrap();
    assert_eq!(json["updated_at"], "2026-11-18T11:06:40.120Z");

    let with_rotation = old.replace(
        r#""data":"x""#,
        r#""data":"x","secret_rotated_at":"2026-11-18T11:06:40Z""#,
    );
    let change: EntityChange = serde_json::from_str(&with_rotation).unwrap();
    let json = serde_json::to_value(&change).unwrap();
    assert_eq!(json["secret_rotated_at"], "2026-11-18T11:06:40Z");
}
