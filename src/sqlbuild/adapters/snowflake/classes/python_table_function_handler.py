"""The generated partition handler of a Snowflake Python table function."""

from __future__ import annotations

import re

from sqlbuild.adapters.snowflake.constants import (
    PANDAS_PACKAGES,
    POLARS_DATAFRAME,
    POLARS_PACKAGES,
    PYTHON_TABLE_FUNCTION_HANDLER,
)

_PACKAGE_NAME: re.Pattern[str] = re.compile(r"^\s*([A-Za-z0-9_.\-]+)")


class PythonTableFunctionHandler:
    """A vectorized handler that calls a user's entry point once per partition."""

    def __init__(
        self,
        *,
        body: str,
        entry_point: str,
        argument_names: tuple[str, ...],
        column_names: tuple[str, ...],
        dataframe: str,
    ) -> None:
        self.body: str = body
        self.entry_point: str = entry_point
        self.argument_names: tuple[str, ...] = argument_names
        self.column_names: tuple[str, ...] = column_names
        self.dataframe: str = dataframe

    def packages(self, *, declared: tuple[str, ...]) -> tuple[str, ...]:
        """The declared packages, then each package the partition frame needs that none names."""

        required: tuple[str, ...] = (
            POLARS_PACKAGES if self.dataframe == POLARS_DATAFRAME else PANDAS_PACKAGES
        )
        named: set[str] = {
            match.group(1).lower()
            for package in declared
            if (match := _PACKAGE_NAME.match(package)) is not None
        }
        return (*declared, *(package for package in required if package not in named))

    def module(self) -> str:
        """The user's module followed by the handler class."""

        polars: bool = self.dataframe == POLARS_DATAFRAME
        imports: list[str] = ["import pandas as _sqb_pandas"]
        if polars:
            imports.append("import polars as _sqb_polars")
        imports.append("from _snowflake import vectorized as _sqb_vectorized")
        frame: str = "_sqb_polars.from_pandas(_sqb_partition)" if polars else "_sqb_partition"
        output: str = (
            "_sqb_result.select(_sqb_expected).to_pandas()"
            if polars
            else "_sqb_result[_sqb_expected]"
        )
        lines: list[str] = [
            self.body,
            "",
            *imports,
            "",
            "",
            f"class {PYTHON_TABLE_FUNCTION_HANDLER}:",
            "    @_sqb_vectorized(input=_sqb_pandas.DataFrame)",
            "    def end_partition(self, _sqb_partition):",
            f"        _sqb_partition.columns = {list(self.argument_names)!r}",
            f"        _sqb_result = {self.entry_point}({frame})",
            "        _sqb_returned = list(_sqb_result.columns)",
            f"        _sqb_expected = {list(self.column_names)!r}",
            "        _sqb_missing = [name for name in _sqb_expected if name not in _sqb_returned]",
            "        _sqb_extra = [name for name in _sqb_returned if name not in _sqb_expected]",
            "        _sqb_repeated = sorted(",
            "            {name for name in _sqb_returned if _sqb_returned.count(name) > 1}",
            "        )",
            "        if _sqb_missing or _sqb_extra or _sqb_repeated:",
            "            raise ValueError(",
            f'                f"table function {self.entry_point} returned columns "',
            '                f"{_sqb_returned}, but its "',
            '                f"returns declare {_sqb_expected}: missing {_sqb_missing}, "',
            '                f"extra {_sqb_extra}, repeated {_sqb_repeated}"',
            "            )",
            f"        return {output}",
        ]
        return "\n".join(lines)
