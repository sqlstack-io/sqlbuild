"""The setting scan reads Rust string literals, not the code around them."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.setting_help.main.find_settings_without_snippet import find_settings_without_snippet
from tests.unit.scripts.setting_help._helpers._test_types import RustSettingScanTestCase

_PATH: str = "crates/demo/src/lib.rs"


@pytest.mark.parametrize(
    "test_case",
    [
        RustSettingScanTestCase(
            description="a match pattern beside a literal names no setting",
            source='match outcome {\n    Done { healthy: true } => "connection healthy",\n}\n',
            expected_lines=(),
        ),
        RustSettingScanTestCase(
            description="a literal naming a setting loosely is found",
            source='let help = "set settings.require_sql_analysis to false";\n',
            expected_lines=(1,),
        ),
        RustSettingScanTestCase(
            description="a YAML-style setting in a literal is found",
            source='let help = "write `require_sql_analysis: false` in the project";\n',
            expected_lines=(1,),
        ),
        RustSettingScanTestCase(
            description="a comment is not diagnostic text",
            source='// set settings.require_sql_analysis to false\nlet quote = "plain";\n',
            expected_lines=(),
        ),
        RustSettingScanTestCase(
            description="a quote char literal doesn't open a string",
            source=(
                "let quote = '\"';\n"
                "let pattern = Done { healthy: true };\n"
                'let text = "set settings.require_sql_analysis to false";\n'
            ),
            expected_lines=(3,),
        ),
        RustSettingScanTestCase(
            description="a raw string is read whole",
            source='let help = r#"say "set settings.require_sql_analysis to false""#;\n',
            expected_lines=(1,),
        ),
        RustSettingScanTestCase(
            description="a literal over several lines is reported where it starts",
            source='let help = "first line,\n    set settings.require_sql_analysis to false";\n',
            expected_lines=(1,),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rust_source_when_scanning_for_settings_then_only_string_literals_count(
    test_case: RustSettingScanTestCase, tmp_path: Path
) -> None:
    source: Path = tmp_path / _PATH
    source.parent.mkdir(parents=True)
    _ = source.write_text(test_case.source, encoding="utf-8")
    (tmp_path / "src" / "sqlbuild").mkdir(parents=True)

    findings: list[str] = find_settings_without_snippet(tmp_path)

    assert tuple(int(finding.split(":")[1]) for finding in findings) == test_case.expected_lines
