"""A meta-path finder that loads modules under registered project roots from source."""

import os
import sys
import sysconfig
from collections.abc import Sequence
from importlib.machinery import ModuleSpec, PathFinder, SourceFileLoader
from pathlib import Path
from types import ModuleType

from sqlbuild.runtime.user_code.classes.source_only_loader import SourceOnlyLoader

_INSTALLED_PACKAGE_DIRECTORIES: frozenset[str] = frozenset({"site-packages", "dist-packages"})


class UserCodeFinder:
    """Load modules found under a project root from source; search only project entries first."""

    def __init__(self) -> None:
        self.roots: set[Path] = set()
        self.installed: tuple[Path, ...] = tuple(
            Path(location).resolve()
            for location in {
                sys.prefix,
                sys.base_prefix,
                sys.exec_prefix,
                *sysconfig.get_paths().values(),
            }
        )
        self._project_entries: dict[tuple[str, str], bool] = {}
        self._entry_roots: frozenset[Path] = frozenset()

    def find_spec(
        self,
        fullname: str,
        path: Sequence[str] | None = None,
        target: ModuleType | None = None,
    ) -> ModuleSpec | None:
        entries: Sequence[str] = sys.path if path is None else path
        project_entries: list[str] = [entry for entry in entries if self._is_project_entry(entry)]
        if not project_entries:
            return None
        if PathFinder.find_spec(fullname, project_entries, target) is None:
            return None
        spec: ModuleSpec | None = PathFinder.find_spec(fullname, path, target)
        if spec is None or spec.origin is None or not isinstance(spec.loader, SourceFileLoader):
            return None
        origin: Path = Path(spec.origin)
        if not any(origin.is_relative_to(root) for root in tuple(self.roots)):
            return None
        if _INSTALLED_PACKAGE_DIRECTORIES.intersection(origin.parts) or any(
            origin.is_relative_to(location) for location in self.installed
        ):
            return None
        spec.loader = SourceOnlyLoader(fullname, spec.origin)
        return spec

    def _is_project_entry(self, entry: str) -> bool:
        roots: frozenset[Path] = frozenset(self.roots)
        if roots != self._entry_roots:
            self._project_entries = {}
            self._entry_roots = roots
        try:
            directory: str = "" if os.path.isabs(entry) else os.getcwd()
        except OSError:
            return False
        known: bool | None = self._project_entries.get((entry, directory))
        if known is not None:
            return known
        try:
            resolved: Path = Path(entry or ".").resolve()
        except OSError:
            resolved = Path(entry)
        under_root: bool = any(
            resolved.is_relative_to(root) or root.is_relative_to(resolved) for root in roots
        )
        self._project_entries[(entry, directory)] = under_root
        return under_root
