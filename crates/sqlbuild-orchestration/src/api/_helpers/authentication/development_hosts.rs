//! The `Host` and `Origin` values a sign-in-free development listener accepts.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::api::constants::{HTTP_PORT, HTTP_SCHEME, HTTPS_PORT, HTTPS_SCHEME, LOOPBACK_HOST};
use crate::api::models::DevelopmentAuthorities;

/// The listener's local names, then each allowed name on its own port, or on the listener's and the default ports.
pub(crate) fn development_authorities(
    address: SocketAddr,
    allowed: &[String],
) -> DevelopmentAuthorities {
    let port: u16 = address.port();
    let mut hosts: Vec<String> = match address.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => vec![Ipv4Addr::LOCALHOST.into()],
        IpAddr::V6(ip) if ip.is_unspecified() => {
            vec![Ipv6Addr::LOCALHOST.into(), Ipv4Addr::LOCALHOST.into()]
        }
        ip => vec![ip],
    }
    .into_iter()
    .map(|ip: IpAddr| SocketAddr::new(ip, port).to_string())
    .collect();
    hosts.push(format!("{LOOPBACK_HOST}:{port}"));
    let mut origins: Vec<String> = hosts
        .iter()
        .map(|host| format!("{HTTP_SCHEME}://{host}"))
        .collect();
    for entry in allowed.iter().map(|entry| entry.to_ascii_lowercase()) {
        let (name, only): (&str, Option<u16>) = split_port(&entry);
        let ports: Vec<u16> =
            only.map_or_else(|| vec![port, HTTP_PORT, HTTPS_PORT], |only| vec![only]);
        for port in ports {
            let full: String = format!("{name}:{port}");
            for (scheme, default) in [(HTTP_SCHEME, HTTP_PORT), (HTTPS_SCHEME, HTTPS_PORT)] {
                if port == default {
                    hosts.push(name.to_owned());
                    origins.push(format!("{scheme}://{name}"));
                } else if port != HTTP_PORT && port != HTTPS_PORT {
                    origins.push(format!("{scheme}://{full}"));
                }
            }
            hosts.push(full);
        }
    }
    DevelopmentAuthorities { hosts, origins }
}

/// `name:port` apart; a bracketed IPv6 address without a port has none.
fn split_port(entry: &str) -> (&str, Option<u16>) {
    match entry
        .rsplit_once(':')
        .map(|(name, port)| (name, port.parse()))
    {
        Some((name, Ok(port))) => (name, Some(port)),
        _ => (entry, None),
    }
}
