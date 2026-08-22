pub fn normalize_source(raw: Option<&str>) -> String {
    match raw.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("creative") => "creative".into(),
        Some("command") => "command".into(),
        Some("admin") => "admin".into(),
        Some("player") => "player".into(),
        _ => "player".into(),
    }
}

pub fn normalize_origin(raw: Option<&str>) -> String {
    match raw.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("creative") => "creative".into(),
        Some("command") => "command".into(),
        Some("survival") => "survival".into(),
        _ => "survival".into(),
    }
}

pub fn is_blocked(source: &str, origin: &str) -> bool {
    const BLOCKED_SOURCES: &[&str] = &["creative", "admin", "command"];
    const BLOCKED_ORIGINS: &[&str] = &["creative", "command", "unknown"];
    let s = source.to_ascii_lowercase();
    let o = origin.to_ascii_lowercase();
    BLOCKED_SOURCES.contains(&s.as_str()) || BLOCKED_ORIGINS.contains(&o.as_str())
}
