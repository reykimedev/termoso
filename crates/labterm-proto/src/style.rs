//! Look of a host, group, vault or team: a tile colour and an icon.
//!
//! Hosts and groups keep these inside their encrypted payload; vaults and
//! teams keep them next to their (plaintext) name on the server, so lists can
//! show them before a vault is unlocked. Every client and the server use the
//! same rules.

/// Longest stored custom icon data URL (after the `custom:data:` prefix).
pub const MAX_CUSTOM_ICON_DATA_LENGTH: usize = 32 * 1024;
/// Prefix of a custom icon value.
pub const CUSTOM_ICON_PREFIX: &str = "custom:data:";
/// Longest built-in icon id (`ubuntu`, `zabbix`, …).
const MAX_ICON_ID_LENGTH: usize = 64;

/// `#rrggbb` for `#3a7`, `33AA77`, ` #33aa77 `; `Ok(None)` for empty input.
pub fn normalize_color(color: Option<&str>) -> Result<Option<String>, &'static str> {
    let c = color
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if c.is_empty() {
        return Ok(None);
    }
    let hex = c.strip_prefix('#').unwrap_or(&c);
    if !matches!(hex.len(), 3 | 6) || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("colour must be a hex value");
    }
    if hex.len() == 3 {
        let long: String = hex.chars().flat_map(|ch| [ch, ch]).collect();
        return Ok(Some(format!("#{long}")));
    }
    Ok(Some(format!("#{hex}")))
}

/// A `custom:data:` value holding a base64 data URL of an image type
/// clients can draw safely, within the size limit. Nothing remote.
pub fn valid_custom_icon(value: &str) -> bool {
    let Some(data_url) = value.strip_prefix(CUSTOM_ICON_PREFIX) else {
        return false;
    };
    if data_url.len() > MAX_CUSTOM_ICON_DATA_LENGTH {
        return false;
    }
    let Some((header, payload)) = data_url.split_once(',') else {
        return false;
    };
    let Some(mime) = header
        .strip_prefix("data:")
        .and_then(|h| h.strip_suffix(";base64"))
    else {
        return false;
    };
    if !matches!(
        mime.to_ascii_lowercase().as_str(),
        "image/png"
            | "image/jpeg"
            | "image/gif"
            | "image/webp"
            | "image/svg+xml"
            | "image/x-icon"
            | "image/avif"
    ) {
        return false;
    }
    !payload.is_empty()
        && payload
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

/// Stored icon for user input: `None` for empty / `auto`, a valid custom
/// icon, or a short built-in id (`[a-z0-9._-]`, which clients that don't
/// know it show as the default).
pub fn normalize_icon(icon: Option<&str>) -> Result<Option<String>, &'static str> {
    let Some(value) = icon
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "auto")
    else {
        return Ok(None);
    };
    if value.starts_with("custom:") {
        return if valid_custom_icon(value) {
            Ok(Some(value.to_string()))
        } else {
            Err("invalid custom icon")
        };
    }
    let id = value.to_ascii_lowercase();
    if id.len() > MAX_ICON_ID_LENGTH
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err("invalid icon id");
    }
    Ok(Some(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours() {
        assert_eq!(
            normalize_color(Some(" #ABC ")).unwrap().as_deref(),
            Some("#aabbcc")
        );
        assert_eq!(
            normalize_color(Some("1e88e5")).unwrap().as_deref(),
            Some("#1e88e5")
        );
        assert_eq!(normalize_color(Some("")).unwrap(), None);
        assert_eq!(normalize_color(None).unwrap(), None);
        assert!(normalize_color(Some("red")).is_err());
        assert!(normalize_color(Some("#12")).is_err());
    }

    #[test]
    fn icons() {
        assert_eq!(
            normalize_icon(Some(" Server ")).unwrap().as_deref(),
            Some("server")
        );
        assert_eq!(normalize_icon(Some("auto")).unwrap(), None);
        assert_eq!(normalize_icon(None).unwrap(), None);
        let png = "custom:data:data:image/png;base64,AAECAw==";
        assert_eq!(normalize_icon(Some(png)).unwrap().as_deref(), Some(png));
        assert!(normalize_icon(Some("custom:data:data:text/html;base64,PGI+")).is_err());
        assert!(normalize_icon(Some("custom:url:https://example.com/a.png")).is_err());
        assert!(normalize_icon(Some("<script>")).is_err());
        let big = format!(
            "custom:data:data:image/png;base64,{}",
            "A".repeat(40 * 1024)
        );
        assert!(normalize_icon(Some(&big)).is_err());
    }
}
