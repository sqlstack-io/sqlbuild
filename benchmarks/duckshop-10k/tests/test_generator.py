"""Offline checks of the DuckShop 10K generator: shape, determinism and the recorded checksum."""

from __future__ import annotations

import re
import sys
from functools import cache
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HERE))

from duckshop10k.core import _placement, declaration_uses  # noqa: E402
from duckshop10k.manifest import checksum, checksum_line, read_project, write_project  # noqa: E402
from duckshop10k.project import build  # noqa: E402
from duckshop10k.stats import shape  # noqa: E402


@cache
def full() -> dict[str, str]:
    return build("full")


def models(files: dict[str, str]) -> dict[str, str]:
    return {
        path: text
        for path, text in files.items()
        if path.startswith("models/") and "/_sqlbuild/" not in path and path.endswith(".sql")
    }


def test_given_two_builds_when_fingerprinting_then_checksums_match() -> None:
    assert checksum(build("full")) == checksum(full())


def test_given_full_profile_when_building_then_matches_recorded_checksum() -> None:
    assert checksum_line(full()) == (HERE / "checksum.txt").read_text()


def test_given_full_profile_when_measuring_then_has_about_eleven_thousand_models_in_real_mix() -> (
    None
):
    measured = shape(full())

    assert 10_500 <= measured.models <= 11_000
    assert 55 <= measured.layer_percent("staging") <= 59
    assert 21 <= measured.layer_percent("intermediate") <= 25
    assert 18 <= measured.layer_percent("marts") + measured.layer_percent("snapshots") <= 22


def test_given_full_profile_when_measuring_then_fan_in_and_depth_match_the_profile() -> None:
    measured = shape(full())
    reading = {count: n for count, n in measured.upstream_counts.items() if count > 0}
    total = sum(reading.values())

    assert sum(n for count, n in reading.items() if count <= 4) / total >= 0.95
    assert 10 <= sum(n for count, n in reading.items() if count >= 10) <= 100
    assert 8 <= measured.max_depth <= 14


def test_given_full_profile_when_measuring_then_samples_a_few_hundred_unit_tests() -> None:
    measured = shape(full())

    assert 200 <= measured.unit_tests <= 600
    assert 1 <= measured.scenarios <= 5


@pytest.mark.parametrize(
    ("construct", "minimum_models"),
    [
        (r"\bHASH\(", 1),
        (r"TIMESTAMP_LTZ", 1),
        (r"CURRENT_TIMESTAMP\(\)", 2),
        (r"ASOF JOIN", 10),
        (r"LATERAL FLATTEN", 200),
        (r"\bQUALIFY\b", 100),
        (r"CONVERT_TIMEZONE|@sql\.(local_business_date|local_to_utc|tz_to_utc)\(", 800),
        (r"TRY_PARSE_JSON", 100),
        (r"\bFILTER\(", 10),
        (r"incremental_mode microbatch", 20),
        (r"materialized incremental", 500),
        (r"cursor_start_max_ahead", 100),
        (r"migrate_from", 100),
        (r"materialized snapshot", 5),
        (r"__udf\(", 2),
        (r"__table_fn\(", 1),
        (r"@py\.", 3),
        (r"GENERATOR\(", 1),
    ],
)
def test_given_full_profile_when_scanning_then_uses_each_construct(
    construct: str, minimum_models: int
) -> None:
    pattern = re.compile(construct)

    assert sum(1 for text in models(full()).values() if pattern.search(text)) >= minimum_models


def test_given_full_profile_when_scanning_then_writes_a_few_hundred_python_assets() -> None:
    files = full()
    modules = [path for path in files if path.startswith("python/exports/")]
    assets = sum(
        files[path].count("\n@asset(") + files[path].startswith("@asset(") for path in modules
    )

    assert 350 <= assets <= 500
    assert len(modules) >= 40
    assert all("from python.shared.export_feed import" in files[path] for path in modules)


def test_given_known_divergence_constructs_when_scanning_then_models_are_tagged() -> None:
    for path, text in models(full()).items():
        if re.search(r"\bHASH\(|TIMESTAMP_LTZ|CURRENT_TIMESTAMP\(\)", text.split(");", 1)[1]):
            assert "known_divergence" in text, path


def test_given_generated_files_when_scanning_then_uses_only_synthetic_identifiers() -> None:
    joined = "\n".join(full().values()).lower()

    for word in ("password", "secret", "token", "@gmail", "snowflakecomputing"):
        assert word not in joined


def test_given_declarations_when_placing_then_follow_their_users() -> None:
    assert _placement({"models/staging/a/x", "models/staging/b/y"}, "macros/sql") == (
        "models/staging/_sqlbuild/macros/sql"
    )
    assert _placement({"models/staging/a"}, "constants") == "models/staging/a/_sqlbuild/_constants"
    assert _placement({"models/staging/a", "tests/unit/b"}, "macros/sql") == "macros/sql"
    assert _placement({"models/a", "models/b"}, "enums") == "enums"


def test_given_full_profile_when_placing_then_every_used_declaration_is_written_once() -> None:
    files = full()
    declared = [path for path in files if "/_sqlbuild/" in path or path.startswith("macros/")]
    names = [Path(path).name for path in declared]

    assert len(names) == len(set(names))
    used = set().union(*(declaration_uses(text) for text in models(files).values()))
    assert ("macro", "cdc_latest") in used
    assert any(path.endswith("/cdc_latest.sql") for path in declared)


def test_given_written_project_when_reading_back_then_checksum_is_unchanged(tmp_path: Path) -> None:
    files = build("smoke")
    write_project(files, tmp_path / "project")

    assert checksum(read_project(tmp_path / "project")) == checksum(files)


def test_given_non_empty_output_when_writing_then_refuses(tmp_path: Path) -> None:
    (tmp_path / "existing.txt").write_text("keep me")

    with pytest.raises(SystemExit):
        write_project(build("smoke"), tmp_path)


def test_given_generated_custom_rule_when_running_its_tests_then_they_pass(tmp_path: Path) -> None:
    project = tmp_path / "project"
    write_project(build("smoke"), project)
    sys.path.insert(0, str(project))
    try:
        namespace: dict[str, object] = {}
        exec(
            compile((project / "tests/test_rule_staging_dedupe.py").read_text(), "rule", "exec"),
            namespace,
        )
        tests = [value for name, value in namespace.items() if name.startswith("test_")]
        assert len(tests) == 3
        for test in tests:
            test()  # type: ignore[operator]
    finally:
        sys.path.remove(str(project))
        for module in [name for name in sys.modules if name.startswith("rules")]:
            del sys.modules[module]
