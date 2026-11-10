# /// script
# requires-python = ">=3.12"
# dependencies = ["polyglot-sql-chio==0.13.10", "pyyaml"]
# ///
"""Write a DuckDB copy of core DuckShop, translated from its compiled Snowflake models.

    uv run --script benchmarks/duckshop/scripts/duckdb_project.py <destination>

Needs `sqb` on PATH. The script copies the core project (`core_project.py`), compiles it, and
writes a DuckDB project to DESTINATION:

- Each model keeps its header, with Snowflake types mapped to DuckDB's, and gets its compiled SQL
  translated by Polyglot and finished by `duckdb_rewrites.py`. Macros, constants and private
  declarations are already inlined there, so the copy drops them; it keeps the enums a column is
  typed with.
- Snowflake functions DuckDB lacks become project SQL macros, written where their callers are.
- Sources, seeds, schemas, audits and the synthetic data loaders are adapted for DuckDB.
- Tests, scenarios, custom Rules and Snowflake-only hooks are left out.

The copy is for demonstrations: build it with `sqb load` and `sqb build` on DuckDB. It is not a
benchmark workload, and the Snowflake project stays the source of truth.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
from typing import Any

import polyglot_sql
import yaml
from core_project import copy_core_project
from duckdb_rewrites import balanced, convert, shim_calls, write_shims

TYPE_MAP = [
    (r"\bNUMBER\(38, ?0\)|\b(INT|INTEGER|SMALLINT|TINYINT)\b", "BIGINT"),
    (r"\b(VARIANT|OBJECT|ARRAY)\b", "JSON"),
    (r"\bTIMESTAMP_NTZ\b", "TIMESTAMP"),
    (r"\bTIMESTAMP_(TZ|LTZ)\b", "TIMESTAMPTZ"),
    (r"\bNUMBER\b", "DECIMAL"),
]
CONNECTION = """[connections.warehouse]
database = "duckshop.duckdb"

[targets.dev]
connection = "warehouse"
schema = "dev"

