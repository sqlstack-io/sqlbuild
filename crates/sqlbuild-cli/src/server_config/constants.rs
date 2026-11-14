//! Flags the configuration file replaced, each with the section that now holds its settings.

pub(super) const REPLACED_SERVER_FLAGS: [(&str, &str, &str); 3] = [
    (
        "control_launchers",
        "--launchers",
        "[[launchers.docker]] or [[launchers.local]]",
    ),
    ("control_oidc_config", "--oidc-config", "[oidc]"),
    (
        "control_trusted_proxy_config",
        "--trusted-proxy-config",
        "[trusted_proxy]",
    ),
];
