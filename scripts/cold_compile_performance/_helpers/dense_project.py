"""Generate bounded CTE graphs with full compiler-rule evaluation."""

from bisect import bisect_right
from pathlib import Path

from scripts.cold_compile_performance.constants import (
    DENSE_AMOUNT_AUDIT_SHARE,
    DENSE_DEEP_CTE_PERCENTILE,
    DENSE_ID_COLUMN,
    DENSE_JOIN_COUNTS,
    DENSE_JOIN_THRESHOLDS,
    DENSE_MACRO_QUANTITY_COLUMNS,
    DENSE_MACRO_SHARE,
    DENSE_MEDIUM_CTE_PERCENTILE,
    DENSE_UNION_MEDIUM_INDEX,
    DENSE_UNION_TAIL_INDEX,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    _layered_write_functions,
    _layered_write_macros,
    _layered_write_seeds,
)


def dense_model_name(index: int) -> str:
    return f"sales{index // 1000:03d}__int_clean__orders_{index:05d}"


def dense_model_path(*, project_dir: Path, index: int) -> Path:
    """The file of the generated dense model at `index`."""

    return (
        project_dir
        / "models"
        / f"sales{index // 1000:03d}"
        / "intermediate"
        / "clean"
        / f"batch{index // 5:05d}"
        / f"{dense_model_name(index)}.sql"
    )


def dense_test_path(*, project_dir: Path, index: int) -> Path:
    """The SQL test file of the generated dense model at `index`."""

    name: str = dense_model_name(index).replace("__", "_")
    return next((project_dir / "tests").rglob(f"test_{name}__*.sql"))


def dense_column_count(index: int) -> int:
    rank: int = (index * 17) % 1000
    for upper, width in (
        (100, 8),
        (250, 12),
        (450, 16),
        (600, 20),
        (730, 24),
        (830, 32),
        (900, 48),
        (940, 64),
        (970, 96),
        (990, 160),
    ):
        if rank < upper:
            return width
    return 256 + (rank - 990) * 24


