//! Server-side checks for vault and team colours and icons (the same rules
//! every client applies, from `labterm_proto::style`).

use crate::error::{ApiResult, Error};

/// One requested change: `None` = leave as is, `Some(None)` = clear.
pub type Change = Option<Option<String>>;

/// Validate a `PATCH` body's `color` / `icon`. An empty string clears.
pub fn validate(color: Option<&str>, icon: Option<&str>) -> ApiResult<(Change, Change)> {
    let color = color
        .map(|c| labterm_proto::style::normalize_color(Some(c)))
        .transpose()
        .map_err(Error::bad_request)?;
    let icon = icon
        .map(|i| labterm_proto::style::normalize_icon(Some(i)))
        .transpose()
        .map_err(Error::bad_request)?;
    Ok((color, icon))
}

/// Audit details: the colour, and only whether the icon changed (a custom
/// image would bloat the log).
pub fn audit_details(color: &Change, icon: &Change) -> serde_json::Value {
    let icon = icon.as_ref().map(|i| match i {
        None => "default".to_string(),
        Some(v) if v.starts_with("custom:") => "custom image".to_string(),
        Some(v) => v.clone(),
    });
    serde_json::json!({ "color": color, "icon": icon })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_clears() {
        let (c, i) = validate(Some("#ABC"), Some("")).unwrap();
        assert_eq!(c, Some(Some("#aabbcc".into())));
        assert_eq!(i, Some(None));
        let (c, i) = validate(None, None).unwrap();
        assert_eq!((c, i), (None, None));
        assert!(validate(Some("red"), None).is_err());
        assert!(validate(None, Some("custom:url:https://example.com/a.png")).is_err());
    }

    #[test]
    fn audit_hides_image_data() {
        let d = audit_details(
            &None,
            &Some(Some("custom:data:data:image/png;base64,AA==".into())),
        );
        assert_eq!(d["icon"], "custom image");
    }
}
