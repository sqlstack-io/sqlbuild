"""Stable build feature alignment constants."""

SQB_PACKAGE: str = "sqlbuild-cli"
SQB_FEATURES: str = "ui"
EXTENSION_PACKAGE: str = "sqlbuild-python"
EXTENSION_FEATURES: str = "extension-module,sqlbuild-cli/ui"
WHEEL_TARGETS: tuple[str, ...] = (
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
)
TREE_FORMAT: str = "{p}|{f}"
TREE_INDENT_WIDTH: int = 4
TREE_GLYPHS: str = "│├└─ "
TREE_FIELD_SEPARATOR: str = "|"
BUILD_DEPENDENCIES_DEPTH_OFFSET: float = 0.5
BUILD_DEPENDENCIES_HEADING: str = "[build-dependencies]"
PROC_MACRO_MARKER: str = " (proc-macro)"
HOST_SIDE: str = "host"
TARGET_SIDE: str = "target"
FEATURE_CRATE: str = "crates/sqlbuild-build-features/Cargo.toml"
