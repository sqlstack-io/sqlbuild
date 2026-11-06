# ruff: noqa: E501, PLR0915
"""Write the same dense project for SQLBuild (DuckDB) and dbt (DuckDB).

Usage, from the repository root:

    python benchmarks/dbt-compare/write_dbt_dense.py --models N --out DIR [--seed-error ref|column]

Creates under DIR:
  sqlbuild/          SQLBuild dense project (the CI dense generator, adapter switched to duckdb)
  sqlbuild-norules/  the same project with `[rules] select = []`
  dbt/               dbt project derived file by file from `sqlbuild/`; sources use their declared
                     columns (`schema_origin: local`), as SQLBuild does
  dbt-remote/        the same dbt project with sources read from the warehouse (dbt's default)
  warehouse.duckdb   DuckDB file holding every source and model relation (empty), as after a build
  profiles/profiles.yml

The SQLBuild project comes unchanged from the generator the compile-performance CI jobs use. The
dbt project is derived from the generated SQLBuild files, so SQL, DAG, names and folders match.
"""

import argparse
import re
import shutil
import sys
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPOSITORY_ROOT))

from scripts.cold_compile_performance._helpers.dense_project import (  # noqa: E402
    dense_column_count,
    dense_model_name,
    write_dense_compile_project,
)

SOURCE_NAME = "raw"
SOURCE_SCHEMA = "raw"
DATABASE = "warehouse"
ERROR_MODEL_INDEX = 2
ERROR_COLUMN = "missing_col"
ERROR_REF = "sales000__int_clean__orders_missing"

COLUMN_RE = re.compile(
    r'^\s+(\w+) \(type (\w+), nullable (true|false), description "([^"]*)"(, audits \[not_null\])?\)'
)
MACRO_CALL_RE = re.compile(r'@py\.(macro_\d{3}_\d{5})\("([^"]*)"\)')
CAST_RE = re.compile(r"CAST\(([^ ]+) AS (\w+)\) AS (\w+)")


def q(text: str) -> str:
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def write_sqlbuild(out: Path, n: int) -> Path:
    project = out / "sqlbuild"
    write_dense_compile_project(project_dir=project, model_count=n)
    toml = project / "sqlbuild_project.toml"
    text = toml.read_text()
    assert 'adapter = "snowflake"' in text
    toml.write_text(text.replace('adapter = "snowflake"', 'adapter = "duckdb"'))
    return project


def write_norules(project: Path) -> Path:
    target = project.parent / "sqlbuild-norules"
    shutil.copytree(project, target)
    toml = target / "sqlbuild_project.toml"
    text = toml.read_text()
    new = text.replace('select = ["SQBR", "SQLCLEAN", "XSQBR"]', "select = []")
    assert new != text
    toml.write_text(new)
    return target


def model_header_and_body(text: str) -> tuple[list[tuple[str, str, bool, str, bool]], str]:
    header, body = text.split("\n));\n", 1)
    columns = []
    for line in header.splitlines()[1:]:
        m = COLUMN_RE.match(line)
        assert m, line
        columns.append((m[1], m[2], m[3] == "false", m[4], bool(m[5])))
    return columns, body


def to_jinja_sql(body: str) -> str:
    body = re.sub(r'__ref\("([^"]+)"\)', r"{{ ref('\1') }}", body)
    body = re.sub(r'__source\("([^"]+)"\)', rf"{{{{ source('{SOURCE_NAME}', '\1') }}}}", body)
    body = re.sub(r'__udf\("([^"]+)"\)\(', r"{{ function('\1') }}(", body)
    body = MACRO_CALL_RE.sub(lambda m: "{{ " + m[1] + "(" + q(m[2]) + ") }}", body)
    assert not re.search(r"__(ref|source|udf)\(", body), body[:400]
    assert "@" not in body
    return body.lstrip("\n")


def parse_cast_row(select_list: str) -> dict[str, str]:
    return {col: value for value, _type, col in CAST_RE.findall(select_list)}


