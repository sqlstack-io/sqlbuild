//! Where `sqb dev` listens, the names it also takes, and whom that opens it to.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use sqlbuild_orchestration::api::models::DevelopmentScope;
use sqlbuild_orchestration::store::errors::ControlError;

use crate::argv::models::Namespace;
use crate::native_commands::_helpers::command::decision::{control_address, text, texts};
use crate::server_config::models::ServerSection;

/// The address `sqb dev` listens on: loopback, unless `--host` says otherwise.
pub(crate) fn development_address(namespace: &Namespace) -> Result<SocketAddr, ControlError> {
    control_address(namespace, false, &ServerSection::default())?
        .ok_or_else(|| ControlError::Invalid("development address is required".into()))
}

/// Only an explicit `--host` lets the sign-in-free API leave loopback.
pub(crate) fn listens_beyond_loopback(namespace: &Namespace) -> bool {
    text(namespace, "control_host").is_some()
}

/// The `--allow-host` names in lower case, IPv6 addresses in brackets.
pub(crate) fn allowed_hosts(namespace: &Namespace) -> Result<Vec<String>, ControlError> {
    texts(namespace, "development_allow_host")
        .unwrap_or_default()
        .iter()
        .map(|value| allowed_host(value))
        .collect()
}

/// One `--allow-host` value as `name` or `name:port`.
fn allowed_host(value: &str) -> Result<String, ControlError> {
    let refused = || {
        ControlError::Invalid(format!(
            "--allow-host {value:?} must be a name or IP address, optionally with :port, without \
             a scheme, path or wildcard"
        ))
    };
    let entry: String = value.to_ascii_lowercase();
    if let Ok(ip) = entry.parse::<Ipv6Addr>() {
        return Ok(format!("[{ip}]"));
    }
    let (name, port): (&str, Option<&str>) = match entry.rsplit_once(':') {
        Some((name, port)) if !port.ends_with(']') => (name, Some(port)),
        _ => (&entry, None),
    };
    if let Some(port) = port {
        let _: std::num::NonZeroU16 = port.parse().map_err(|_| refused())?;
    }
    if let Some(address) = name
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        let _: Ipv6Addr = address.parse().map_err(|_| refused())?;
    } else if name.is_empty()
        || !name
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || "-._".contains(letter))
    {
        return Err(refused());
    }
    Ok(entry)
}

/// An address this machine can connect to: loopback when listening on every interface.
pub(crate) fn connect_address(address: SocketAddr) -> SocketAddr {
    let ip: IpAddr = match address.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => Ipv4Addr::LOCALHOST.into(),
        IpAddr::V6(ip) if ip.is_unspecified() => Ipv6Addr::LOCALHOST.into(),
        ip => ip,
    };
    SocketAddr::new(ip, address.port())
}

/// Who gets in without signing in; `None` for a loopback-only session.
pub(crate) fn exposure(address: SocketAddr, scope: &DevelopmentScope) -> Option<String> {
    let listener: Option<String> = (!address.ip().is_loopback()).then(|| address.to_string());
    let reached: Vec<String> = listener
        .into_iter()
        .chain(scope.allowed_hosts.iter().cloned())
        .collect();
    (!reached.is_empty()).then(|| {
        format!(
            "anyone who can reach {} has full, sign-in-free access with your credentials",
            reached.join(" or ")
        )
    })
}
