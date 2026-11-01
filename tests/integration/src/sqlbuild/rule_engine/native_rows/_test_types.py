from dataclasses import dataclass


@dataclass(frozen=True)
class SqlRulePublicationTestCase:
    """Paths whose lint findings are published to the SQL rule cache within a time limit."""

    description: str
    sizes: tuple[int, ...]
    expected_max_seconds: float


@dataclass(frozen=True)
class NativeRulesRequestTestCase:
    """A compile of the rules fixture: findings and cache use."""

    description: str
    expected_cold_cache: tuple[int, int]
    expected_warm_cache: tuple[int, int]
    expected_edited_cache: tuple[int, int]


@dataclass(frozen=True)
class NativeTypeProofTestCase:
    """A type-proof findings for a contract with matching and mismatched casts."""

    description: str
    expected_findings: tuple[tuple[str, str, int | None, int | None, str], ...]


@dataclass(frozen=True)
class StructPassthroughTestCase:
    """A STRUCT passthrough whose type proof asks the adapter's `types_equal` callback."""

    description: str
    declared_type: str
    expected_exit_code: int
    expected_codes: tuple[str, ...]


@dataclass(frozen=True)
class SqlRuleCompileVarsTestCase:
    """A compile's effective vars and whether SQL Rules report the NULL comparison they expand."""

    description: str
    effective_vars: dict[str, object]
    expected_codes: tuple[str, ...]
