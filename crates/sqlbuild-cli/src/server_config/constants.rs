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

/// Units `[output] max_bytes_per_run` accepts, with the bytes each stands for.
pub(crate) const BYTE_UNITS: [(&str, u64); 10] = [
    ("", 1),
    ("B", 1),
    ("kB", 1_000),
    ("MB", 1_000_000),
    ("GB", 1_000_000_000),
    ("TB", 1_000_000_000_000),
    ("KiB", 1 << 10),
    ("MiB", 1 << 20),
    ("GiB", 1 << 30),
    ("TiB", 1 << 40),
];
