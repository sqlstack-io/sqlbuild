from collections.abc import Callable
from pathlib import Path
from types import MappingProxyType
from typing import cast

from sqlbuild._native import (
    CompileDirectLogicSqlTestInputPayload,
    CompileModelSqlTestInputPayload,
    CompileSqlTestInput,
    build_compile_reference_resolver,
)
from sqlbuild.compiler.compile.types import ExternalSqlReferenceResolver


def base_repo_files() -> dict[str, str]:
    return {
        "sqlbuild_project.toml": (
            'name = "demo"\nadapter = "duckdb"\nextra_paths = ["dbt"]\n\n'
            '[settings]\ndefault_audit_severity = "warn"\n'
        ),
    }


def build_external_sql_reference_resolver(
    *, project_dir: Path
) -> ExternalSqlReferenceResolver | None:
    manifest_path: Path = project_dir / "dbt" / "target" / "manifest.json"
    return _EXTERNAL_RESOLVER_BUILDERS[manifest_path.is_file()](manifest_path)


def _build_external_resolver(manifest_path: Path) -> ExternalSqlReferenceResolver | None:
    return build_compile_reference_resolver(
        manifest_contents=manifest_path.read_text(encoding="utf-8")
    )


def _build_no_external_resolver(manifest_path: Path) -> ExternalSqlReferenceResolver | None:
    del manifest_path
    return None


_EXTERNAL_RESOLVER_BUILDERS: MappingProxyType[
    bool, Callable[[Path], ExternalSqlReferenceResolver | None]
] = MappingProxyType({False: _build_no_external_resolver, True: _build_external_resolver})


def expected_or_actual[T](expected: T | None, actual: T) -> T:
    return (actual, cast(T, expected))[expected is not None]


def compile_sql_test_authored_cte_names(test_input: CompileSqlTestInput) -> tuple[str, ...]:
    return _MODEL_PAYLOAD_GETTERS[type(test_input.payload)][0](test_input.payload)


def compile_sql_test_mock_model_names(test_input: CompileSqlTestInput) -> tuple[str, ...]:
    return _MODEL_PAYLOAD_GETTERS[type(test_input.payload)][1](test_input.payload)


def compile_sql_test_mock_source_names(test_input: CompileSqlTestInput) -> tuple[str, ...]:
    return _MODEL_PAYLOAD_GETTERS[type(test_input.payload)][2](test_input.payload)


def compile_sql_test_mock_seed_names(test_input: CompileSqlTestInput) -> tuple[str, ...]:
    return _MODEL_PAYLOAD_GETTERS[type(test_input.payload)][3](test_input.payload)


def compile_sql_test_expected_model_names(test_input: CompileSqlTestInput) -> tuple[str, ...]:
    return _MODEL_PAYLOAD_GETTERS[type(test_input.payload)][4](test_input.payload)


def _authored_cte_names(payload: object) -> tuple[str, ...]:
    model_payload: CompileModelSqlTestInputPayload = cast(CompileModelSqlTestInputPayload, payload)
    return tuple(cte.name for cte in model_payload.authored_ctes)


def _mock_model_names(payload: object) -> tuple[str, ...]:
    return cast(CompileModelSqlTestInputPayload, payload).mock_model_names


def _mock_source_names(payload: object) -> tuple[str, ...]:
    return cast(CompileModelSqlTestInputPayload, payload).mock_source_names


def _mock_seed_names(payload: object) -> tuple[str, ...]:
    return cast(CompileModelSqlTestInputPayload, payload).mock_seed_names


def _expected_model_names(payload: object) -> tuple[str, ...]:
    return cast(CompileModelSqlTestInputPayload, payload).expected_model_names


def _empty_names(payload: object) -> tuple[str, ...]:
    del payload
    return ()


_MODEL_PAYLOAD_GETTERS: MappingProxyType[
    type[object], tuple[Callable[[object], tuple[str, ...]], ...]
] = MappingProxyType(
    {
        CompileModelSqlTestInputPayload: (
            _authored_cte_names,
            _mock_model_names,
            _mock_source_names,
            _mock_seed_names,
            _expected_model_names,
        ),
        CompileDirectLogicSqlTestInputPayload: (_empty_names,) * 5,
    }
)
