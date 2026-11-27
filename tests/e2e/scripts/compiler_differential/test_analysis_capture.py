"""Real compiles capture every compiled-project field canonically and prove every analysis kind."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

from scripts.compiler_differential._helpers.coverage.analysis import (
    analysed_input_kinds,
    analysis_capture_problems,
    required_analysis_kinds,
)
from scripts.compiler_differential._helpers.running.execution import harness_environment
from scripts.compiler_differential.constants import (
    ANALYSIS_STAGE_CAPTURE_SUFFIX,
    GENERATOR_DIALECT_BLOCKS,
    GENERATOR_FEATURE_BLOCKS,
    SQB_ENTRY,
    STAGE_CAPTURE_ENV_VAR,
)
from scripts.compiler_differential.main.generate_project import generate_project
from scripts.compiler_differential.models import GeneratedProject
from tests.e2e.scripts.compiler_differential._test_types import AnalysisCaptureTestCase

# dbt_ref fails compile without its manifest; macro_generated_reference turns off the explicit
# references that Python SQL relation checks need; project_adapter drops the loader and load
# blocks (they write only to built-in adapters) and analyses nothing itself.
_CONFLICTING_BLOCKS: frozenset[str] = frozenset(
    {"dbt_ref", "macro_generated_reference", "project_adapter"}
)
_OTHER_DIALECT_KINDS: frozenset[str] = frozenset(
    {"dialect_postgresql", "dialect_snowflake", "managed_write_schema"}
)


@pytest.mark.parametrize(
    "test_case",
    [
        AnalysisCaptureTestCase(
            description="every_compiling_feature_block_in_every_analysis_mode",
            seed=11,
            blocks=tuple(
                block for block in GENERATOR_FEATURE_BLOCKS if block not in _CONFLICTING_BLOCKS
            ),
            dialect=None,
            commands=(("compile", "--json"), ("plan", "--json")),
            generated_command_count=3,
            expected_kinds=frozenset(required_analysis_kinds()) - _OTHER_DIALECT_KINDS,
            expected_absent=_OTHER_DIALECT_KINDS,
        ),
        AnalysisCaptureTestCase(
            description="analysis_blocks_compiled_under_snowflake",
            seed=11,
            blocks=GENERATOR_DIALECT_BLOCKS,
            dialect="snowflake",
            commands=(("compile", "--json"),),
            generated_command_count=0,
            expected_kinds=frozenset(
                {"dialect_snowflake", "managed_write_schema", "upstream_chain", "typed_column"}
            ),
            expected_absent=frozenset({"dialect_duckdb", "dialect_postgresql"}),
        ),
        AnalysisCaptureTestCase(
            description="analysis_blocks_compiled_under_postgres",
            seed=11,
            blocks=GENERATOR_DIALECT_BLOCKS,
            dialect="postgres",
            commands=(("compile", "--json"),),
            generated_command_count=0,
            expected_kinds=frozenset({"dialect_postgresql", "sized_contract_shape", "scalar_udf"}),
            expected_absent=frozenset({"dialect_duckdb", "managed_write_schema"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_with_every_analysis_input_when_capturing_then_capture_is_complete(
    test_case: AnalysisCaptureTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "project"
    generated: GeneratedProject = generate_project(
        seed=test_case.seed, blocks=test_case.blocks, dialect=test_case.dialect
    )
    generated.write(project_dir)
    generated_commands: tuple[tuple[str, ...], ...] = tuple(
        command.arguments for command in generated.extra_commands
    )[: test_case.generated_command_count]
    commands: tuple[tuple[str, ...], ...] = (*test_case.commands, *generated_commands)

    kinds: set[str] = set()
    for index, command in enumerate(commands):
        capture_dir: Path = tmp_path / f"captures-{index}"
        completed: subprocess.CompletedProcess[str] = subprocess.run(
            [sys.executable, "-c", SQB_ENTRY, "--no-color", *command],
            cwd=project_dir,
            env={**harness_environment(), STAGE_CAPTURE_ENV_VAR: str(capture_dir)},
            capture_output=True,
            text=True,
            check=False,
            timeout=600,
        )
        assert completed.returncode == 0, (command, completed.stdout + completed.stderr)
        text: str = next(capture_dir.glob(f"*{ANALYSIS_STAGE_CAPTURE_SUFFIX}")).read_text(
            encoding="utf-8"
        )
        assert analysis_capture_problems(text) == (), command
        kinds.update(analysed_input_kinds(text))

    assert len(generated_commands) == test_case.generated_command_count
    assert test_case.expected_kinds <= kinds, test_case.expected_kinds - kinds
    assert not test_case.expected_absent & kinds, test_case.expected_absent & kinds


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
