from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import cast

import pytest

from sqlbuild._native import (
    DiscoveredSqlHookFile,
    SqlHookParseError,
    discover_sql_hook_files,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    DiscoverSqlHooksTestCase,
    ParseSqlHookErrorTestCase,
    ParseSqlHookTestCase,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers.helpers import discover_declaration_file


@pytest.mark.parametrize(
    "test_case",
    [
        ParseSqlHookTestCase(
            description="parses hook description and SQL body",
            contents=('HOOK (description "Grant: access");\n\n@py.grant(role: "analyst:read")\n'),
            expected_name="grant_access",
            expected_description="Grant: access",
            expected_sql_body='@py.grant(role: "analyst:read")',
        ),
        ParseSqlHookTestCase(
            description="allows header-like text inside strings and comments",
            contents=("HOOK ();\n\nSELECT 'first\nHOOK ();\nlast' AS text /*\nHOOK ();\n*/;\n"),
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body=("SELECT 'first\nHOOK ();\nlast' AS text /*\nHOOK ();\n*/;"),
        ),
        ParseSqlHookTestCase(
            description="allows header terminator text inside quoted descriptions",
            contents='HOOK (description "Run cleanup(); safely");\n\nSELECT 1\n',
            expected_name="grant_access",
            expected_description="Run cleanup(); safely",
            expected_sql_body="SELECT 1",
        ),
        ParseSqlHookTestCase(
            description="allows semicolons inside dollar quotes and bracketed identifiers",
            contents=(
                "HOOK ();\n\n"
                "DO $$ BEGIN RAISE NOTICE 'value;still-string'; "
                "PERFORM [procedure;name]; END $$;\n"
            ),
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body=(
                "DO $$ BEGIN RAISE NOTICE 'value;still-string'; PERFORM [procedure;name]; END $$;"
            ),
        ),
        ParseSqlHookTestCase(
            description="allows semicolons inside a procedural begin end block",
            contents="HOOK ();\n\nBEGIN SELECT 1; SELECT 2; END;\n",
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="BEGIN SELECT 1; SELECT 2; END;",
        ),
        ParseSqlHookTestCase(
            description="allows create procedure statements with an internal block",
            contents=("HOOK ();\n\nCREATE PROCEDURE p AS\nBEGIN\n  SELECT 1;\n  SELECT 2;\nEND;\n"),
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body=("CREATE PROCEDURE p AS\nBEGIN\n  SELECT 1;\n  SELECT 2;\nEND;"),
        ),
        ParseSqlHookTestCase(
            description="allows SQL Server conditional blocks",
            contents="HOOK ();\n\nIF 1 = 1 BEGIN SELECT 1; SELECT 2; END;\n",
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="IF 1 = 1 BEGIN SELECT 1; SELECT 2; END;",
        ),
        ParseSqlHookTestCase(
            description="allows SQL Server if else blocks",
            contents=("HOOK ();\n\nIF 1 = 1 BEGIN SELECT 1; END ELSE BEGIN SELECT 2; END;\n"),
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body=("IF 1 = 1 BEGIN SELECT 1; END ELSE BEGIN SELECT 2; END;"),
        ),
        ParseSqlHookTestCase(
            description="allows SQL Server if else branches without blocks",
            contents="HOOK ();\n\nIF 1 = 1 SELECT 1 ELSE SELECT 2\n",
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="IF 1 = 1 SELECT 1 ELSE SELECT 2",
        ),
        ParseSqlHookTestCase(
            description="allows SQL Server try catch blocks",
            contents=("HOOK ();\n\nBEGIN TRY SELECT 1; END TRY BEGIN CATCH SELECT 2; END CATCH;\n"),
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body=("BEGIN TRY SELECT 1; END TRY BEGIN CATCH SELECT 2; END CATCH;"),
        ),
        ParseSqlHookTestCase(
            description="allows comments before unresolved hook arguments",
            contents="HOOK ();\n\n-- supplied by the model\n@statement\n",
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="-- supplied by the model\n@statement",
        ),
        ParseSqlHookTestCase(
            description="allows standalone values statements",
            contents="HOOK ();\n\nVALUES (1)\n",
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="VALUES (1)",
        ),
        ParseSqlHookTestCase(
            description="allows go as a query identifier",
            contents="HOOK ();\n\nSELECT go FROM audit_log\n",
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="SELECT go FROM audit_log",
        ),
        ParseSqlHookTestCase(
            description="allows go as a query identifier on its own line",
            contents="HOOK ();\n\nSELECT\n  go\nFROM audit_log\n",
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="SELECT\n  go\nFROM audit_log",
        ),
        ParseSqlHookTestCase(
            description="allows insert select composite statements",
            contents="HOOK ();\n\nINSERT INTO target SELECT * FROM source\n",
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="INSERT INTO target SELECT * FROM source",
        ),
        ParseSqlHookTestCase(
            description="allows grant privileges named like statement roots",
            contents=("HOOK ();\n\nGRANT CREATE ON SCHEMA public TO analyst;\n"),
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="GRANT CREATE ON SCHEMA public TO analyst;",
        ),
        ParseSqlHookTestCase(
            description="allows SQL Server deny statements",
            contents=("HOOK ();\n\nDENY CONTROL ON DATABASE::analytics TO analyst\n"),
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body=("DENY CONTROL ON DATABASE::analytics TO analyst"),
        ),
        ParseSqlHookTestCase(
            description="allows SQL Server deny select statements",
            contents=("HOOK ();\n\nDENY SELECT ON OBJECT::orders TO analyst\n"),
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="DENY SELECT ON OBJECT::orders TO analyst",
        ),
        ParseSqlHookTestCase(
            description="allows privilege recipients named like statement roots",
            contents=("HOOK ();\n\nDENY SELECT ON OBJECT::orders TO analyze\n"),
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="DENY SELECT ON OBJECT::orders TO analyze",
        ),
        ParseSqlHookTestCase(
            description="allows explain with queries",
            contents=("HOOK ();\n\nEXPLAIN WITH orders AS (SELECT 1 AS id) SELECT * FROM orders\n"),
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body=("EXPLAIN WITH orders AS (SELECT 1 AS id) SELECT * FROM orders"),
        ),
        ParseSqlHookTestCase(
            description="allows SQL Server if insert select branches",
            contents=("HOOK ();\n\nIF 1 = 1 INSERT INTO target SELECT * FROM source\n"),
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body=("IF 1 = 1 INSERT INTO target SELECT * FROM source"),
        ),
        ParseSqlHookTestCase(
            description="preserves multiple statements as one execution payload",
            contents="HOOK ();\n\nDELETE FROM staging;\nVACUUM staging;\n",
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="DELETE FROM staging;\nVACUUM staging;",
        ),
        ParseSqlHookTestCase(
            description="preserves vendor SQL without classifying its grammar",
            contents="HOOK ();\n\nDBCC CHECKIDENT ('orders', RESEED, 0);\n",
            expected_name="grant_access",
            expected_description=None,
            expected_sql_body="DBCC CHECKIDENT ('orders', RESEED, 0);",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_hook_header_and_sql_when_parsing_then_returns_named_resource(
    test_case: ParseSqlHookTestCase,
    tmp_path: Path,
) -> None:
    hook: DiscoveredSqlHookFile = cast(
        DiscoveredSqlHookFile,
        discover_declaration_file(
            project_dir=tmp_path,
            relative_path="sql/hooks/sql/grant_access.sql",
            contents=test_case.contents,
        ),
    )

    assert hook.name == test_case.expected_name
    assert hook.description == test_case.expected_description
    assert hook.sql_body == test_case.expected_sql_body
    assert hook.relative_path == Path("sql/hooks/sql/grant_access.sql")


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverSqlHooksTestCase(
            description="discovers nested public SQL hooks only",
            repo_files={
                "sql/hooks/sql/admin/grant_access.sql": "HOOK ();\n\nSELECT 1\n",
                "sql/hooks/sql/_private.sql": "not a hook\n",
                "sql/hooks/python/not_sql.sql": "not a hook\n",
            },
            expected_names=("grant_access",),
            expected_paths=("sql/hooks/sql/admin/grant_access.sql",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_nested_sql_hook_files_when_discovering_then_names_come_from_stems(
    test_case: DiscoverSqlHooksTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    hooks: tuple[DiscoveredSqlHookFile, ...] = discover_sql_hook_files(project_dir=tmp_path)

    assert tuple(hook.name for hook in hooks) == test_case.expected_names
    assert tuple(hook.relative_path.as_posix() for hook in hooks) == test_case.expected_paths


@pytest.mark.parametrize(
    "test_case",
    [
        ParseSqlHookErrorTestCase(
            description="rejects a missing SQL body",
            contents="HOOK ();\n",
            expected_error_fragment="must define SQL after HOOK",
        ),
        ParseSqlHookErrorTestCase(
            description="rejects unsupported hook header key",
            contents='HOOK (name "override");\nSELECT 1\n',
            expected_error_fragment="unsupported keys: name",
        ),
        ParseSqlHookErrorTestCase(
            description="rejects a missing hook header",
            contents="SELECT 1\n",
            expected_error_fragment="must start with a HOOK",
        ),
        ParseSqlHookErrorTestCase(
            description="rejects the old colon syntax",
            contents='HOOK (description: "legacy");\nSELECT 1\n',
            expected_error_fragment="unexpected ':' after key 'description'",
        ),
        ParseSqlHookErrorTestCase(
            description="rejects duplicate hook keys",
            contents='HOOK (description "first", description "second");\nSELECT 1\n',
            expected_error_fragment="duplicate.*description",
        ),
        ParseSqlHookErrorTestCase(
            description="rejects a non-string description",
            contents="HOOK (description 123);\nSELECT 1\n",
            expected_error_fragment="description.*non-empty string",
        ),
        ParseSqlHookErrorTestCase(
            description="rejects an explicit null description",
            contents="HOOK (description null);\nSELECT 1\n",
            expected_error_fragment="description.*non-empty string",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_sql_hook_when_parsing_then_raises(
    test_case: ParseSqlHookErrorTestCase,
    tmp_path: Path,
) -> None:
    with pytest.raises(SqlHookParseError, match=test_case.expected_error_fragment):
        discover_declaration_file(
            project_dir=tmp_path, relative_path="sql/hooks/sql/invalid.sql", contents=test_case.contents
        )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
