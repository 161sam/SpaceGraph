//! Socket reachability classification (ADR-0012 §2) — pure + headless.
//!
//! Derives a socket's network exposure from its `local_addr`. Drives radial depth
//! in the viewer (outer shell = Public, core = Loopback) and the posture
//! attack-surface signal (D5, [`crate::posture`]). No render/Bevy dependency; the
//! informational color tint lives viewer-side (`render::spatial::exposure_tint`).

/// A socket's network reachability, derived from its `local_addr` (ADR-0012 §2).
/// Drives radial depth — Public on the outer shell, Loopback at the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exposure {
    Loopback,
    Lan,
    Public,
}

impl Exposure {
    /// Radial shell factor for layout placement (Public outermost, Loopback core).
    pub fn shell_factor(self) -> f32 {
        match self {
            Exposure::Public => 1.8,
            Exposure::Lan => 1.25,
            Exposure::Loopback => 1.0,
        }
    }

    /// Short label for the inspector tooltip.
    pub fn label(self) -> &'static str {
        match self {
            Exposure::Loopback => "loopback",
            Exposure::Lan => "LAN",
            Exposure::Public => "public",
        }
    }
}

/// Classify a socket `local_addr` into an exposure bucket. A wildcard listener
/// (`0.0.0.0` / `::`) is treated as Public (reachable on every interface); an
/// unparseable address is conservatively Public.
pub fn exposure_bucket(local_addr: &str) -> Exposure {
    use std::net::{IpAddr, Ipv6Addr};
    fn v6_is_lan(v6: &Ipv6Addr) -> bool {
        let seg0 = v6.segments()[0];
        // link-local fe80::/10 or unique-local fc00::/7
        (seg0 & 0xffc0) == 0xfe80 || (seg0 & 0xfe00) == 0xfc00
    }
    match local_addr.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => {
            if v4.is_loopback() {
                Exposure::Loopback
            } else if v4.is_unspecified() {
                Exposure::Public // 0.0.0.0 listens on all interfaces
            } else if v4.is_private() || v4.is_link_local() {
                Exposure::Lan
            } else {
                Exposure::Public
            }
        }
        Ok(IpAddr::V6(v6)) => {
            if v6.is_loopback() {
                Exposure::Loopback
            } else if v6.is_unspecified() {
                Exposure::Public // :: listens on all interfaces
            } else if v6_is_lan(&v6) {
                Exposure::Lan
            } else {
                Exposure::Public
            }
        }
        Err(_) => Exposure::Public,
    }
}

#[cfg(test)]
mod tests {
    use super::{exposure_bucket, Exposure};

    #[test]
    fn exposure_bucket_classifies_reachability() {
        assert_eq!(exposure_bucket("127.0.0.1"), Exposure::Loopback);
        assert_eq!(exposure_bucket("::1"), Exposure::Loopback);
        assert_eq!(exposure_bucket("10.0.0.5"), Exposure::Lan);
        assert_eq!(exposure_bucket("192.168.1.4"), Exposure::Lan);
        assert_eq!(exposure_bucket("172.16.0.9"), Exposure::Lan);
        assert_eq!(exposure_bucket("172.31.255.1"), Exposure::Lan);
        assert_eq!(exposure_bucket("169.254.1.1"), Exposure::Lan);
        assert_eq!(exposure_bucket("fc00::1"), Exposure::Lan);
        assert_eq!(exposure_bucket("fe80::1"), Exposure::Lan);
        assert_eq!(exposure_bucket("8.8.8.8"), Exposure::Public);
        assert_eq!(exposure_bucket("2606:4700:4700::1111"), Exposure::Public);
        assert_eq!(exposure_bucket("0.0.0.0"), Exposure::Public);
        assert_eq!(exposure_bucket("::"), Exposure::Public);
        // 172.32 is just outside the private 172.16/12 block.
        assert_eq!(exposure_bucket("172.32.0.1"), Exposure::Public);
    }

    #[test]
    fn exposure_shell_factor_orders_by_reachability() {
        assert!(Exposure::Public.shell_factor() > Exposure::Lan.shell_factor());
        assert!(Exposure::Lan.shell_factor() > Exposure::Loopback.shell_factor());
    }
}
