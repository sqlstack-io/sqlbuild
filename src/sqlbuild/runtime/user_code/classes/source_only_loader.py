"""A source loader that neither reads nor writes cached bytecode."""

from importlib.machinery import SourceFileLoader
from types import CodeType
from typing import ClassVar


class SourceOnlyLoader(SourceFileLoader):
    """Compile a module from current source and count it; pyc checks miss same-size edits."""

    executions: ClassVar[int] = 0

    def get_code(self, fullname: str) -> CodeType:
        source_path: str = self.get_filename(fullname)
        SourceOnlyLoader.executions += 1
        return self.source_to_code(self.get_data(source_path), source_path)
