//! Configuration from the environment. Secrets come from files (`*_FILE`), never from variables.

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub listen: String,
    pub allowlist_file: String,
    /// Ed25519 seed for tickets, 32 bytes base64. Unset: a fresh key per start, which only works
    /// with a single replica.
    pub ticket_key_file: Option<String>,
    pub oidc_issuer: String,
    pub oidc_audience: String,
    pub oidc_groups_claim: String,
    /// The web app's origins, exact (`https://hatch.example.com`).
    pub allowed_origins: Vec<String>,
    pub limits: Limits,
}

#[derive(Debug, Clone)]
pub struct Limits {
    pub ticket_seconds: u64,
    pub tickets_per_minute: u32,
    pub sessions_per_user: usize,
    pub sessions_total: usize,
    pub dial_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_session: Duration,
    /// Largest WebSocket message the browser may send.
    pub max_message: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            ticket_seconds: 30,
            tickets_per_minute: 30,
            sessions_per_user: 20,
            sessions_total: 500,
            dial_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(30 * 60),
            max_session: Duration::from_secs(12 * 3600),
            max_message: 256 * 1024,
        }
    }
}

type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

fn req(env: Env, name: &str) -> Result<String, String> {
    env(name)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| format!("{name} is not set"))
}

fn num<T: std::str::FromStr>(env: Env, name: &str, default: T) -> Result<T, String> {
    match env(name) {
        None => Ok(default),
        Some(v) => v
            .trim()
            .parse()
            .map_err(|_| format!("{name} is not a number: {v:?}")),
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(&|k| std::env::var(k).ok())
    }

    pub fn from_lookup(env: Env) -> Result<Self, String> {
        let d = Limits::default();
        let ticket_seconds = num(env, "HATCH_RELAY_TICKET_SECONDS", d.ticket_seconds)?;
        if ticket_seconds == 0 || ticket_seconds > hatch_proto::MAX_TICKET_SECONDS {
            return Err(format!(
                "HATCH_RELAY_TICKET_SECONDS must be 1 to {}",
                hatch_proto::MAX_TICKET_SECONDS
            ));
        }
        let allowed_origins: Vec<String> = req(env, "HATCH_RELAY_ALLOWED_ORIGINS")?
            .split(',')
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if let Some(bad) = allowed_origins
            .iter()
            .find(|o| !(o.starts_with("https://") || o.starts_with("http://localhost")))
        {
            return Err(format!(
                "HATCH_RELAY_ALLOWED_ORIGINS: {bad:?} must be https (or http://localhost for development)"
            ));
        }
        let issuer = req(env, "HATCH_RELAY_OIDC_ISSUER")?;
        Ok(Self {
            listen: env("HATCH_RELAY_LISTEN").unwrap_or_else(|| "0.0.0.0:8080".into()),
            allowlist_file: req(env, "HATCH_RELAY_ALLOWLIST_FILE")?,
            ticket_key_file: env("HATCH_RELAY_TICKET_KEY_FILE").filter(|v| !v.is_empty()),
            oidc_issuer: issuer,
            oidc_audience: req(env, "HATCH_RELAY_OIDC_AUDIENCE")?,
            oidc_groups_claim: env("HATCH_RELAY_OIDC_GROUPS_CLAIM")
                .unwrap_or_else(|| "groups".into()),
            allowed_origins,
            limits: Limits {
                ticket_seconds,
                tickets_per_minute: num(
                    env,
                    "HATCH_RELAY_TICKETS_PER_MINUTE",
                    d.tickets_per_minute,
                )?,
                sessions_per_user: num(env, "HATCH_RELAY_SESSIONS_PER_USER", d.sessions_per_user)?,
                sessions_total: num(env, "HATCH_RELAY_SESSIONS_TOTAL", d.sessions_total)?,
                dial_timeout: Duration::from_secs(num(
                    env,
                    "HATCH_RELAY_DIAL_TIMEOUT_SECONDS",
                    d.dial_timeout.as_secs(),
                )?),
                idle_timeout: Duration::from_secs(num(
                    env,
                    "HATCH_RELAY_IDLE_TIMEOUT_SECONDS",
                    d.idle_timeout.as_secs(),
                )?),
                max_session: Duration::from_secs(num(
                    env,
                    "HATCH_RELAY_MAX_SESSION_SECONDS",
                    d.max_session.as_secs(),
                )?),
                max_message: d.max_message,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| m.get(k).cloned()
    }

    const BASE: &[(&str, &str)] = &[
        ("HATCH_RELAY_ALLOWLIST_FILE", "/etc/hatch/allowlist.yaml"),
        (
            "HATCH_RELAY_OIDC_ISSUER",
            "https://auth.example.com/application/o/hatch/",
        ),
        ("HATCH_RELAY_OIDC_AUDIENCE", "hatch"),
        (
            "HATCH_RELAY_ALLOWED_ORIGINS",
            "https://hatch.example.com/, https://hatch2.example.com",
        ),
    ];

    #[test]
    fn defaults() {
        let c = Config::from_lookup(&env(BASE)).unwrap();
        assert_eq!(
            c.allowed_origins,
            ["https://hatch.example.com", "https://hatch2.example.com"]
        );
        assert_eq!(c.limits.ticket_seconds, 30);
        assert_eq!(c.oidc_groups_claim, "groups");
    }

    #[test]
    fn rejects_missing_and_bad_values() {
        assert!(Config::from_lookup(&env(&BASE[..3])).is_err());
        let mut v = BASE.to_vec();
        v.push(("HATCH_RELAY_TICKET_SECONDS", "600"));
        assert!(Config::from_lookup(&env(&v)).is_err());
        let mut v = BASE.to_vec();
        v[3] = ("HATCH_RELAY_ALLOWED_ORIGINS", "http://hatch.example.com");
        assert!(Config::from_lookup(&env(&v)).is_err());
    }
}
