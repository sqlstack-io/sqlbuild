"""Parameterized user-code finder test cases."""

from dataclasses import dataclass


@dataclass(frozen=True)
class FinderSpecTestCase:
    """One module to import, whether an earlier path entry shadows it, and the expected loader."""

    description: str
    module_name: str
    earlier_entries: tuple[str, ...]
    expected_source_only: bool


@dataclass(frozen=True)
class FinderWorkingDirectoryTestCase:
    """A lookup through the working-directory path entry before and after changing directory."""

    description: str
    first_directory: str
    second_directory: str
    expected_source_only: tuple[bool, bool]


@dataclass(frozen=True)
class FinderDeletedDirectoryTestCase:
    """A lookup while the working directory, on the search path as "", has been deleted."""

    description: str
    module_name: str
    expected_spec: None = None
