"""Integration coverage for the built-in rules request built natively from compiled rows."""

from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.rule_engine.native_rows._test_types import (
    NativeRulesRequestTestCase,
    NativeTypeProofTestCase,
    StructPassthroughTestCase,
)
from tests.integration.src.sqlbuild.rule_engine.native_rows.helpers import (
    TYPE_PROOF_FILES,
    FindingKey,
    compile_codes,
    compile_rules,
    edit_model,
    struct_passthrough_files,
    write_files,
    write_rules_fixture,
)

_TYPE_PROOF_FINDINGS: tuple[FindingKey, ...] = (
    (
        "K002",
        "sql/models/orders.sql",
        2,
        37,
        "column 'amount' inferred as INTEGER but declared type is BIGINT",
    ),
    (
        "SQBRCONTRACT105",
        "sql/models/orders.sql",
        5,
        18,
        'output "amount" is a passthrough whose type is not proven against the declared '
        "contract; declared type is BIGINT",
    ),
)


@pytest.mark.parametrize(
    "test_case",
    [
        NativeRulesRequestTestCase(
            description="compile builds every request row natively",
            expected_cold_cache=(0, 26),
            expected_warm_cache=(26, 0),
            expected_edited_cache=(24, 2),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rules_fixture_when_compiling_cold_warm_and_edited_then_findings_match_fresh(
    test_case: NativeRulesRequestTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    fresh_dir: Path = tmp_path / "fresh"
    fresh_edited_dir: Path = tmp_path / "fresh_edited"
    project_dir: Path = tmp_path / "project"
    write_rules_fixture(project_dir=fresh_dir)
    write_rules_fixture(project_dir=fresh_edited_dir)
    edit_model(project_dir=fresh_edited_dir)
    write_rules_fixture(project_dir=project_dir)
    fresh: tuple[FindingKey, ...] = compile_rules(project_dir=fresh_dir, capsys=capsys)[0]
    fresh_edited: tuple[FindingKey, ...] = compile_rules(
        project_dir=fresh_edited_dir, capsys=capsys
    )[0]

    cold: tuple[tuple[FindingKey, ...], tuple[int, int]] = compile_rules(
        project_dir=project_dir, capsys=capsys
    )
    warm: tuple[tuple[FindingKey, ...], tuple[int, int]] = compile_rules(
        project_dir=project_dir, capsys=capsys
    )
    edit_model(project_dir=project_dir)
    edited: tuple[tuple[FindingKey, ...], tuple[int, int]] = compile_rules(
        project_dir=project_dir, capsys=capsys
    )

    assert (cold[0], warm[0], edited[0]) == (fresh, fresh, fresh_edited)
    assert (cold[1], warm[1], edited[1]) == (
        test_case.expected_cold_cache,
        test_case.expected_warm_cache,
        test_case.expected_edited_cache,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        NativeTypeProofTestCase(
            description="compile proves passthrough types natively",
            expected_findings=_TYPE_PROOF_FINDINGS,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_matching_and_mismatched_passthroughs_when_compiling_then_only_mismatch_is_found(
    test_case: NativeTypeProofTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    write_files(project_dir=tmp_path, files=TYPE_PROOF_FILES)

    findings: tuple[FindingKey, ...] = compile_rules(project_dir=tmp_path, capsys=capsys)[0]

    assert findings == test_case.expected_findings


@pytest.mark.parametrize(
    "test_case",
    [
        StructPassthroughTestCase(
            description="declared STRUCT equals the passthrough",
            declared_type='STRUCT("café" INTEGER)',
            expected_exit_code=0,
            expected_codes=(),
        ),
        StructPassthroughTestCase(
            description="declared STRUCT differs from the passthrough",
            declared_type='STRUCT("café" BIGINT)',
            expected_exit_code=1,
            expected_codes=("K002", "SQBRCONTRACT105"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_struct_passthrough_when_proving_types_then_adapter_callback_decides(
    test_case: StructPassthroughTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    write_files(
        project_dir=tmp_path,
        files=struct_passthrough_files(declared_type=test_case.declared_type),
    )

    outcome: tuple[int, tuple[str, ...]] = compile_codes(project_dir=tmp_path, capsys=capsys)

    assert outcome == (test_case.expected_exit_code, test_case.expected_codes)