[targets.prod]
connection = "warehouse"
schema = "analytics"
"""
WRITER_QUOTE = (
    '    return "\'" + text.replace("\\\\", "\\\\\\\\").replace("\'", "\'\'") + "\'"',
    '    return "\'" + text.replace("\'", "\'\'") + "\'"',
)
WRITER_INSERT = '''def insert_sql(
    destination: str, columns: tuple[tuple[str, str], ...], rows: list[dict[str, object]]
) -> str:
    """One INSERT for a batch: typed casts happen in the select list."""

    names = ", ".join(name for name, _ in columns)
    aliases = ", ".join(f"column{index}" for index in range(1, len(columns) + 1))
    select = ", ".join(
        f"CAST(column{index} AS {sql_type})" for index, (_, sql_type) in enumerate(columns, 1)
    )
    values = ",\\n".join(
        "(" + ", ".join(literal(row[name]) for name, _ in columns) + ")" for row in rows
    )
    rows_sql = f"FROM (VALUES\\n{values}) AS v({aliases})"
    return f"INSERT INTO {destination} ({names})\\nSELECT {select}\\n{rows_sql}"


def statements'''


def types(text: str) -> str:
    """Snowflake types as DuckDB's. Snowflake's integers are all NUMBER(38, 0): BIGINT here."""

    for pattern, new in TYPE_MAP:
        text = re.sub(pattern, new, text)
    return text


def translate(sql: str, model: str | None = None) -> str:
    # GET with a computed key and REGEXP_SUBSTR, which returns NULL rather than '' when nothing
    # matches, go to their shims untranslated: Polyglot's DuckDB forms behave differently.
    sql = re.sub(r"(?<![\w.])(GET|REGEXP_SUBSTR)\(", r"SF_\1(", sql)
    sql = re.sub(r'__udf\("(\w+)"\)\(', r"__sqbudf__\1(", sql)
    sql = re.sub(r'__table_fn\("(\w+)"\)\(', r"__sqbtfn__\1(", sql)
    duckdb_sql = polyglot_sql.transpile(sql, read="snowflake", write="duckdb", pretty=True)[0]
    out = convert(duckdb_sql, model)
    out = re.sub(
        r"__(REF|SOURCE|SEED|CURSOR_START|CURSOR_END)\(", lambda m: f"__{m.group(1).lower()}(", out
    )
    out = re.sub(r"(?i)__sqbudf__(\w+)\(", lambda m: f'__udf("{m.group(1).lower()}")(', out)
    out = re.sub(r"(?i)__sqbtfn__(\w+)\(", lambda m: f'__table_fn("{m.group(1).lower()}")(', out)
    out = re.sub(r"(\bAS |::)(DECIMAL\(38, ?0\)|INT|INTEGER|SMALLINT)(?![\w(])", r"\1BIGINT", out)
    return shim_calls(out)


def header(text: str, keyword: str) -> str:
    m = re.match(rf"\s*{keyword} \(.*?^\);\n", text, re.S | re.M)
    if m is None:
        raise ValueError(f"no {keyword} header")
    return m.group(0)


def audit_sql(text: str) -> str:
    """Audit SQL in headers and audit files, with Snowflake's IFF as DuckDB's IF."""

    return re.sub(r"(?<![\w.])IFF\(", "IF(", text, flags=re.I)


def model_header(text: str) -> str:
    """A model header without private constants and enums, which its compiled SQL inlines, and
    without Snowflake-only hooks."""

    head = header(text, "MODEL")
    head = re.sub(r'(?m)^ +inline_sql\("[^"]*CHANGE_TRACKING[^"]*"\),\n', "", head)
    head = re.sub(r"(?m)^  post_hooks \[\n  \],\n", "", head)
    for block in ("constants", "enums"):
        m = re.search(rf"^  {block} \(", head, re.M)
        if m:
            end = balanced(head, m.end())
            head = head[: m.start()] + head[end:].lstrip(",").lstrip("\n")
    return audit_sql(types(head))


def compile_snowflake(project: pathlib.Path) -> dict[str, Any]:
    result = subprocess.run(
        ["sqb", "compile", "--json"], cwd=project, capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        raise SystemExit("compiling the Snowflake project failed")
    return json.loads(result.stdout)


def write_models(snow: pathlib.Path, out: pathlib.Path, report: dict[str, Any]) -> None:
    for model in report["resources"]["models"]:
        path = out / model["relative_path"]
        body = translate(model["query_sql"], model["name"])
        path.write_text(model_header(path.read_text()) + "\n" + body + "\n")
    for path in (out / "functions" / "sql").glob("*.sql"):
        text = path.read_text()
        head = header(text, "FUNCTION")
        compiled = snow / "target" / "compiled" / "functions" / path.name
        body = compiled.read_text() if compiled.exists() else text[len(head) :]
        path.write_text(types(head) + "\n" + translate(body) + "\n")


def write_sources(out: pathlib.Path) -> None:
    for path in [*(out / "sources").glob("*.yml"), out / "seeds" / "seeds.yml"]:
        doc = yaml.safe_load(path.read_text())
        for entry in doc.get("sources", doc.get("seeds", [])):
            for column in entry.get("columns", []):
                column["type"] = types(column["type"])
            settings = entry.get("csv_settings") or {}
            if isinstance(settings.get("na_values"), dict):
                # DuckDB seeds take one null list for every column.
                values = {str(v) for vs in settings["na_values"].values() for v in vs}
                settings["na_values"] = sorted(values)
            if entry.get("expression"):
                entry["expression"] = translate(entry["expression"]) + "\n"
            if (entry.get("freshness") or {}).get("query"):
                # The query reads the Snowflake raw table, which the copy does not have.
                del entry["freshness"]
        path.write_text(yaml.safe_dump(doc, sort_keys=False, width=100))
    for path in out.rglob("*.py"):
        if "functions" not in path.parts:
            path.write_text(types(path.read_text()))
    writer = out / "python" / "loaders" / "_writer.py"
    text = writer.read_text()
    if WRITER_QUOTE[0] not in text:
        raise SystemExit("python/loaders/_writer.py changed: update WRITER_QUOTE")
    text = text.replace(*WRITER_QUOTE)
    text, count = re.subn(
        r"def insert_sql\(.*?\n\n\ndef statements", lambda _: WRITER_INSERT, text, flags=re.S
    )
    if count != 1:
        raise SystemExit("python/loaders/_writer.py changed: update WRITER_INSERT")
    writer.write_text(text)


def write_declarations(out: pathlib.Path) -> None:
    """Keep enums that type a column, owned by their columns' folder; drop other declarations,
    which compiled model SQL inlines. Integer enums become BIGINT columns: DuckDB types their
    members INTEGER."""

    for path in [p for p in out.rglob("*.sql") if "audits" in str(p.relative_to(out))]:
        path.write_text(audit_sql(path.read_text()))
    for path in out.glob("models/**/_sqlbuild/_schemas/*.sql"):
        path.write_text(types(path.read_text()))
    models = [p for p in out.glob("models/**/*.sql") if "_sqlbuild" not in p.parts]
    enums = {path.stem: path for path in out.glob("**/enums/*.sql")}
    for name in [n for n, p in enums.items() if re.search(r"members \(\w+ \d", p.read_text())]:
        del enums[name]
        for path in models:
            path.write_text(re.sub(rf"\(type {name}\b", "(type BIGINT", path.read_text()))
    owners: dict[str, set[pathlib.Path]] = {}
    for path in models:
        for name in re.findall(r"\(type (\w+)\b", header(path.read_text(), "MODEL")):
            if name in enums:
                owners.setdefault(name, set()).add(path.parent)
    keep: dict[pathlib.Path, str] = {}
    for name, folders in owners.items():
        folder = folders.pop() / "_sqlbuild" / "_enums" if len(folders) == 1 else out / "enums"
        keep[folder / f"{name}.sql"] = types(enums[name].read_text())
    for gone in ("tests", "macros", "rules", "constants", "enums"):
        shutil.rmtree(out / gone, ignore_errors=True)
    for folder in ("_macros", "macros", "constants", "_constants", "enums", "_enums"):
        for path in list(out.glob(f"models/**/_sqlbuild/{folder}")):
            shutil.rmtree(path)
    for path, text in keep.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    write_shims(out)


def write_project_file(out: pathlib.Path) -> None:
    path = out / "sqlbuild_project.toml"
    text = path.read_text().replace('adapter = "snowflake"', 'adapter = "duckdb"')
    start = text.index("[connections.warehouse]")
    end = text.index("\n\n", text.index("[targets.prod]"))
    text = text[:start] + CONNECTION + text[end:]
    text = re.sub(r"select = \[[^\]]*\]", 'select = ["SQBRGRAPH", "SQBRCONTRACT"]', text, count=1)
    text = re.sub(r"(?ms)^\[scenario\.local_type_overrides\.snowflake\]\n.*?(?=^\[)", "", text)
    # Rule exceptions for files the copy leaves out, such as macros, would fail compilation.
    head, *blocks = re.split(r"(?m)^(?=\[\[rules\.rule_exceptions\]\])", text)
    kept = []
    for block in blocks:
        m = re.search(r'^path = "([^"]+)"', block, re.M)
        if not m or (out / m.group(1)).exists():
            kept.append(block)
    path.write_text(head + "".join(kept))
    (out / "sqlbuild_local.toml.example").unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="Write a DuckDB copy of core DuckShop.")
    parser.add_argument("destination", type=pathlib.Path, help="new folder for the DuckDB copy")
    args = parser.parse_args()
    out: pathlib.Path = args.destination
    if out.exists():
        print(f"{out} already exists", file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory() as scratch:
        snow = pathlib.Path(scratch) / "snowflake"
        copy_core_project(snow)
        report = compile_snowflake(snow)
        shutil.copytree(snow, out, ignore=shutil.ignore_patterns("target", ".sqlbuild", "logs"))
        write_models(snow, out, report)
    write_sources(out)
    write_declarations(out)
    write_project_file(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
