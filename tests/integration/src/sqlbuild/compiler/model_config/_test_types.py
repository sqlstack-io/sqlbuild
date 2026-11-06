"""Test case types for the native model configuration parity tests."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class HeaderMetadataParityTestCase:
    """Seeded MODEL header metadata parsed natively and in Python."""

    description: str
    seed: int
    count: int
    expected_minimum_parsed: int
    expected_minimum_rejected: int
    expected_maximum_later_validated: int


@dataclass(frozen=True)
class HeaderAuditArgumentsTestCase:
    """Python-supplied audit arguments and the compiled audit targets they produce."""

    description: str
    header: dict[object, object]
    expected_audits: list[tuple[str, str | None]]


@dataclass(frozen=True)
class ModelConfigTierTestCase:
    """How often the model loop calls each native model config entry."""

    description: str
    expected_native_calls: dict[str, int]


@dataclass(frozen=True)
class ColumnMetadataExpansionTestCase:
    """A header column field holding a config template or macro, and the loop's outcome."""

    description: str
    header_column: str
    expected_type: str | None
    expected_error: str | None


@dataclass(frozen=True)
class ConfigValueBuildTestCase:
    """A Python-given MODEL header and the config values the build gives the model."""

    description: str
    header: dict[object, object]
    expected_values: tuple[tuple[str, str, str], ...]
    expected_header_keys: tuple[str, ...]


@dataclass(frozen=True)
class ConfigValueRejectionTestCase:
    """A Python-given MODEL header the config build rejects, and the error it raises."""

    description: str
    header: dict[object, object]
    expected_error: tuple[str, str]


@dataclass(frozen=True)
class OpaqueConfigValueTestCase:
    """A MODEL header holding an opaque value under `key`, which reaches the config unchanged."""

    description: str
    header: dict[object, object]
    key: object
    expected_same_object: bool


@dataclass(frozen=True)
class HeaderColumnPathTestCase:
    """MODEL headers with columns, and how many models reuse or reparse their native columns."""

    description: str
    headers: tuple[str, ...]
    expected_native: int
    expected_parsed: int


@dataclass(frozen=True)
class TemplateTextTestCase:
    """A MODEL header and CLI vars whose templated alias reads values as Python's text."""

    description: str
    header: dict[object, object]
    cli_vars: dict[str, object]
    expected_outcome: tuple[str, str]


@dataclass(frozen=True)
class OpaqueReadsTestCase:
    """Whether an opaque header value reaches the config, and which of its texts compiling reads."""

    description: str
    expected_reads: tuple[bool, tuple[str, ...]]