def write_dense_compile_project(*, project_dir: Path, model_count: int) -> None:
    """Write a deterministic query-heavy project without external inputs or services."""

    project_dir.mkdir(parents=True)
    domains: str = ", ".join(
        f'"sales{cohort:03d}"' for cohort in range((model_count + 999) // 1000)
    )
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "dense_orders"\nadapter = "snowflake"\n'
        '[defaults]\ndatabase = "warehouse"\nschema = "analytics"\n'
        f'[rules]\nselect = ["SQBR", "SQLCLEAN", "XSQBR"]\ndomains = [{domains}]\n',
        encoding="utf-8",
    )
    source_count: int = max(1, model_count * 24 // 100)
    seed_count: int = max(1, model_count // 20)
    function_count: int = max(1, model_count // 40)
    _write_sources(project_dir=project_dir, source_count=source_count)
    _layered_write_seeds(project_dir=project_dir, seed_count=seed_count)
    _layered_write_functions(project_dir=project_dir, function_count=function_count)
    for cohort in range((model_count + 999) // 1000):
        models: Path = project_dir / "models" / f"sales{cohort:03d}" / "intermediate" / "clean"
        models.mkdir(parents=True)
        macro_count: int = max(1, min(1000, model_count - cohort * 1000) // 80)
        _layered_write_macros(project_dir=project_dir, macro_count=macro_count)
        macro_root: Path = project_dir / "macros" / "python"
        for macro_index, macro_path in enumerate(sorted(macro_root.glob("*.py"))):
            bucket: Path = (
                models / "_sqlbuild" / "macros" / "python" / f"adjustment{macro_index // 8:03d}"
            )
            bucket.mkdir(parents=True, exist_ok=True)
            content: str = macro_path.read_text(encoding="utf-8")
            content = content.replace("macro_", f"macro_{cohort:03d}_").replace(
                "base_offset", f"base_offset_{cohort:03d}"
            )
            (bucket / f"offset{macro_index:05d}.py").write_text(content, encoding="utf-8")
            macro_path.unlink()
        macro_root.rmdir()
        macro_root.parent.rmdir()
    for index in range(model_count):
        name: str = dense_model_name(index)
        domain: str = f"sales{index // 1000:03d}"
        model_owner: Path = (
            project_dir / "models" / domain / "intermediate" / "clean" / f"batch{index // 5:05d}"
        )
        test_owner: Path = (
            project_dir
            / "tests"
            / "unit"
            / domain
            / "intermediate"
            / "clean"
            / f"batch{index // 5:05d}"
        )
        macro_count = max(1, min(1000, model_count - index // 1000 * 1000) // 80)
        model_owner.mkdir(parents=True, exist_ok=True)
        test_owner.mkdir(parents=True, exist_ok=True)
        (model_owner / f"{name}.sql").write_text(
            _model_sql(
                index=index,
                source_count=source_count,
                function_count=function_count,
                macro_count=macro_count,
            ),
            encoding="utf-8",
        )
        (test_owner / f"test_{name.replace('__', '_')}__preserves_order_id.sql").write_text(
            _test_sql(index=index, source_count=source_count, macro_count=macro_count),
            encoding="utf-8",
        )
    rules: Path = project_dir / "rules"
    rules.mkdir()
    (rules / "orders.py").write_text(
        "from sqlbuild.rules import Finding, Project, RuleContext, rule\n\n"
        '@rule(code="XSQBRBENCH001", message="Order contracts must be present", '
        'remediation="Declare the order contract.")\n'
        "def order_contracts(*, project: Project, ctx: RuleContext) -> list[Finding]:\n"
        "    findings: list[Finding] = []\n"
        "    for model in ctx.project.models:\n"
        "        if not ctx.contracts.enforced(model):\n"
        "            findings.append(ctx.finding(subject=model))\n"
        "    return findings\n",
        encoding="utf-8",
    )
    (project_dir / "tests" / "test_rule_contracts.py").write_text(
        "import pytest\n"
        "from sqlbuild.rules.testing import RuleCase, evaluate_rule\n"
        "from rules.orders import order_contracts\n\n"
        "CASES = (\n"
        '    RuleCase(description="enforced contract", source="MODEL (contract enforced, '
        'columns (id (type INTEGER))); SELECT CAST(1 AS INTEGER) AS id", '
        "expected_finding_count=0),\n"
        '    RuleCase(description="missing contract", source="MODEL (); SELECT 1 AS id", '
        "expected_finding_count=1),\n"
        ")\n\n"
        '@pytest.mark.parametrize("test_case", CASES, ids=lambda case: case.description)\n'
        "def test_given_contract_when_checking_then_reports_expected_findings(test_case):\n"
        "    result = evaluate_rule(rule=order_contracts, test_case=test_case)\n"
        "    assert result.finding_count == test_case.expected_finding_count\n",
        encoding="utf-8",
    )


def _write_sources(*, project_dir: Path, source_count: int) -> None:
    sources: Path = project_dir / "sources"
    sources.mkdir()
    entries: list[str] = ["sources:"]
    for index in range(source_count):
        width: int = dense_column_count(index % 240)
        extra: str = "".join(
            f", CAST(1 AS DOUBLE) AS quantity_{column:03d}" for column in range(width - 2)
        )
        entries.extend(
            (
                f"  - name: orders_{index:05d}",
                f"    description: Generated source orders_{index:05d}.",
                "    contract: enforced",
                f'    expression: "(SELECT 1 AS id, CAST(1 AS DOUBLE) AS amount{extra})"',
                "    columns:",
                "      - name: id",
                "        type: INTEGER",
                "        nullable: false",
                "      - name: amount",
                "        type: DOUBLE",
            )
        )
        for column in range(width - 2):
            entries.extend(
                (
                    f"      - name: quantity_{column:03d}",
                    "        type: DOUBLE",
                    "        nullable: true",
                )
            )
    (sources / "orders.yml").write_text("\n".join(entries) + "\n", encoding="utf-8")


def _join_count(index: int) -> int:
    rank: int = (index * 37) % 100
    return DENSE_JOIN_COUNTS[bisect_right(DENSE_JOIN_THRESHOLDS, rank)]


def _branch_count(index: int) -> int:
    return (
        128
        if index % 1000 == DENSE_UNION_TAIL_INDEX
        else 20
        if index % 20 == 1
        else 8
        if index % 10 == DENSE_UNION_MEDIUM_INDEX
        else 1
        if index % 10 == 0
        else 2
    )


def _test_sql(*, index: int, source_count: int, macro_count: int) -> str:
    name: str = dense_model_name(index)
    mocks: dict[str, str] = {}
    base: str = (
        f"__source__orders_{_source_index(index=index, source_count=source_count):05d}"
        if index % 1000 % 48 == 0
        else f"__ref__{dense_model_name(index - 1)}"
    )
    mocks[base] = "SELECT CAST(1 AS INTEGER) AS id, CAST(1 AS DOUBLE) AS amount"
    for lookup in range(_join_count(index)):
        source_index: int = _source_index(index=index, source_count=source_count, offset=lookup + 1)
        mocks[f"__source__orders_{source_index:05d}"] = mocks[base]
    expected_id: int = 1
    if index % 10 < DENSE_MACRO_SHARE:
        expected_id += index % 1000 % macro_count or 1
    adjustment: int = expected_id - 1
    amount: int = 1 + _join_count(index) + adjustment + int(index % 1000 % 43 == 0)
    expected_columns: list[str] = [
        f"CAST({expected_id} AS INTEGER) AS id",
        f"CAST({amount} AS DOUBLE) AS amount",
    ]
    for column in range(dense_column_count(index) - 2):
        quantity: int = 2 + (adjustment if column < DENSE_MACRO_QUANTITY_COLUMNS else 0)
        expected_columns.append(f"CAST({quantity} AS DOUBLE) AS quantity_{column:03d}")
    expected: str = "SELECT " + ", ".join(expected_columns)
    ctes: list[str] = [f"{mock} AS ({body})" for mock, body in mocks.items()]
    ctes.append(f"__expected__{name} AS ({expected})")
    return f'TEST (name "{name}__preserves_order_id");\nWITH ' + ",\n".join(ctes) + "\nSELECT 1\n"


def _source_index(*, index: int, source_count: int, offset: int = 0) -> int:
    start: int = index // 1000 * 240
    return start + (index % 1000 + offset) % min(240, source_count - start)


def _model_sql(*, index: int, source_count: int, function_count: int, macro_count: int) -> str:
    width: int = dense_column_count(index)
    names: list[str] = ["id", "amount", *(f"quantity_{column:03d}" for column in range(width - 2))]
    declarations: list[str] = []
    for column, name in enumerate(names):
        data_type: str = "INTEGER" if column == 0 else "DOUBLE"
        audit: str = (
            ", audits [not_null]"
            if column == 0 or (column == 1 and index % 10 < DENSE_AMOUNT_AUDIT_SHARE)
            else ""
        )
        nullable: str = "false" if audit else "true"
        declarations.append(
            f"    {name} (type {data_type}, nullable {nullable}, "
            f'description "Order quantity measure"{audit})'
        )
    header: str = (
        'MODEL (description "Generated dense model.", materialized table, contract enforced, '
        "columns (\n" + ",\n".join(declarations) + "\n));\n"
    )
    source_index: int = _source_index(index=index, source_count=source_count)
    relation: str = (
        f'__source("orders_{source_index:05d}")'
        if index % 1000 % 48 == 0
        else f'__ref("{dense_model_name(index - 1)}")'
    )
    ctes: list[str] = [f"imported AS (SELECT * FROM {relation})"]
    rank: int = (index * 37) % 100
    joins: int = _join_count(index)
    imported_sources: dict[int, str] = {source_index: "imported"} if index % 1000 % 48 == 0 else {}
    lookup_names: list[str] = []
    for lookup in range(joins):
        source_index = _source_index(index=index, source_count=source_count, offset=lookup + 1)
        if source_index not in imported_sources:
            imported_sources[source_index] = f"lookup_{lookup}"
            ctes.append(f'lookup_{lookup} AS (SELECT * FROM __source("orders_{source_index:05d}"))')
        lookup_names.append(imported_sources[source_index])
    amount: str = "b.amount" + "".join(
        f" + COALESCE(l{lookup}.amount, 0)" for lookup in range(joins)
    )
    if index % 10 < DENSE_MACRO_SHARE:
        amount = f'@py.macro_{index // 1000:03d}_{index % 1000 % macro_count:05d}("{amount}")'
    if index % 1000 % 43 == 0:
        function_start: int = index // 1000 * 25
        function_index: int = function_start + index % 1000 % min(
            25, function_count - function_start
        )
        amount = f'__udf("fn_{function_index:05d}")({amount})'
    identity: str = (
        f'@py.macro_{index // 1000:03d}_{index % 1000 % macro_count:05d}("b.id")'
        if index % 10 < DENSE_MACRO_SHARE
        else "b.id"
    )
    projections: list[str] = [
        f"CAST({identity} AS INTEGER) AS id",
        f"CAST({amount} AS DOUBLE) AS amount",
    ]
    for column, name in enumerate(names[2:]):
        expression: str = "COALESCE(b.amount, 0) + CAST(b.id AS DOUBLE)"
        if index % 10 < DENSE_MACRO_SHARE and column < DENSE_MACRO_QUANTITY_COLUMNS:
            expression = (
                f'@py.macro_{index // 1000:03d}_{index % 1000 % macro_count:05d}("{expression}")'
            )
        projections.append(f"CAST({expression} AS DOUBLE) AS {name}")
    join_sql: str = "".join(
        f"\nLEFT JOIN {lookup_names[lookup]} AS l{lookup} ON l{lookup}.id = b.id"
        for lookup in range(joins)
    )
    ctes.append(
        "projected AS (SELECT\n  "
        + ",\n  ".join(projections)
        + "\nFROM imported AS b"
        + join_sql
        + ")"
    )
    columns: str = ", ".join(names)
    branches: int = _branch_count(index)
    union_branches: list[str] = [f"SELECT {columns} FROM projected"]
    union_branches.extend(
        f"SELECT {columns} FROM projected WHERE id = 0" for _ in range(branches - 1)
    )
    ctes.append("combined AS (\n" + "\nUNION ALL BY NAME\n".join(union_branches) + "\n)")
    previous: str = "combined"
    depth: int = (
        12 if rank >= DENSE_DEEP_CTE_PERCENTILE else 4 if rank >= DENSE_MEDIUM_CTE_PERCENTILE else 0
    )
    for step in range(depth):
        current: str = f"calculated_{step}"
        ctes.append(f"{current} AS (SELECT {columns} FROM {previous})")
        previous = current
    casts: str = ",\n  ".join(
        f"CAST({name} AS {'INTEGER' if name == DENSE_ID_COLUMN else 'DOUBLE'}) AS {name}"
        for name in names
    )
    ctes.append(f"final AS (SELECT\n  {casts}\nFROM {previous})")
    return header + "\nWITH " + ",\n".join(ctes) + f"\nSELECT {columns} FROM final\n"
