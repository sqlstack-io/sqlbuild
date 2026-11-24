from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import SchemaColumn, SchemaModelEntry, SchemaSeedEntry, SeedCsvSettings
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    ParseSchemaYamlErrorTestCase,
    ParseSchemaYamlTestCase,
    ParseSeedAuditsYamlTestCase,
    ParseSeedCsvSettingsYamlTestCase,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers.helpers import expected_or_actual
from tests.unit.src.sqlbuild.compiler.discovery.helpers import parse_schema_yml


@pytest.mark.parametrize(
    "test_case",
    [
        ParseSchemaYamlTestCase(
            description="parses seed metadata while model metadata is header-owned",
            contents="""
        seeds:
          - name: country_codes
            tags: [reference, daily]
            columns:
              - name: country_code
                type: VARCHAR
                nullable: false
              - name: country_name
                type: VARCHAR
                nullable: true
        """,
            expected_model_names=(),
            expected_seed_names=("country_codes",),
            expected_model_column_names=(),
            expected_seed_column_names=(("country_code", "country_name"),),
            expected_model_audit_names=(),
            expected_column_audit_names=(),
            expected_seed_column_nullables=((False, True),),
            expected_seed_databases=(None,),
            expected_seed_schemas=(None,),
            expected_seed_tags=(("reference", "daily"),),
        ),
        ParseSchemaYamlTestCase(
            description="parses seed target overrides",
            contents="""
        seeds:
          - name: country_codes
            database: "${ENV:SEED_DB}"
            schema: "${coalesce(ENV:SEED_SCHEMA, 'lookups')}"
            columns:
              - name: country_code
                type: VARCHAR
        """,
            expected_model_names=(),
            expected_seed_names=("country_codes",),
            expected_model_column_names=(),
            expected_seed_column_names=(("country_code",),),
            expected_model_audit_names=(),
            expected_column_audit_names=(),
            expected_seed_databases=("${ENV:SEED_DB}",),
            expected_seed_schemas=("${coalesce(ENV:SEED_SCHEMA, 'lookups')}",),
        ),
        ParseSchemaYamlTestCase(
            description="allows empty schema files with no models or seeds",
            contents="{}\n",
            expected_model_names=(),
            expected_seed_names=(),
            expected_model_column_names=(),
            expected_seed_column_names=(),
            expected_model_audit_names=(),
            expected_column_audit_names=(),
            expected_seed_databases=(),
            expected_seed_schemas=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_schema_yaml_variants_when_parsing_then_it_returns_expected_raw_metadata(
    test_case: ParseSchemaYamlTestCase,
) -> None:
    model_entries: tuple[SchemaModelEntry, ...]
    seed_entries: tuple[SchemaSeedEntry, ...]
    model_entries, seed_entries = parse_schema_yml(
        contents=test_case.contents, file_path=Path("sql/seeds/lookups.yml")
    )

    assert tuple(entry.name for entry in model_entries) == test_case.expected_model_names
    assert tuple(entry.name for entry in seed_entries) == test_case.expected_seed_names
    actual_model_column_names: list[tuple[str, ...]] = []
    actual_model_column_nullables_list: list[tuple[bool | None, ...]] = []
    actual_model_audit_names: list[tuple[str, ...]] = []
    actual_column_audit_names: list[tuple[tuple[str, ...], ...]] = []
    for entry in model_entries:
        column_names: list[str] = []
        column_nullables: list[bool | None] = []
        column_audit_names: list[tuple[str, ...]] = []
        for column in entry.columns:
            column_names.append(column.name)
            column_nullables.append(column.nullable)
            column_audit_names.append(tuple(audit.definition_name for audit in column.audits))
        actual_model_column_names.append(tuple(column_names))
        actual_model_column_nullables_list.append(tuple(column_nullables))
        actual_model_audit_names.append(tuple(audit.definition_name for audit in entry.audits))
        actual_column_audit_names.append(tuple(column_audit_names))
    assert tuple(actual_model_column_names) == test_case.expected_model_column_names
    actual_seed_column_names: list[tuple[str, ...]] = []
    actual_seed_column_nullables_list: list[tuple[bool | None, ...]] = []
    for entry in seed_entries:
        column_names = []
        column_nullables = []
        for column in entry.columns:
            column_names.append(column.name)
            column_nullables.append(column.nullable)
        actual_seed_column_names.append(tuple(column_names))
        actual_seed_column_nullables_list.append(tuple(column_nullables))
    assert tuple(actual_seed_column_names) == test_case.expected_seed_column_names
    actual_model_column_nullables: tuple[tuple[bool | None, ...], ...] = tuple(
        actual_model_column_nullables_list
    )
    assert actual_model_column_nullables == expected_or_actual(
        test_case.expected_model_column_nullables, actual_model_column_nullables
    )
    actual_seed_column_nullables: tuple[tuple[bool | None, ...], ...] = tuple(
        actual_seed_column_nullables_list
    )
    assert actual_seed_column_nullables == expected_or_actual(
        test_case.expected_seed_column_nullables, actual_seed_column_nullables
    )
    assert tuple(entry.database for entry in seed_entries) == test_case.expected_seed_databases
    assert tuple(entry.schema for entry in seed_entries) == test_case.expected_seed_schemas
    actual_seed_tags: tuple[tuple[str, ...], ...] = tuple(entry.tags for entry in seed_entries)
    assert actual_seed_tags == expected_or_actual(test_case.expected_seed_tags, actual_seed_tags)
    assert tuple(actual_model_audit_names) == test_case.expected_model_audit_names
    assert tuple(actual_column_audit_names) == test_case.expected_column_audit_names


@pytest.mark.parametrize(
    "test_case",
    [
        ParseSeedCsvSettingsYamlTestCase(
            description="parses full seed csv settings",
            contents="""
        seeds:
          - name: country_codes
            csv_settings:
              delimiter: "|"
              quotechar: '"'
              doublequote: false
              escapechar: "\\\\"
              skipinitialspace: true
              lineterminator: LF
              encoding: utf-8
              na_values:
                country_name: ["", NULL, 0, true]
              keep_default_na: false
            columns:
              - name: country_code
                type: VARCHAR
              - name: country_name
                type: VARCHAR
        """,
            expected_delimiter="|",
            expected_quotechar='"',
            expected_doublequote=False,
            expected_escapechar="\\",
            expected_skipinitialspace=True,
            expected_lineterminator="LF",
            expected_encoding="utf-8",
            expected_na_values={"country_name": ("", None, 0, True)},
            expected_keep_default_na=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_seed_csv_settings_when_parsing_then_it_returns_normalized_settings(
    test_case: ParseSeedCsvSettingsYamlTestCase,
) -> None:
    _, seed_entries = parse_schema_yml(
        contents=test_case.contents, file_path=Path("sql/seeds/lookups.yml")
    )

    assert seed_entries[0].csv_settings == SeedCsvSettings(
        delimiter=test_case.expected_delimiter,
        quotechar=test_case.expected_quotechar,
        doublequote=test_case.expected_doublequote,
        escapechar=test_case.expected_escapechar,
        skipinitialspace=test_case.expected_skipinitialspace,
        lineterminator=test_case.expected_lineterminator,
        encoding=test_case.expected_encoding,
        na_values=test_case.expected_na_values,
        keep_default_na=test_case.expected_keep_default_na,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ParseSchemaYamlErrorTestCase(
            description="raises when the file does not contain a top-level mapping",
            contents="- name: stg_orders\n",
            expected_error_fragment="must contain a top-level mapping",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when schema yml declares model metadata",
            contents="""
        models:
          - name: stg_orders
        """,
            expected_error_fragment="model metadata must live in the model file MODEL",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when seeds is not a list",
            contents="seeds: {}\n",
            expected_error_fragment="seeds must be a list",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when a seed entry is not a mapping",
            contents="""
        seeds:
          - country_codes
        """,
            expected_error_fragment="seeds must contain only mappings",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when a seed omits columns",
            contents="""
        seeds:
          - name: country_codes
        """,
            expected_error_fragment="seed must declare at least one column",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when seed meta is not a mapping",
            contents="""
        seeds:
          - name: country_codes
            meta: nope
            columns:
              - name: country_code
                type: VARCHAR
        """,
            expected_error_fragment="seed 'meta' must be a mapping",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when seed columns is not a list",
            contents="""
        seeds:
          - name: country_codes
            columns: {}
        """,
            expected_error_fragment="seed columns must be a list",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when seed column entry is not a mapping",
            contents="""
        seeds:
          - name: country_codes
            columns:
              - country_code
        """,
            expected_error_fragment="seed columns must contain only mappings",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when a seed column omits type",
            contents="""
        seeds:
          - name: country_codes
            columns:
              - name: country_code
        """,
            expected_error_fragment="seed column 'country_code' must define non-empty string 'type'",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when seed column name is blank",
            contents="""
        seeds:
          - name: country_codes
            columns:
              - name: ""
                type: VARCHAR
        """,
            expected_error_fragment="seed column must define non-empty string 'name'",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when seed column nullable is not a boolean",
            contents="""
        seeds:
          - name: country_codes
            columns:
              - name: country_code
                type: VARCHAR
                nullable: 123
        """,
            expected_error_fragment="seed column 'nullable' must be a boolean",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when seed column allows nulls and uses not null audit",
            contents="""
        seeds:
          - name: country_codes
            columns:
              - name: country_code
                type: VARCHAR
                nullable: true
                audits:
                  - not_null
        """,
            expected_error_fragment=(
                "column 'country_code' cannot set nullable = true and audit not_null"
            ),
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when seed csv settings is not a mapping",
            contents="""
        seeds:
          - name: country_codes
            csv_settings: nope
            columns:
              - name: country_code
                type: VARCHAR
        """,
            expected_error_fragment="seed 'csv_settings' must be a mapping",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when seed csv settings has unknown keys",
            contents="""
        seeds:
          - name: country_codes
            csv_settings:
              unknown: true
            columns:
              - name: country_code
                type: VARCHAR
        """,
            expected_error_fragment="unknown keys: unknown",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when seed csv string setting is not a string",
            contents="""
        seeds:
          - name: country_codes
            csv_settings:
              delimiter: false
            columns:
              - name: country_code
                type: VARCHAR
        """,
            expected_error_fragment="csv_settings 'delimiter' must be a string",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when seed csv boolean setting is not a boolean",
            contents="""
        seeds:
          - name: country_codes
            csv_settings:
              keep_default_na: nope
            columns:
              - name: country_code
                type: VARCHAR
        """,
            expected_error_fragment="csv_settings 'keep_default_na' must be a boolean",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when seed csv na values type is invalid",
            contents="""
        seeds:
          - name: country_codes
            csv_settings:
              na_values: nope
            columns:
              - name: country_code
                type: VARCHAR
        """,
            expected_error_fragment="csv_settings 'na_values' must be a list or mapping",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when a seed entry has an unknown key",
            contents="""
        seeds:
          - name: country_codes
            tests: [not_null]
            columns:
              - name: country_code
                type: VARCHAR
        """,
            expected_error_fragment="seed has unknown keys: tests",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when a seed column has an unknown key",
            contents="""
        seeds:
          - name: country_codes
            columns:
              - name: country_code
                type: VARCHAR
                checks: [not_null]
        """,
            expected_error_fragment="seed column has unknown keys: checks",
        ),
        ParseSchemaYamlErrorTestCase(
            description="raises when a seed file has an unknown top-level key",
            contents="""
        models: []
        extras: true
        seeds:
          - name: country_codes
            columns:
              - name: country_code
                type: VARCHAR
        """,
            expected_error_fragment="seed file has unknown keys: extras, models",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_schema_yaml_when_parsing_then_it_raises_clear_errors(
    test_case: ParseSchemaYamlErrorTestCase,
) -> None:
    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        parse_schema_yml(contents=test_case.contents, file_path=Path("sql/seeds/lookups.yml"))


@pytest.mark.parametrize(
    "test_case",
    [
        ParseSeedAuditsYamlTestCase(
            description="parses seed table and column audits",
            contents="""
        seeds:
          - name: country_codes
            audits:
              - expression_is_true:
                  expression: "country_code <> ''"
                  severity: error
            columns:
              - name: country_code
                type: VARCHAR
                audits: [not_null, unique]
        """,
            expected_table_audits=("expression_is_true",),
            expected_column_audits=(("not_null", "unique"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_seed_audits_when_parsing_then_table_and_column_audits_are_kept(
    test_case: ParseSeedAuditsYamlTestCase,
) -> None:
    seed_entries: tuple[SchemaSeedEntry, ...]
    _models, seed_entries = parse_schema_yml(
        contents=test_case.contents, file_path=Path("sql/seeds/lookups.yml")
    )

    table_audits: tuple[str, ...] = tuple(audit.definition_name for audit in seed_entries[0].audits)
    column_audits: list[tuple[str, ...]] = []
    column: SchemaColumn
    for column in seed_entries[0].columns:
        column_audits.append(tuple(audit.definition_name for audit in column.audits))

    assert table_audits == test_case.expected_table_audits
    assert tuple(column_audits) == test_case.expected_column_audits
