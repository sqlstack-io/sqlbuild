from dataclasses import dataclass

from sqlbuild.adapter.contract.types import LogLevel


@dataclass(frozen=True)
class NodeLoggerTestCase:
    description: str
    level: LogLevel
    expected_python_level: int


@dataclass(frozen=True)
class NodeLoggerCallTestCase:
    description: str
    message: str
    expected_error: type[Exception]