def parse_test(text: str) -> tuple[dict[str, dict[str, str]], dict[str, str]]:
    inputs: dict[str, dict[str, str]] = {}
    expected: dict[str, str] = {}
    for line in text.splitlines():
        line = line.strip().removeprefix("WITH ").rstrip(",")
        m = re.match(r"(__(ref|source|expected)__(\w+)) AS \(SELECT (.*)\)$", line)
        if not m:
            continue
        row = parse_cast_row(m[4])
        if m[2] == "expected":
            expected = row
        elif m[2] == "ref":
            inputs[f"ref('{m[3]}')"] = row
        else:
            inputs[f"source('{SOURCE_NAME}', '{m[3]}')"] = row
    assert expected and inputs, text[:300]
    return inputs, expected


def flow_row(row: dict[str, str]) -> str:
    return "{" + ", ".join(f"{k}: {v}" for k, v in row.items()) + "}"


def write_dbt(sqb: Path, out: Path, n: int) -> Path:
    dbt = out / "dbt"
    dbt.mkdir()
    (dbt / "dbt_project.yml").write_text(
        "name: dense_orders\n"
        "version: '1.0.0'\n"
        "profile: dense_orders\n"
        "model-paths: [models]\n"
        "macro-paths: [macros]\n"
        "seed-paths: [seeds]\n"
        "function-paths: [functions]\n"
        "test-paths: [tests]\n"
        "models:\n  dense_orders:\n    +materialized: table\n"
    )
    # Sources
    source_count = max(1, n * 24 // 100)
    lines = [
        "sources:",
        f"  - name: {SOURCE_NAME}",
        f"    database: {DATABASE}",
        f"    schema: {SOURCE_SCHEMA}",
        "    config:",
        "      schema_origin: local",
        "    tables:",
    ]
    for index in range(source_count):
        width = dense_column_count(index % 240)
        lines += [
            f"      - name: orders_{index:05d}",
            f"        description: Generated source orders_{index:05d}.",
            "        columns:",
            "          - name: id",
            "            data_type: integer",
            "            constraints: [{type: not_null}]",
            "          - name: amount",
            "            data_type: double",
        ]
        for column in range(width - 2):
            lines += [f"          - name: quantity_{column:03d}", "            data_type: double"]
    (dbt / "models").mkdir()
    (dbt / "models" / "_sources.yml").write_text("\n".join(lines) + "\n")
    # Seeds
    (dbt / "seeds").mkdir()
    seed_yml = ["seeds:"]
    for csv in sorted((sqb / "seeds").glob("*.csv")):
        shutil.copy(csv, dbt / "seeds" / csv.name)
        seed_yml += [
            f"  - name: {csv.stem}",
            f"    description: Test seed {csv.stem}.",
            "    config:",
            "      column_types: {id: integer, label: varchar}",
            "    columns:",
            "      - name: id",
            "      - name: label",
        ]
    (dbt / "seeds" / "_seeds.yml").write_text("\n".join(seed_yml) + "\n")
    # Functions (dbt UDF resources)
    (dbt / "functions").mkdir()
    fn_yml = ["functions:"]
    for fn in sorted((sqb / "functions" / "sql").glob("*.sql")):
        body = fn.read_text().split(");\n", 1)[1].strip()
        (dbt / "functions" / fn.name).write_text(body + "\n")
        fn_yml += [
            f"  - name: {fn.stem}",
            "    description: Test function.",
            "    arguments:",
            "      - name: input_value",
            "        data_type: double",
            "    returns:",
            "      data_type: double",
        ]
    (dbt / "functions" / "_functions.yml").write_text("\n".join(fn_yml) + "\n")
    # Macros: SQLBuild Python macros -> Jinja macros, same per-cohort bucket layout
    macro_def_re = re.compile(
        r"def (macro_\d{3}_\d{5})\(expression: str\) -> str:\n    return (.*)\n"
    )
    offset_re = re.compile(
        r'def _offset\(expression: str\) -> str:\n    return f"\(\{expression\} \+ (\d+)\)"'
    )
    base_re = re.compile(
        r'def (base_offset_\d{3})\(expression: str\) -> str:\n    return f"\(\{expression\} \+ 1\)"'
    )
    for macro in sorted(sqb.glob("models/*/intermediate/clean/_sqlbuild/macros/python/*/*.py")):
        text = macro.read_text()
        name, ret = macro_def_re.search(text).groups()
        offset = offset_re.search(text)[1]
        parts = []
        base = base_re.search(text)
        if base:
            parts.append(
                f"{{% macro {base[1]}(expression) -%}}({{{{ expression }}}} + 1){{%- endmacro %}}"
            )
            assert ret == f"{base[1]}(_offset(expression))"
            parts.append(
                f"{{% macro {name}(expression) -%}}"
                f"{{{{ {base[1]}('(' ~ expression ~ ' + {offset})') }}}}{{%- endmacro %}}"
            )
        else:
            assert ret == "_offset(expression)"
            parts.append(
                f"{{% macro {name}(expression) -%}}({{{{ expression }}}} + {offset}){{%- endmacro %}}"
            )
        domain = macro.relative_to(sqb / "models").parts[0]
        target = dbt / "macros" / domain / macro.parent.name / (macro.stem + ".sql")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("\n".join(parts) + "\n")
    # Models + properties + unit tests, one YAML per batch folder
    for index in range(n):
        name = dense_model_name(index)
        domain = f"sales{index // 1000:03d}"
        batch = f"batch{index // 5:05d}"
        rel = Path(domain) / "intermediate" / "clean" / batch
        src = sqb / "models" / rel / f"{name}.sql"
        columns, body = model_header_and_body(src.read_text())
        out_dir = dbt / "models" / rel
        out_dir.mkdir(parents=True, exist_ok=True)
        (out_dir / f"{name}.sql").write_text(to_jinja_sql(body))
        yml_path = out_dir / "_models.yml"
        if not yml_path.exists():
            yml_path.write_text("models:\n")
        entry = [
            f"  - name: {name}",
            "    description: Generated dense model.",
            "    config:",
            "      materialized: table",
            "      contract: {enforced: true}",
            "    columns:",
        ]
        for col, typ, not_null, desc, audit in columns:
            entry += [
                f"      - name: {col}",
                f"        data_type: {typ.lower()}",
                f"        description: {desc}",
            ]
            if not_null:
                entry.append("        constraints: [{type: not_null}]")
            if audit:
                entry.append("        data_tests: [not_null]")
        with yml_path.open("a") as fh:
            fh.write("\n".join(entry) + "\n")
        test_src = (
            sqb / "tests" / "unit" / rel / f"test_{name.replace('__', '_')}__preserves_order_id.sql"
        )
        inputs, expected = parse_test(test_src.read_text())
        ut_path = out_dir / "_unit_tests.yml"
        if not ut_path.exists():
            ut_path.write_text("unit_tests:\n")
        ut = [f"  - name: {name}__preserves_order_id", f"    model: {name}", "    given:"]
        for inp, row in inputs.items():
            ut += [f"      - input: {inp}", "        rows:", f"          - {flow_row(row)}"]
        ut += ["    expect:", "      rows:", f"        - {flow_row(expected)}"]
        with ut_path.open("a") as fh:
            fh.write("\n".join(ut) + "\n")
    (dbt / "tests").mkdir()
    return dbt


MATERIALIZED_RE = re.compile(r"\bmaterialized (table|view)\b")


def model_relations(sqb: Path) -> list[tuple[str, str, list[tuple[str, str, bool, str, bool]]]]:
    """Name, materialization and columns of every model, read from the SQLBuild headers."""
    relations = []
    for path in sorted((sqb / "models").rglob("*.sql")):
        text = path.read_text()
        materialized = MATERIALIZED_RE.search(text.split("\n", 1)[0])
        assert materialized, path
        columns, _body = model_header_and_body(text)
        relations.append((path.stem, materialized[1], columns))
    return relations


def write_dbt_remote(dbt: Path) -> Path:
    """Copy the dbt project with source columns read from the warehouse, dbt's default."""
    target = dbt.parent / "dbt-remote"
    shutil.copytree(dbt, target)
    sources = target / "models" / "_sources.yml"
    text = sources.read_text()
    new = text.replace("    config:\n      schema_origin: local\n", "", 1)
    assert new != text
    sources.write_text(new)
    return target


def write_warehouse(out: Path, n: int, sqb: Path) -> None:
    """Create every source and model relation empty, as the warehouse looks after `dbt build`.

    dbt v2 reads the columns of unselected upstream models from the warehouse in
    `--static-analysis strict --dirty` runs, so the model relations must exist.
    """
    import duckdb

    path = (out / f"{DATABASE}.duckdb").resolve()
    if path.exists():
        path.unlink()
    con = duckdb.connect(str(path))
    con.execute(f"create schema {SOURCE_SCHEMA}")
    con.execute("create schema analytics")
    for index in range(max(1, n * 24 // 100)):
        width = dense_column_count(index % 240)
        cols = ["id integer not null", "amount double"] + [
            f"quantity_{c:03d} double" for c in range(width - 2)
        ]
        con.execute(f"create table {SOURCE_SCHEMA}.orders_{index:05d} ({', '.join(cols)})")
    for name, materialized, columns in model_relations(sqb):
        if materialized == "table":
            cols = [
                f"{col} {typ.lower()}{' not null' if not_null else ''}"
                for col, typ, not_null, _desc, _audit in columns
            ]
            con.execute(f"create table analytics.{name} ({', '.join(cols)})")
        else:
            cols = [f"cast(null as {typ.lower()}) as {col}" for col, typ, *_ in columns]
            con.execute(f"create view analytics.{name} as select {', '.join(cols)} where false")
    con.close()
    profiles = out / "profiles"
    profiles.mkdir(exist_ok=True)
    (profiles / "profiles.yml").write_text(
        "dense_orders:\n  target: dev\n  outputs:\n    dev:\n      type: duckdb\n"
        f"      path: {path}\n      schema: analytics\n      threads: 4\n"
    )


def seed_error(out: Path, kind: str) -> None:
    """Break model ERROR_MODEL_INDEX the same way in every project.

    `column` selects a column the upstream model does not have; `ref` references a model that does
    not exist.
    """
    name = dense_model_name(ERROR_MODEL_INDEX)
    upstream = dense_model_name(ERROR_MODEL_INDEX - 1)
    for tool in ("sqlbuild", "sqlbuild-norules", "dbt", "dbt-remote"):
        for path in (out / tool / "models").rglob(f"{name}.sql"):
            text = path.read_text()
            if kind == "column":
                new = text.replace(
                    "COALESCE(b.amount, 0) + CAST(b.id AS DOUBLE)",
                    f"COALESCE(b.{ERROR_COLUMN}, 0) + CAST(b.id AS DOUBLE)",
                    1,
                )
            else:
                new = text.replace(f'__ref("{upstream}")', f'__ref("{ERROR_REF}")', 1).replace(
                    f"{{{{ ref('{upstream}') }}}}", f"{{{{ ref('{ERROR_REF}') }}}}", 1
                )
            assert new != text
            path.write_text(new)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--models", type=int, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--seed-error", choices=("ref", "column"))
    args = parser.parse_args()
    if args.out.exists():
        shutil.rmtree(args.out)
    args.out.mkdir(parents=True)
    sqb = write_sqlbuild(args.out, args.models)
    write_dbt_remote(write_dbt(sqb, args.out, args.models))
    write_norules(sqb)
    write_warehouse(args.out, args.models, sqb)
    if args.seed_error:
        seed_error(args.out, args.seed_error)


if __name__ == "__main__":
    main()
