"""Patterns for diagnostic text that names a setting without its exact TOML snippet."""

from __future__ import annotations

import re

SNIPPET_PATTERN: re.Pattern[str] = re.compile(
    r"^ +\[([a-z_.]+)\]\n +([a-z_]+) = (.+)$", re.MULTILINE
)
YAML_STYLE_SETTING_PATTERN: re.Pattern[str] = re.compile(
    r"`?\b(?:settings\.)?[a-z_]+:\s*(?:true|false|staged)\b|`[a-z_]+: [a-z_]+`"
    r"|\[(?:dbt|settings|references|rules|janitor|defaults)\]\.[a-z_]+"
)
DOTTED_SETTING_INSTRUCTION_PATTERN: re.Pattern[str] = re.compile(
    r"\b(?:[Ss]et|[Ee]nable|[Dd]isable|[Aa]dd|requires|allow)\b[^.;\n\"]{0,60}?"
    r"\b(?:settings|references|janitor|dbt|rules|connection|defaults)\.[a-z_]+"
)
YAML_FILE_TEXT_MODULES: frozenset[str] = frozenset(
    {
        "crates/sqlbuild-discovery/src/yaml_entries/_helpers/removed_integrations.rs",
        "crates/sqlbuild-discovery/src/yaml_entries/_helpers/sources.rs",
    }
)
RUST_TEST_PATH_MARKER: str = "/tests/"
RUST_TOKEN_PATTERN: re.Pattern[str] = re.compile(
    r"//[^\n]*"
    r"|/\*.*?\*/"
    r'|(?<![A-Za-z0-9_])[bc]?r(?P<hashes>#*)"(?P<raw>.*?)"(?P=hashes)'
    r'|(?<![A-Za-z0-9_])[bc]?"(?P<text>(?:\\.|[^"\\])*)"'
    r"|(?<![A-Za-z0-9_])b?'(?:\\u\{[0-9a-fA-F]{1,6}\}|\\.|[^\\'\n])'",
    re.DOTALL,
)
