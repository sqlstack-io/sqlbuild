from __future__ import annotations

from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import PythonHookEntry, SqlHookEntry
from tests.unit.src.sqlbuild.compiler.planner.main.identity._test_types import (
    HookIdentityPayloadTestCase,
    HookIdentityTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner.main.identity.helpers import (
    build_hooked_model_identity_json,
)

_RECORD_ACCESS: SqlHookEntry = SqlHookEntry(
    statement="INSERT INTO order_audit SELECT 'orders'",
    name="record_order_access",
    relative_path=Path("sql/hooks/sql/record_order_access.sql"),
    definition_sql="INSERT INTO order_audit SELECT {{ table_name }}",
    kwargs={"table_name": "'orders'"},
    description="Record order table access",
)
_ANALYZE: SqlHookEntry = SqlHookEntry(statement="ANALYZE orders")
_NOTIFY: PythonHookEntry = PythonHookEntry(name="notify_orders", kwargs={"channel": "orders"})


@pytest.mark.parametrize(
    "test_case",
    [
        HookIdentityTestCase(
            description="ignores a hook description change",
            original_hooks={"pre_hooks": [_RECORD_ACCESS]},
            changed_hooks={"pre_hooks": [replace(_RECORD_ACCESS, description="Audit order reads")]},
            expected_changed=False,
        ),
        HookIdentityTestCase(
            description="ignores a hook file location change",
            original_hooks={"pre_hooks": [_RECORD_ACCESS]},
            changed_hooks={
                "pre_hooks": [
                    replace(
                        _RECORD_ACCESS,
                        relative_path=Path("sql/hooks/sql/audit/record_order_access.sql"),
                    )
                ]
            },
            expected_changed=False,
        ),
        HookIdentityTestCase(
            description="changes when hook SQL changes",
            original_hooks={"pre_hooks": [_RECORD_ACCESS]},
            changed_hooks={
                "pre_hooks": [
                    replace(_RECORD_ACCESS, statement="INSERT INTO order_audit SELECT 'all'")
                ]
            },
            expected_changed=True,
        ),
        HookIdentityTestCase(
            description="changes when hook definition changes",
            original_hooks={"pre_hooks": [_RECORD_ACCESS]},
            changed_hooks={
                "pre_hooks": [
                    replace(
                        _RECORD_ACCESS,
                        definition_sql="INSERT INTO order_audit SELECT {{ table_name }} -- v2",
                    )
                ]
            },
            expected_changed=True,
        ),
        HookIdentityTestCase(
            description="changes when hook arguments change",
            original_hooks={"pre_hooks": [_RECORD_ACCESS]},
            changed_hooks={
                "pre_hooks": [replace(_RECORD_ACCESS, kwargs={"table_name": "'customers'"})]
            },
            expected_changed=True,
        ),
        HookIdentityTestCase(
            description="changes when hook name changes",
            original_hooks={"pre_hooks": [_RECORD_ACCESS]},
            changed_hooks={"pre_hooks": [replace(_RECORD_ACCESS, name="record_access")]},
            expected_changed=True,
        ),
        HookIdentityTestCase(
            description="changes when a hook moves between phases",
            original_hooks={"pre_hooks": [_RECORD_ACCESS]},
            changed_hooks={"post_hooks": [_RECORD_ACCESS]},
            expected_changed=True,
        ),
        HookIdentityTestCase(
            description="changes when hook order changes",
            original_hooks={"post_hooks": [_RECORD_ACCESS, _ANALYZE]},
            changed_hooks={"post_hooks": [_ANALYZE, _RECORD_ACCESS]},
            expected_changed=True,
        ),
        HookIdentityTestCase(
            description="changes when Python hook arguments change",
            original_hooks={"post_hooks": [_NOTIFY]},
            changed_hooks={"post_hooks": [replace(_NOTIFY, kwargs={"channel": "customers"})]},
            expected_changed=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_hook_change_when_building_model_identity_then_tracks_only_execution_fields(
    test_case: HookIdentityTestCase,
) -> None:
    original_json: str = build_hooked_model_identity_json(hook_config=test_case.original_hooks)
    changed_json: str = build_hooked_model_identity_json(hook_config=test_case.changed_hooks)

    assert (original_json != changed_json) is test_case.expected_changed


@pytest.mark.parametrize(
    "test_case",
    [
        HookIdentityPayloadTestCase(
            description="keeps execution fields and omits documentation fields",
            hooks={"pre_hooks": [_RECORD_ACCESS], "post_hooks": [_NOTIFY]},
            expected_fragments=(
                '"definition_sql":"INSERT INTO order_audit SELECT {{ table_name }}"',
                '"name":"record_order_access"',
                '"version_hash":"notify-hash-v1"',
            ),
            forbidden_fragments=(
                "Record order table access",
                "sql/hooks/sql/record_order_access.sql",
                '"description"',
                '"relative_path"',
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_documented_hooks_when_building_model_identity_then_omits_doc_fields(
    test_case: HookIdentityPayloadTestCase,
) -> None:
    identity_json: str = build_hooked_model_identity_json(hook_config=test_case.hooks)

    assert all(fragment in identity_json for fragment in test_case.expected_fragments)
    assert not any(fragment in identity_json for fragment in test_case.forbidden_fragments)
