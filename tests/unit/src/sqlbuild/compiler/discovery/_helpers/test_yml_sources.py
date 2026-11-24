from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import SourceEntry, SourceWriteStrategy
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    ParseSourcesYamlErrorTestCase,
    ParseSourcesYamlTestCase,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers.helpers import expected_or_actual
from tests.unit.src.sqlbuild.compiler.discovery.helpers import parse_sources_yml


@pytest.mark.parametrize(
    "test_case",
    [
        ParseSourcesYamlTestCase(
            description="parses sources with quality metadata and columns",
            contents="""
        sources:
          - name: raw_orders
            database: raw
            schema: public
            table: orders
            description: Raw orders from ingestion.
            type_enforcement: true
            contract: enforced
            meta:
              owner: finance
            audits:
              - source_freshness:
                  threshold_hours: 2
            columns:
              - name: order_id
                type: VARCHAR
                nullable: false
                description: Stable order identifier.
                audits:
                  - not_null
              - name: created_at
                type: TIMESTAMP_NTZ
                nullable: true
                audits:
                  - recency:
                      max_age_hours: 24
          - name: raw_customers
            schema: public
            table: customers
        """,
            expected_source_names=("raw_orders", "raw_customers"),
            expected_column_names=(("order_id", "created_at"), ()),
            expected_type_enforcement_values=(True, None),
            expected_contract_values=("enforced", None),
            expected_expressions=(None, None),
            expected_source_audit_names=(("source_freshness",), ()),
            expected_column_audit_names=((("not_null",), ("recency",)), ()),
            expected_column_nullables=((False, True), ()),
        ),
        ParseSourcesYamlTestCase(
            description="defaults expression source type enforcement from typed columns",
            contents="""
        sources:
          - name: raw_orders
            expression: |
              SELECT 1 AS order_id, 'placed' AS status
            columns:
              - name: order_id
                type: INTEGER
              - name: status
                type: VARCHAR
        """,
            expected_source_names=("raw_orders",),
            expected_column_names=(("order_id", "status"),),
            expected_type_enforcement_values=(True,),
            expected_contract_values=(None,),
            expected_expressions=("SELECT 1 AS order_id, 'placed' AS status\n",),
            expected_source_audit_names=((),),
            expected_column_audit_names=(((), ()),),
        ),
        ParseSourcesYamlTestCase(
            description="allows expression source type enforcement to opt in explicitly",
            contents="""
        sources:
          - name: raw_orders
            expression: |
              SELECT 1 AS order_id, 'placed' AS status
            type_enforcement: true
            columns:
              - name: order_id
                type: INTEGER
        """,
            expected_source_names=("raw_orders",),
            expected_column_names=(("order_id",),),
            expected_type_enforcement_values=(True,),
            expected_contract_values=(None,),
            expected_expressions=("SELECT 1 AS order_id, 'placed' AS status\n",),
            expected_source_audit_names=((),),
            expected_column_audit_names=(((),),),
        ),
        ParseSourcesYamlTestCase(
            description="does not enforce source types for untyped column metadata",
            contents="""
        sources:
          - name: raw_orders
            schema: public
            table: orders
            columns:
              - name: order_id
                description: Stable order identifier.
        """,
            expected_source_names=("raw_orders",),
            expected_column_names=(("order_id",),),
            expected_type_enforcement_values=(None,),
            expected_contract_values=(None,),
            expected_expressions=(None,),
            expected_source_audit_names=((),),
            expected_column_audit_names=(((),),),
        ),
        ParseSourcesYamlTestCase(
            description="allows source columns to opt out of default type enforcement",
            contents="""
        sources:
          - name: raw_orders
            schema: public
            table: orders
            type_enforcement: false
            columns:
              - name: order_id
                type: INTEGER
        """,
            expected_source_names=("raw_orders",),
            expected_column_names=(("order_id",),),
            expected_type_enforcement_values=(False,),
            expected_contract_values=(None,),
            expected_expressions=(None,),
            expected_source_audit_names=((),),
            expected_column_audit_names=(((),),),
        ),
        ParseSourcesYamlTestCase(
            description="parses source loader metadata and write strategies",
            contents="""
        sources:
          - name: raw_events
            managed: true
            write_strategy: delete_insert
            cursor_column: event_at
          - name: raw_customers
            managed: true
            write_strategy: merge
            unique_key: [customer_id, updated_at]
            cursor_column: updated_at
          - name: raw_prices
            managed: true
            write_strategy: table
            load_batch_size: 500
          - name: raw_webhooks
            managed: true
            write_strategy: append
            cursor_column: received_at
        """,
            expected_source_names=("raw_events", "raw_customers", "raw_prices", "raw_webhooks"),
            expected_column_names=((), (), (), ()),
            expected_type_enforcement_values=(None, None, None, None),
            expected_contract_values=(None, None, None, None),
            expected_expressions=(None, None, None, None),
            expected_loaders=("raw_events", "raw_customers", "raw_prices", "raw_webhooks"),
            expected_write_strategies=("delete_insert", "merge", "table", "append"),
            expected_load_batch_sizes=(None, None, 500, None),
            expected_cursor_columns=("event_at", "updated_at", None, "received_at"),
            expected_unique_keys=((), ("customer_id", "updated_at"), (), ()),
            expected_source_audit_names=((), (), (), ()),
            expected_column_audit_names=((), (), (), ()),
        ),
        ParseSourcesYamlTestCase(
            description="parses source freshness strategies",
            contents="""
        sources:
          - name: raw_orders
            schema: public
            table: orders
            freshness:
              strategy: adapter
          - name: raw_events
            schema: public
            table: events
            freshness:
              strategy: column
              column: updated_at
              type: timestamp
              filter: "updated_at >= current_date - interval '2 days'"
              lag_tolerance: 15m
              age_policy:
                warn_after: 12h
                error_after: 24h
          - name: raw_customers
            schema: public
            table: customers
            freshness:
              strategy: sql
              query: SELECT MAX(version) FROM raw.public.customers
              type: integer
          - name: raw_clicks
            schema: public
            table: clicks
            freshness:
              strategy: column
              column: loaded_at
              type: timestamp
              age_policy:
                warn_after: 6h
          - name: raw_shipments
            schema: public
            table: shipments
            freshness:
              strategy: column
              column: loaded_at
              type: timestamp
              age_policy:
                error_after: 1d
        """,
            expected_source_names=(
                "raw_orders",
                "raw_events",
                "raw_customers",
                "raw_clicks",
                "raw_shipments",
            ),
            expected_column_names=((), (), (), (), ()),
            expected_type_enforcement_values=(None, None, None, None, None),
            expected_contract_values=(None, None, None, None, None),
            expected_expressions=(None, None, None, None, None),
            expected_source_audit_names=((), (), (), (), ()),
            expected_column_audit_names=((), (), (), (), ()),
            expected_freshness_strategies=("adapter", "column", "sql", "column", "column"),
            expected_freshness_value_kinds=(None, "timestamp", "integer", "timestamp", "timestamp"),
            expected_freshness_columns=(None, "updated_at", None, "loaded_at", "loaded_at"),
            expected_freshness_queries=(
                None,
                None,
                "SELECT MAX(version) FROM raw.public.customers",
                None,
                None,
            ),
            expected_freshness_filters=(
                None,
                "updated_at >= current_date - interval '2 days'",
                None,
                None,
                None,
            ),
            expected_freshness_lag_tolerances=(None, "15m", None, None, None),
            expected_freshness_age_policy_warn_afters=(None, "12h", None, "6h", None),
            expected_freshness_age_policy_error_afters=(None, "24h", None, None, "1d"),
        ),
        ParseSourcesYamlTestCase(
            description="infers source freshness strategy and type from keys and columns",
            contents="""
        sources:
          - name: raw_orders
            schema: public
            table: orders
            freshness: {}
          - name: raw_events
            schema: public
            table: events
            columns:
              - name: Updated_At
                type: timestamp_ntz(9)
            freshness:
              column: updated_at
              lag_tolerance: 15m
          - name: raw_customers
            schema: public
            table: customers
            freshness:
              query: SELECT MAX(version) FROM raw.public.customers
              type: integer
          - name: raw_clicks
            schema: public
            table: clicks
            columns:
              - name: batch_id
                type: BIGINT
            freshness:
              strategy: column
              column: batch_id
          - name: raw_tags
            schema: public
            table: tags
            columns:
              - name: tag_version
                type: character varying(32)
            freshness:
              column: tag_version
          - name: raw_shipments
            schema: public
            table: shipments
            columns:
              - name: shipped_on
                type: date
            freshness:
              column: loaded_at
              type: timestamp
          - name: raw_returns
            schema: public
            table: returns
            columns:
              - name: returned_at
                type: timestamp with time zone
            freshness:
              strategy: column
              column: returned_at
              type: timestamp
        """,
            expected_source_names=(
                "raw_orders",
                "raw_events",
                "raw_customers",
                "raw_clicks",
                "raw_tags",
                "raw_shipments",
                "raw_returns",
            ),
            expected_column_names=(
                (),
                ("Updated_At",),
                (),
                ("batch_id",),
                ("tag_version",),
                ("shipped_on",),
                ("returned_at",),
            ),
            expected_type_enforcement_values=(None, True, None, True, True, True, True),
            expected_contract_values=(None,) * 7,
            expected_expressions=(None,) * 7,
            expected_source_audit_names=((),) * 7,
            expected_column_audit_names=((), ((),), (), ((),), ((),), ((),), ((),)),
            expected_freshness_strategies=(
                "adapter",
                "column",
                "sql",
                "column",
                "column",
                "column",
                "column",
            ),
            expected_freshness_value_kinds=(
                None,
                "timestamp",
                "integer",
                "integer",
                "string",
                "timestamp",
                "timestamp",
            ),
            expected_freshness_columns=(
                None,
                "updated_at",
                None,
                "batch_id",
                "tag_version",
                "loaded_at",
                "returned_at",
            ),
        ),
        ParseSourcesYamlTestCase(
            description="allows empty sources files with no declarations",
            contents="{}\n",
            expected_source_names=(),
            expected_column_names=(),
            expected_type_enforcement_values=(),
            expected_contract_values=(),
            expected_expressions=(),
            expected_source_audit_names=(),
            expected_column_audit_names=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sources_yaml_variants_when_parsing_then_it_returns_expected_raw_metadata(
    test_case: ParseSourcesYamlTestCase,
) -> None:
    source_entries: tuple[SourceEntry, ...] = parse_sources_yml(
        contents=test_case.contents, file_path=Path("sql/sources/raw.yml")
    )

    assert tuple(entry.name for entry in source_entries) == test_case.expected_source_names
    actual_column_names: list[tuple[str, ...]] = []
    actual_column_nullables_list: list[tuple[bool | None, ...]] = []
    actual_source_audit_names: list[tuple[str, ...]] = []
    actual_column_audit_names: list[tuple[tuple[str, ...], ...]] = []
    for entry in source_entries:
        column_names: list[str] = []
        column_nullables: list[bool | None] = []
        column_audit_names: list[tuple[str, ...]] = []
        for column in entry.columns:
            column_names.append(column.name)
            column_nullables.append(column.nullable)
            column_audit_names.append(tuple(audit.definition_name for audit in column.audits))
        actual_column_names.append(tuple(column_names))
        actual_column_nullables_list.append(tuple(column_nullables))
        actual_source_audit_names.append(tuple(audit.definition_name for audit in entry.audits))
        actual_column_audit_names.append(tuple(column_audit_names))
    assert tuple(actual_column_names) == test_case.expected_column_names
    actual_column_nullables: tuple[tuple[bool | None, ...], ...] = tuple(
        actual_column_nullables_list
    )
    assert actual_column_nullables == expected_or_actual(
        test_case.expected_column_nullables, actual_column_nullables
    )
    assert (
        tuple(entry.type_enforcement for entry in source_entries)
        == test_case.expected_type_enforcement_values
    )
    assert tuple(entry.contract for entry in source_entries) == test_case.expected_contract_values
    assert tuple(entry.expression for entry in source_entries) == test_case.expected_expressions
    actual_loaders: tuple[str | None, ...] = tuple(entry.loader for entry in source_entries)
    assert actual_loaders == expected_or_actual(test_case.expected_loaders, actual_loaders)
    actual_write_strategies: tuple[str | None, ...] = tuple(
        entry.write_strategy for entry in source_entries
    )
    assert actual_write_strategies == expected_or_actual(
        test_case.expected_write_strategies, actual_write_strategies
    )
    assert all(
        entry.write_strategy is None or isinstance(entry.write_strategy, SourceWriteStrategy)
        for entry in source_entries
    )
    actual_load_batch_sizes: tuple[int | None, ...] = tuple(
        entry.load_batch_size for entry in source_entries
    )
    assert actual_load_batch_sizes == expected_or_actual(
        test_case.expected_load_batch_sizes, actual_load_batch_sizes
    )
    actual_cursor_columns: tuple[str | None, ...] = tuple(
        entry.cursor_column for entry in source_entries
    )
    assert actual_cursor_columns == expected_or_actual(
        test_case.expected_cursor_columns, actual_cursor_columns
    )
    actual_unique_keys: tuple[tuple[str, ...], ...] = tuple(
        entry.unique_key for entry in source_entries
    )
    assert actual_unique_keys == expected_or_actual(
        test_case.expected_unique_keys, actual_unique_keys
    )
    actual_freshness_strategies: tuple[str | None, ...] = tuple(
        getattr(entry.freshness, "strategy", None) for entry in source_entries
    )
    assert actual_freshness_strategies == expected_or_actual(
        test_case.expected_freshness_strategies, actual_freshness_strategies
    )
    actual_freshness_value_kinds: tuple[str | None, ...] = tuple(
        getattr(entry.freshness, "value_kind", None) for entry in source_entries
    )
    assert actual_freshness_value_kinds == expected_or_actual(
        test_case.expected_freshness_value_kinds, actual_freshness_value_kinds
    )
    actual_freshness_columns: tuple[str | None, ...] = tuple(
        getattr(entry.freshness, "column", None) for entry in source_entries
    )
    assert actual_freshness_columns == expected_or_actual(
        test_case.expected_freshness_columns, actual_freshness_columns
    )
    actual_freshness_queries: tuple[str | None, ...] = tuple(
        getattr(entry.freshness, "query", None) for entry in source_entries
    )
    assert actual_freshness_queries == expected_or_actual(
        test_case.expected_freshness_queries, actual_freshness_queries
    )
    actual_freshness_filters: tuple[str | None, ...] = tuple(
        getattr(entry.freshness, "filter", None) for entry in source_entries
    )
    assert actual_freshness_filters == expected_or_actual(
        test_case.expected_freshness_filters, actual_freshness_filters
    )
    actual_freshness_lag_tolerances: tuple[str | None, ...] = tuple(
        getattr(entry.freshness, "lag_tolerance", None) for entry in source_entries
    )
    assert actual_freshness_lag_tolerances == expected_or_actual(
        test_case.expected_freshness_lag_tolerances, actual_freshness_lag_tolerances
    )
    actual_freshness_age_policy_warn_afters: tuple[str | None, ...] = tuple(
        getattr(getattr(entry.freshness, "age_policy", None), "warn_after", None)
        for entry in source_entries
    )
    assert actual_freshness_age_policy_warn_afters == expected_or_actual(
        test_case.expected_freshness_age_policy_warn_afters,
        actual_freshness_age_policy_warn_afters,
    )
    actual_freshness_age_policy_error_afters: tuple[str | None, ...] = tuple(
        getattr(getattr(entry.freshness, "age_policy", None), "error_after", None)
        for entry in source_entries
    )
    assert actual_freshness_age_policy_error_afters == expected_or_actual(
        test_case.expected_freshness_age_policy_error_afters,
        actual_freshness_age_policy_error_afters,
    )
    assert tuple(actual_source_audit_names) == test_case.expected_source_audit_names
    assert tuple(actual_column_audit_names) == test_case.expected_column_audit_names


@pytest.mark.parametrize(
    "test_case",
    [
        ParseSourcesYamlErrorTestCase(
            description="raises when the file does not contain a top-level mapping",
            contents="- name: raw_orders\n",
            expected_error_fragment="must contain a top-level mapping",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when the file declares an unknown top-level key",
            contents="""
        version: 2
        sources:
          - name: raw_orders
        """,
            expected_error_fragment="sources file has unknown keys: version",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when a source declares a misspelled key",
            contents="""
        sources:
          - name: raw_orders
            descripton: Raw orders
        """,
            expected_error_fragment="source has unknown keys: descripton",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when a source column declares a misspelled key",
            contents="""
        sources:
          - name: raw_orders
            columns:
              - name: order_id
                nulable: false
        """,
            expected_error_fragment="source column has unknown keys: nulable",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness declares an unknown key",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: column
              column: loaded_at
              warn_after: 1d
        """,
            expected_error_fragment="source freshness has unknown keys: warn_after",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness age policy declares an unknown key",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: column
              column: loaded_at
              age_policy:
                warn_afer: 1d
        """,
            expected_error_fragment="source freshness age_policy has unknown keys: warn_afer",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when a source declares the removed ingestr integration",
            contents="""
        sources:
          - name: raw_orders
            ingestr:
              source_uri: postgresql://localhost/shop
              source_table: public.orders
        """,
            expected_error_fragment=(
                "source 'raw_orders' declares an ingestr integration, which SQLBuild no longer "
                "supports"
            ),
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when a sources file declares the removed dlt integration",
            contents="""
        dlt_sources:
          - type: sql_database
            config:
              credentials: duckdb:///source.duckdb
            resources:
              - name: raw_orders
                table: orders
        """,
            expected_error_fragment=(
                "declares dlt_sources, a dlt integration, which SQLBuild no longer supports"
            ),
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source meta is not a mapping",
            contents="""
        sources:
          - name: raw_orders
            meta: []
        """,
            expected_error_fragment="source 'meta' must be a mapping",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source contract is unknown",
            contents="""
        sources:
          - name: raw_orders
            contract: strict
        """,
            expected_error_fragment="source 'contract' must be one of",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when sources is not a list",
            contents="sources: {}\n",
            expected_error_fragment="sources must be a list",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when one source entry is not a mapping",
            contents="""
        sources:
          - raw_orders
        """,
            expected_error_fragment="sources must contain only mappings",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when a source omits name",
            contents="""
        sources:
          - schema: public
        """,
            expected_error_fragment="source must define non-empty string 'name'",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source database is blank",
            contents="""
        sources:
          - name: raw_orders
            database: ""
        """,
            expected_error_fragment="source 'database' must be a non-empty string",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source schema is blank",
            contents="""
        sources:
          - name: raw_orders
            schema: ""
        """,
            expected_error_fragment="source 'schema' must be a non-empty string",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source table is blank",
            contents="""
        sources:
          - name: raw_orders
            table: ""
        """,
            expected_error_fragment="source 'table' must be a non-empty string",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source expression is blank",
            contents="""
        sources:
          - name: raw_orders
            expression: ""
        """,
            expected_error_fragment="source 'expression' must be a non-empty string",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source expression is mixed with relation fields",
            contents="""
        sources:
          - name: raw_orders
            schema: main
            expression: SELECT 1 AS order_id
        """,
            expected_error_fragment="source 'raw_orders' cannot define expression with schema",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when expression source enforces types without typed columns",
            contents="""
        sources:
          - name: raw_orders
            expression: SELECT 1 AS order_id
            type_enforcement: true
        """,
            expected_error_fragment=(
                "source 'raw_orders' uses expression with type_enforcement but has no typed columns"
            ),
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source type enforcement is not a boolean",
            contents="""
        sources:
          - name: raw_orders
            type_enforcement: 123
        """,
            expected_error_fragment="source 'type_enforcement' must be a boolean",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source loader is blank",
            contents="""
        sources:
          - name: raw_orders
            loader: ""
        """,
            expected_error_fragment="source 'loader' is not supported",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when write strategy is unknown",
            contents="""
        sources:
          - name: raw_orders
            managed: true
            write_strategy: replace
        """,
            expected_error_fragment="write_strategy must be one of",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when write strategy has no loader",
            contents="""
        sources:
          - name: raw_orders
            write_strategy: table
        """,
            expected_error_fragment="defines write_strategy but is not managed",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when load batch size is not positive",
            contents="""
        sources:
          - name: raw_orders
            managed: true
            write_strategy: table
            load_batch_size: 0
        """,
            expected_error_fragment="source 'load_batch_size' must be a positive integer",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when load batch size is boolean",
            contents="""
        sources:
          - name: raw_orders
            managed: true
            write_strategy: table
            load_batch_size: true
        """,
            expected_error_fragment="source 'load_batch_size' must be a positive integer",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when cursor column is used with table strategy",
            contents="""
        sources:
          - name: raw_orders
            managed: true
            write_strategy: table
            cursor_column: event_at
        """,
            expected_error_fragment="cursor_column is not supported with write_strategy table",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when unique key is used with append strategy",
            contents="""
        sources:
          - name: raw_orders
            managed: true
            write_strategy: append
            unique_key: order_id
        """,
            expected_error_fragment="unique_key is not supported with write_strategy append",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when merge has no unique key",
            contents="""
        sources:
          - name: raw_orders
            managed: true
            write_strategy: merge
        """,
            expected_error_fragment="write_strategy merge requires unique_key",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when unique key is used with table strategy",
            contents="""
        sources:
          - name: raw_orders
            managed: true
            write_strategy: table
            unique_key: order_id
        """,
            expected_error_fragment="unique_key is not supported with write_strategy table",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when delete insert has no cursor column",
            contents="""
        sources:
          - name: raw_orders
            managed: true
            write_strategy: delete_insert
        """,
            expected_error_fragment="write_strategy delete_insert requires cursor_column",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when unique key is used with delete insert strategy",
            contents="""
        sources:
          - name: raw_orders
            managed: true
            write_strategy: delete_insert
            cursor_column: event_at
            unique_key: order_id
        """,
            expected_error_fragment="unique_key is not supported with write_strategy delete_insert",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when unique key is empty list",
            contents="""
        sources:
          - name: raw_orders
            managed: true
            write_strategy: merge
            unique_key: []
        """,
            expected_error_fragment="source 'unique_key' must be non-empty",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when unique key list contains non string",
            contents="""
        sources:
          - name: raw_orders
            managed: true
            write_strategy: merge
            unique_key: [order_id, 123]
        """,
            expected_error_fragment="source 'unique_key' must contain only non-empty strings",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when unique key is wrong type",
            contents="""
        sources:
          - name: raw_orders
            managed: true
            write_strategy: merge
            unique_key: {}
        """,
            expected_error_fragment="source 'unique_key' must be a string or list",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness is not a mapping",
            contents="""
        sources:
          - name: raw_orders
            freshness: []
        """,
            expected_error_fragment="source 'freshness' must be a mapping",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness strategy is unknown",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: latest
        """,
            expected_error_fragment="source freshness 'strategy' must be one of",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness type is unknown",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: column
              column: updated_at
              type: boolean
        """,
            expected_error_fragment="source freshness 'type' must be one of",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when adapter freshness has column",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: adapter
              column: updated_at
        """,
            expected_error_fragment=(
                "source freshness strategy adapter does not support type, column, query, or filter"
            ),
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when column freshness has no column",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: column
              type: timestamp
        """,
            expected_error_fragment="source freshness strategy column requires column",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when column freshness has expression column",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: column
              column: MAX(updated_at)
              type: timestamp
        """,
            expected_error_fragment="source freshness column must be a plain column name",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when column freshness has no type",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: column
              column: updated_at
        """,
            expected_error_fragment="source freshness strategy column requires type",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when implied column freshness has an untyped column",
            contents="""
        sources:
          - name: raw_orders
            columns:
              - name: updated_at
            freshness:
              column: updated_at
        """,
            expected_error_fragment="source freshness strategy column requires type",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when freshness column type cannot be inferred",
            contents="""
        sources:
          - name: raw_orders
            columns:
              - name: ordered_on
                type: date
            freshness:
              column: ordered_on
        """,
            expected_error_fragment=(
                "source freshness cannot infer type from column 'ordered_on' declared as date"
            ),
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when explicit freshness type contradicts the declared column",
            contents="""
        sources:
          - name: raw_orders
            columns:
              - name: updated_at
                type: timestamp
            freshness:
              column: updated_at
              type: integer
        """,
            expected_error_fragment=(
                "source freshness type integer contradicts column 'updated_at' declared as "
                "timestamp; remove type or set it to timestamp"
            ),
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when freshness sets column and query without a strategy",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              column: updated_at
              query: SELECT MAX(updated_at) FROM raw.orders
              type: timestamp
        """,
            expected_error_fragment="source freshness cannot set both column and query",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when freshness sets type without column or query",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              type: timestamp
        """,
            expected_error_fragment="source freshness type and filter require column or query",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when explicit freshness strategy contradicts implied keys",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: column
              query: SELECT MAX(updated_at) FROM raw.orders
              type: timestamp
        """,
            expected_error_fragment="source freshness strategy column requires column",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when sql freshness has no query",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: sql
              type: string
        """,
            expected_error_fragment="source freshness strategy sql requires query",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when sql freshness has no type",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: sql
              query: SELECT MAX(version) FROM raw.orders
        """,
            expected_error_fragment="source freshness strategy sql requires type",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when sql freshness has column",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: sql
              query: SELECT MAX(version) FROM raw.orders
              column: version
              type: integer
        """,
            expected_error_fragment="source freshness strategy sql does not support column",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when sql freshness has filter",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: sql
              query: SELECT MAX(version) FROM raw.orders
              type: integer
              filter: version > 10
        """,
            expected_error_fragment="source freshness strategy sql does not support filter",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when adapter freshness has filter",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: adapter
              filter: updated_at > current_date
        """,
            expected_error_fragment=(
                "source freshness strategy adapter does not support type, column, query, or filter"
            ),
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness lag tolerance has non-timestamp type",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: sql
              query: SELECT MAX(version) FROM raw.orders
              type: integer
              lag_tolerance: 10m
        """,
            expected_error_fragment="source freshness lag_tolerance requires type timestamp",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness lag tolerance is invalid",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: sql
              query: SELECT MAX(updated_at) FROM raw.orders
              type: timestamp
              lag_tolerance: soon
        """,
            expected_error_fragment="source freshness lag_tolerance must be a positive duration",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness age policy has non-timestamp type",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: column
              column: batch_id
              type: integer
              age_policy:
                error_after: 1d
        """,
            expected_error_fragment="source freshness age_policy requires type timestamp",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness age policy duration is invalid",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: column
              column: updated_at
              type: timestamp
              age_policy:
                warn_after: soon
        """,
            expected_error_fragment="source freshness age_policy values must be positive durations",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness age policy has unknown duration unit",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: column
              column: updated_at
              type: timestamp
              age_policy:
                warn_after: 1w
        """,
            expected_error_fragment="source freshness age_policy values must be positive durations",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness age policy warning exceeds error",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: column
              column: updated_at
              type: timestamp
              age_policy:
                warn_after: 2d
                error_after: 12h
        """,
            expected_error_fragment=(
                "source freshness age_policy warn_after must be less than or equal to error_after"
            ),
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness age policy is empty",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: column
              column: updated_at
              type: timestamp
              age_policy: {}
        """,
            expected_error_fragment="source freshness age_policy requires warn_after or error_after",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source freshness lag tolerance uses seconds",
            contents="""
        sources:
          - name: raw_orders
            freshness:
              strategy: sql
              query: SELECT MAX(updated_at) FROM raw.orders
              type: timestamp
              lag_tolerance: 30s
        """,
            expected_error_fragment="source freshness lag_tolerance must be a positive duration",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source audits is not a list",
            contents="""
        sources:
          - name: raw_orders
            audits: {}
        """,
            expected_error_fragment="source audits must be a list",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source audits is null",
            contents="""
        sources:
          - name: raw_orders
            audits:
        """,
            expected_error_fragment="source audits must be a list",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source columns is not a list",
            contents="""
        sources:
          - name: raw_orders
            columns: {}
        """,
            expected_error_fragment="source columns must be a list",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source column meta is not a mapping",
            contents="""
        sources:
          - name: raw_orders
            columns:
              - name: order_id
                meta: []
        """,
            expected_error_fragment="source column 'meta' must be a mapping",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when a source column entry is not a mapping",
            contents="""
        sources:
          - name: raw_orders
            columns:
              - order_id
        """,
            expected_error_fragment="source columns must contain only mappings",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source column omits name",
            contents="""
        sources:
          - name: raw_orders
            columns:
              - type: VARCHAR
        """,
            expected_error_fragment="source column must define non-empty string 'name'",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source column audits is not a list",
            contents="""
        sources:
          - name: raw_orders
            columns:
              - name: order_id
                audits: {}
        """,
            expected_error_fragment="source column audits must be a list",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source column nullable is not a boolean",
            contents="""
        sources:
          - name: raw_orders
            columns:
              - name: order_id
                nullable: 123
        """,
            expected_error_fragment="source column 'nullable' must be a boolean",
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source column allows nulls and uses not null audit",
            contents="""
        sources:
          - name: raw_orders
            columns:
              - name: order_id
                nullable: true
                audits:
                  - not_null
        """,
            expected_error_fragment=(
                "column 'order_id' cannot set nullable = true and audit not_null"
            ),
        ),
        ParseSourcesYamlErrorTestCase(
            description="raises when source column type is blank",
            contents="""
        sources:
          - name: raw_orders
            columns:
              - name: order_id
                type: ""
        """,
            expected_error_fragment="source column 'type' must be a non-empty string",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_sources_yaml_when_parsing_then_it_raises_clear_errors(
    test_case: ParseSourcesYamlErrorTestCase,
) -> None:
    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        parse_sources_yml(contents=test_case.contents, file_path=Path("sql/sources/raw.yml"))
