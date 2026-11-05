from dataclasses import dataclass

from fensu import RuleFile


@dataclass(frozen=True)
class CustomRuleTestCase:
    description: str
    path: str
    source: str
    expected_fault_count: int
    files: tuple[RuleFile, ...] = ()
    scope: str = "root"
    scope_root: str | None = "src/sqlbuild"


@dataclass(frozen=True)
class NativeLinesTestCase:
    description: str
    source: str
    expected_lines: tuple[int, ...]


@dataclass(frozen=True)
class NativePatchTestCase:
    description: str
    source: str
    expected_fault_count: int
    budgets: tuple[str, ...] = ()


@dataclass(frozen=True)
class NativeReferencesTestCase:
    description: str
    source: str
    expected_references: tuple[tuple[int, tuple[str, ...]], ...]


@dataclass(frozen=True)
class NativeLayerTestCase:
    description: str
    module: tuple[str, ...]
    expected_layer: str | None


@dataclass(frozen=True)
class NativeDirectionTestCase:
    description: str
    source: str
    target: str
    expected_upward: bool


@dataclass(frozen=True)
class NativeLayeringTestCase:
    description: str
    path: str
    source: str
    expected_lines: list[int] | None


@dataclass(frozen=True)
class UnreadStateTestCase:
    description: str
    schema: str
    rust: str
    expected_unread: tuple[tuple[str, str | None], ...]


@dataclass(frozen=True)
class EnumVariantTestCase:
    description: str
    files: tuple[tuple[str, str], ...]
    expected_unnamed: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class StrictVariantTestCase:
    description: str
    source: str
    expected_unnamed: tuple[str, ...]


@dataclass(frozen=True)
class ReasonedEntriesTestCase:
    description: str
    entries: tuple[str, ...]
    expected_kept: tuple[object, ...]
    expected_problem_count: int
