"""Native module path policy constants; crates/README.md describes the rules."""

import re

SCANNED_SOURCE_ROOTS: tuple[str, ...] = (
    "crates/sqlbuild-python-base/src",
    "crates/sqlbuild-python-front/src",
    "crates/sqlbuild-python-project/src",
    "crates/sqlbuild-python/src",
    "crates/sqlbuild-cli/src",
    "crates/sqlbuild-host/src",
)
RUST_SOURCE_GLOB: str = "*.rs"
PACKAGE_LITERAL: re.Pattern[str] = re.compile(r'"(sqlbuild\.[^"\\]*)"')
DOTTED_PATH: re.Pattern[str] = re.compile(r"sqlbuild(?:\.[A-Za-z_][A-Za-z0-9_]*)+")
STRING_CONSTANT: re.Pattern[str] = re.compile(
    r"\bconst\s+([A-Z][A-Z0-9_]*)\s*:\s*&(?:'static\s+)?str\s*=\s*\"(sqlbuild\.[^\"\\]*)\"\s*;"
)
LOGGER_CONSTANT_NAME: re.Pattern[str] = re.compile(r"LOGGER")
ATTRIBUTE_NAME: str = r"\"([A-Za-z_][A-Za-z0-9_]*)\""
MODULE_ARGUMENT: str = r"([A-Z][A-Z0-9_]*|\"sqlbuild\.[^\"\\]*\")"
IMPORT_HELPER_CALL: re.Pattern[str] = re.compile(
    r"\b[a-z_][a-z0-9_]*\(\s*py\s*,\s*"
    + MODULE_ARGUMENT
    + r"\s*,\s*"
    + ATTRIBUTE_NAME
    + r"\s*,?\s*\)"
)
IMPORT_GETATTR_CHAIN: re.Pattern[str] = re.compile(
    r"\bimport\(\s*" + MODULE_ARGUMENT + r"\s*\)\?\s*\.getattr\(\s*" + ATTRIBUTE_NAME + r"\s*\)"
)
LITERAL_MODULE_ATTRIBUTE_PAIR: re.Pattern[str] = re.compile(
    r"\(\s*\"(sqlbuild\.[^\"\\]*)\"\s*,\s*" + ATTRIBUTE_NAME + r"\s*,?\s*\)"
)
DYNAMIC_MODULE_PATHS: tuple[re.Pattern[str], ...] = (
    re.compile(r"\b(?:format|concat)!\(\s*\"\{[A-Z0-9_]*(?:PACKAGE|MODULE)[A-Z0-9_]*\}\."),
    re.compile(
        r"\b(?:format|concat)!\(\s*\"\{\}\.[^\"]*\"\s*,\s*[A-Z0-9_]*(?:PACKAGE|MODULE)[A-Z0-9_]*\b"
    ),
    re.compile(r"\b(?:format|concat)!\(\s*\"sqlbuild\."),
)
HOST_MODULE: str = "sqlbuild._native_host"
HOST_ENTRY: re.Pattern[str] = re.compile(r"\bHostEntry\s*\{\s*name:\s*" + ATTRIBUTE_NAME)
HOST_ROOT_REASON: str = "Native code resolves these through the sqlbuild._native_host name table."
MODULE_FILE_ATTRIBUTE: str = "__file__"
PYTHON_SUFFIX: str = ".py"
