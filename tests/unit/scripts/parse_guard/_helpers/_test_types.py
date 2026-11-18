from dataclasses import dataclass


@dataclass(frozen=True)
class UnguardedSiteTestCase:
    description: str
    text: str
    expected_sites: tuple[str, ...]


@dataclass(frozen=True)
class RepositoryParseGuardTestCase:
    description: str
    expected_errors: tuple[str, ...]
