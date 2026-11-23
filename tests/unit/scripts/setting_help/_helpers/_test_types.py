from dataclasses import dataclass


@dataclass(frozen=True)
class RustSettingScanTestCase:
    description: str
    source: str
    expected_lines: tuple[int, ...]
