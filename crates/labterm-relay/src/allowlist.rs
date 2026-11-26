//! Which addresses and ports the relay may dial. Deny by default; some ranges are never allowed.

use ipnet::IpNet;
use serde::Deserialize;
use std::net::IpAddr;

/// Never dialled, whatever the allowlist says: the relay's own host, link-local (cloud metadata
/// lives there), unspecified and multicast. The cluster's pod and service ranges are added from config.
const ALWAYS_DENIED: &[&str] = &[
    "127.0.0.0/8",
    "::1/128",
    "0.0.0.0/8",
    "::/128",
    "169.254.0.0/16",
    "fe80::/10",
    "224.0.0.0/4",
    "ff00::/8",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowlistFile {
    #[serde(default)]
    pub deny: Vec<IpNet>,
    pub allow: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub cidr: IpNet,
    #[serde(default = "default_ports")]
    pub ports: Vec<u16>,
    /// When set, only members of one of these Authentik groups may use the rule.
    #[serde(default)]
    pub groups: Vec<String>,
}

fn default_ports() -> Vec<u16> {
    vec![22]
}

#[derive(Debug, thiserror::Error)]
pub enum AllowlistError {
    #[error("allowlist is not valid YAML: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("allowlist rule {0} has no ports")]
    NoPorts(IpNet),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Allowed,
    /// Always-denied or listed under `deny`.
    Denied,
    /// No rule matches the address, port and the user's groups.
    NotListed,
}

pub struct Allowlist {
    deny: Vec<IpNet>,
    allow: Vec<Rule>,
}

impl Allowlist {
    pub fn from_yaml(text: &str) -> Result<Self, AllowlistError> {
        let file: AllowlistFile = serde_yaml::from_str(text)?;
        if let Some(r) = file.allow.iter().find(|r| r.ports.is_empty()) {
            return Err(AllowlistError::NoPorts(r.cidr));
        }
        let mut deny: Vec<IpNet> = ALWAYS_DENIED
            .iter()
            .map(|s| s.parse().expect("static range"))
            .collect();
        deny.extend(file.deny);
        Ok(Self {
            deny,
            allow: file.allow,
        })
    }

    /// Lets the relay's own tests dial a loopback echo server. Never compiled into the binary.
    #[cfg(test)]
    pub(crate) fn allow_loopback_for_tests(&mut self) {
        let lo: IpNet = "127.0.0.0/8".parse().expect("static range");
        self.deny.retain(|n| *n != lo);
    }

    pub fn rule_count(&self) -> usize {
        self.allow.len()
    }

    /// Checked on the resolved address, never the name, so DNS can't widen it.
    pub fn check(&self, ip: IpAddr, port: u16, groups: &[String]) -> Verdict {
        // An IPv4-mapped IPv6 address is judged as the IPv4 address it carries.
        let ip = match ip {
            IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
            v4 => v4,
        };
        if self.deny.iter().any(|n| n.contains(&ip)) {
            return Verdict::Denied;
        }
        let ok = self.allow.iter().any(|r| {
            r.cidr.contains(&ip)
                && r.ports.contains(&port)
                && (r.groups.is_empty() || r.groups.iter().any(|g| groups.contains(g)))
        });
        if ok {
            Verdict::Allowed
        } else {
            Verdict::NotListed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = r#"
deny:
  - 192.0.2.64/27     # pods
allow:
  - cidr: 192.0.2.0/26
  - cidr: 192.0.2.128/25
    ports: [22, 2222]
    groups: [ssh-admins]
  - cidr: 192.0.2.64/26
"#;

    fn list() -> Allowlist {
        Allowlist::from_yaml(YAML).unwrap()
    }
    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }
    fn g(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn default_port_is_22_only() {
        assert_eq!(list().check(ip("192.0.2.10"), 22, &[]), Verdict::Allowed);
        assert_eq!(list().check(ip("192.0.2.10"), 80, &[]), Verdict::NotListed);
    }

    #[test]
    fn group_rules_need_the_group() {
        assert_eq!(
            list().check(ip("192.0.2.200"), 2222, &g(&["ssh-users"])),
            Verdict::NotListed
        );
        assert_eq!(
            list().check(ip("192.0.2.200"), 2222, &g(&["ssh-admins"])),
            Verdict::Allowed
        );
    }

    #[test]
    fn deny_wins_over_allow() {
        assert_eq!(list().check(ip("192.0.2.70"), 22, &[]), Verdict::Denied);
        assert_eq!(list().check(ip("192.0.2.100"), 22, &[]), Verdict::Allowed);
    }

    #[test]
    fn always_denied_ranges() {
        let wide = Allowlist::from_yaml("allow:\n  - cidr: 0.0.0.0/0\n  - cidr: ::/0\n").unwrap();
        for a in [
            "127.0.0.1",
            "169.254.169.254",
            "::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "0.0.0.0",
        ] {
            assert_eq!(wide.check(ip(a), 22, &[]), Verdict::Denied, "{a}");
        }
        assert_eq!(wide.check(ip("192.0.2.1"), 22, &[]), Verdict::Allowed);
    }

    #[test]
    fn rejects_bad_files() {
        assert!(Allowlist::from_yaml("allow:\n  - cidr: 192.0.2.0/24\n    ports: []\n").is_err());
        assert!(Allowlist::from_yaml("allow:\n  - cidr: nonsense\n").is_err());
        assert!(Allowlist::from_yaml("allow: []\nextra: 1\n").is_err());
    }
}
