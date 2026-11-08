"""Compiler Rules end-to-end fixture helpers."""

import hashlib
import json
import os
import shutil
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

from scripts.rules_benchmark._helpers.custom_rules import write_custom_rules
from sqlbuild._native import rules_bulk_cache_path
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import replace_project_text

__all__ = ("write_custom_rules",)


def write_unevaluated_rules_project(
    *,
    project_dir: Path,
    model_options: str = "",
    configuration: str = "",
    adapter: str = "duckdb",
    selected_rules: tuple[str, ...] = ("SQLCLEAN035",),
    resource_path: str = "models/staging/orders.sql",
    resource_template: str = (
        "MODEL (description 'Test model orders.', {options});\nSELECT {expression} AS order_id"
    ),
    extra_files: tuple[tuple[str, str], ...] = (),
) -> None:
    (project_dir / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "{adapter}"\n[rules]\nselect = {json.dumps(selected_rules)}\n'
        + configuration
    )
    models: Path = project_dir / "models/staging"
    models.mkdir(parents=True)
    (models / "orders.sql").write_text(
        "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n"
    )
    relative_path: str
    contents: str
    for relative_path, contents in extra_files:
        extra: Path = project_dir / relative_path
        extra.parent.mkdir(parents=True, exist_ok=True)
        extra.write_text(contents)
    resource: Path = project_dir / resource_path
    resource.parent.mkdir(parents=True, exist_ok=True)
    resource.write_text(
        resource_template.format(options=model_options, expression="ABS(" * 130 + "1" + ")" * 130)
    )


def run_unevaluated_rules_cli(project_dir: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--no-color",
            "--project-dir",
            str(project_dir),
            *args,
        ],
        capture_output=True,
        text=True,
        check=False,
    )


def write_custom_rule_project(
    *,
    project_dir: Path,
    selected_rules: tuple[str, ...],
    files: tuple[tuple[str, str], ...],
    configuration: str = "",
    adapter: str = "duckdb",
) -> None:
    """Write a project, DuckDB by default, whose selected custom Rules live in the given files."""

    (project_dir / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "{adapter}"\n\n'
        f"[rules]\nselect = {json.dumps(selected_rules)}\n"
        "\n[rules.thresholds]\nmin_custom_rule_test_cases = 0\n" + configuration,
        encoding="utf-8",
    )
    orders: Path = project_dir / "models" / "orders.sql"
    orders.parent.mkdir(parents=True, exist_ok=True)
    orders.write_text('MODEL (description "Orders");\nSELECT 1 AS order_id\n', encoding="utf-8")
    relative_path: str
    contents: str
    for relative_path, contents in files:
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")


CUSTOM_RULE_HEADER: str = (
    "import pathlib\n\nfrom sqlbuild.rules import Finding, Model, RuleContext, rule\n"
)


def custom_rule_source(*, code: str, body: str, header: str = CUSTOM_RULE_HEADER) -> str:
    """Return one model-subject custom Rule module with the given check body."""

    return (
        f"{header}\n\n"
        f'@rule(code="{code}", message="orders rule", remediation="Adjust the orders model.")\n'
        "def check(*, model: Model, ctx: RuleContext) -> list[Finding]:\n"
        f"{body}"
    )


def run_compile_cli(
    project_dir: Path,
    *,
    environment: tuple[tuple[str, str], ...] = (),
    working_directory: Path | None = None,
    preexec: Callable[[], None] | None = None,
    arguments: tuple[str, ...] = (),
) -> subprocess.CompletedProcess[str]:
    """Run the real `sqb compile --json` command from an optional invocation directory."""

    return subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--no-color",
            "--project-dir",
            str(project_dir),
            "compile",
            "--json",
            *arguments,
        ],
        capture_output=True,
        text=True,
        check=False,
        env={**os.environ, **dict(environment)},
        cwd=working_directory,
        preexec_fn=preexec,
    )


def pin_to_one_cpu() -> None:
    """Restrict the calling process, and so every custom-rule host it starts, to one CPU."""

    os.sched_setaffinity(0, {min(os.sched_getaffinity(0))})


SPLIT_ORDER_TABLES: str = (
    "import re\n\n"
    "REGIONS = {f'region_{index:02d}': index for index in range(40)}\n"
    "FLAGGED_SUFFIXES = ('3', '7')\n"
    "ORDER_NAME = re.compile(r'^orders(_[0-9]{3})?$')\n"
)


def split_model_files(model_count: int) -> tuple[tuple[str, str], ...]:
    """Return numbered order models beside the project's `models/orders.sql`."""

    return tuple(
        (f"models/orders_{index:03d}.sql", 'MODEL (description "Orders");\nSELECT 1 AS order_id\n')
        for index in range(model_count)
    )


def split_expected_findings(model_count: int) -> tuple[tuple[str, str], ...]:
    """Return what one host reports for the duplicate-parity and flagged-suffix order rules."""

    names: tuple[str, ...] = tuple(f"orders_{index:03d}" for index in range(model_count))
    # `models/orders.sql` sorts first and is the first odd model; `orders_000` is the first even.
    duplicates: tuple[tuple[str, str], ...] = tuple(
        ("XSQBRSPLIT001", f"models/{name}.sql") for name in names[1:]
    )
    suffixes: tuple[tuple[str, str], ...] = tuple(
        ("XSQBRSPLIT002", f"models/{name}.sql")
        for name in filter(lambda name: name.endswith(("3", "7")), names)
    )
    return tuple(sorted(duplicates + suffixes))


def custom_rule_diagnostics(result: subprocess.CompletedProcess[str]) -> list[dict[str, Any]]:
    """Return custom-rule diagnostics from one JSON compile result."""

    payload: dict[str, Any] = json.loads(result.stdout)
    return list(filter(lambda item: str(item["code"]).startswith("XSQBR"), payload["diagnostics"]))


def custom_rule_findings(result: subprocess.CompletedProcess[str]) -> tuple[tuple[str, str], ...]:
    """Return sorted custom-rule diagnostic codes and paths from one JSON compile result."""

    return tuple(
        sorted((str(item["code"]), str(item["path"])) for item in custom_rule_diagnostics(result))
    )


def custom_rule_codes(result: subprocess.CompletedProcess[str]) -> tuple[str, ...]:
    """Return custom-rule diagnostic codes from one JSON compile result."""

    return tuple(str(item["code"]) for item in custom_rule_diagnostics(result))


def custom_rule_paths(result: subprocess.CompletedProcess[str]) -> tuple[str, ...]:
    """Return sorted custom-rule diagnostic paths from one JSON compile result."""

    return tuple(sorted(str(item["path"]) for item in custom_rule_diagnostics(result)))


def rule_cache_hits(result: subprocess.CompletedProcess[str]) -> int:
    """Return the Rules cache hit count from one JSON compile result."""

    return int(json.loads(result.stdout)["compile_timings"]["rule_cache_hits"])


def custom_rule_timings(
    result: subprocess.CompletedProcess[str], codes: tuple[str, ...]
) -> dict[str, tuple[int, int, int, int]]:
    """Map each custom Rule code to its invocations, CPU ms, cache hits and cache misses."""

    timings: dict[str, int] = json.loads(result.stdout)["compile_timings"]
    return {
        code: (
            timings[f"custom_rules.{code}.invocations"],
            timings[f"custom_rules.{code}.cpu_ms"],
            timings[f"custom_rules.{code}.cache_hits"],
            timings[f"custom_rules.{code}.cache_misses"],
        )
        for code in codes
    }


def custom_rule_cache_counts(
    result: subprocess.CompletedProcess[str], codes: tuple[str, ...]
) -> dict[str, tuple[int, int, int]]:
    """Map each custom Rule code to its invocations, cache hits and cache misses."""

    return {
        code: (invocations, hits, misses)
        for code, (invocations, _, hits, misses) in custom_rule_timings(result, codes).items()
    }


def named_custom_rule_source(
    *, code: str, body: str, function: str, header: str = CUSTOM_RULE_HEADER
) -> str:
    """Return one model-subject custom Rule whose check function has the given name."""

    return custom_rule_source(code=code, header=header, body=body).replace(
        "def check(", f"def {function}(", 1
    )


def string_set_order(*, names: tuple[str, ...], hash_seed: str) -> str:
    """Return the comma-joined iteration order of a string set under one hash seed."""

    return subprocess.run(
        [sys.executable, "-c", f"print(','.join({{*{list(names)!r}}}))"],
        capture_output=True,
        text=True,
        check=True,
        env={"PYTHONHASHSEED": hash_seed},
    ).stdout.strip()


def custom_rule_messages(result: subprocess.CompletedProcess[str]) -> tuple[str, ...]:
    """Return custom-rule diagnostic messages from one JSON compile result."""

    return tuple(str(item["message"]) for item in custom_rule_diagnostics(result))


def write_quality_project(*, project_dir: Path, toml: str, upstream: str, summary: str) -> None:
    """Write a two-model project: `raw_orders` and the `order_summary` model under test."""

    (project_dir / "sqlbuild_project.toml").write_text(toml, encoding="utf-8")
    models: Path = project_dir / "models"
    models.mkdir()
    (models / "raw_orders.sql").write_text(upstream, encoding="utf-8")
    (models / "order_summary.sql").write_text(summary, encoding="utf-8")


def diagnostic_codes(result: subprocess.CompletedProcess[str]) -> tuple[str, ...]:
    """Return diagnostic codes from one JSON compile result, in report order."""

    return tuple(str(item["code"]) for item in json.loads(result.stdout)["diagnostics"])


def finding_fixability(result: subprocess.CompletedProcess[str]) -> dict[str, bool]:
    """Map each code in a `rules --json run` result to whether its finding reports a fix."""

    return {
        str(item["code"]): item.get("fixable") is True
        for item in json.loads(result.stdout)["findings"]
    }


def fixable_finding_count(result: subprocess.CompletedProcess[str]) -> int:
    """Count findings that report an available fix in a `rules --json run` result."""

    return sum(item.get("fixable") is True for item in json.loads(result.stdout)["findings"])


def diagnostic_notes(result: subprocess.CompletedProcess[str]) -> dict[str, tuple[str, ...]]:
    """Map each code in a JSON compile result to its diagnostic notes."""

    return {
        str(item["code"]): tuple(item.get("notes", ()))
        for item in json.loads(result.stdout)["diagnostics"]
    }


def help_fragment_presence(
    result: subprocess.CompletedProcess[str], fragments: tuple[tuple[str, str], ...]
) -> tuple[bool, ...]:
    """Return, per `(code, fragment)`, whether that code's diagnostic help contains it."""

    helps: dict[str, str] = {
        str(item["code"]): str(item.get("help", ""))
        for item in json.loads(result.stdout)["diagnostics"]
    }
    return tuple(fragment in helps.get(code, "") for code, fragment in fragments)


def long_literal_hints(result: subprocess.CompletedProcess[str]) -> dict[str, str]:
    """Map each finding path of a SQLCLEAN044-only run to the help before the generic guidance."""

    return {
        str(item["path"]): str(item["remediation"]).partition("Define the value once")[0].strip()
        for item in json.loads(result.stdout)["findings"]
    }


INCREMENTAL_RULES_PROJECT: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connection]\ndatabase = "warehouse.duckdb"\n\n'
        '[targets.dev]\nschema = "dev"\n\n[targets.prod]\nschema = "prod"\n\n'
        '[vars]\ndiscount_rate = "0.1"\n\n'
        '[rules]\nselect = ["SQBR", "SQLCLEAN", "XSQBR"]\n\n'
        "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n"
    ),
    "models/staging/stg_orders.sql": (
        "MODEL (description 'Test model stg_orders.',\n  materialized view,\n"
        "  contract enforced,\n  columns (\n"
        "    order_id (type INTEGER, nullable false, audits [not_null]),\n"
        "    amount (type DOUBLE),\n  ),\n);\n\n"
        "SELECT CAST(1 AS INTEGER) AS order_id, CAST(10.5 AS DOUBLE) AS amount\n"
    ),
    "models/marts/order_totals.sql": (
        "MODEL (description 'Test model order_totals.',\n  columns (\n"
        "    order_id (type INTEGER, nullable false, audits [not_null]),\n"
        "    net_amount (type DOUBLE),\n  ),\n);\n\n"
        "SELECT\n  o.order_id,\n"
        '  CAST(@py.discounted("o.amount") * (1 - CAST(@@discount_rate AS DOUBLE)) AS DOUBLE)'
        " AS net_amount\n"
        'FROM __ref("stg_orders") AS o\n'
    ),
    "models/marts/_sqlbuild/_macros/python/amounts.py": (
        'def discounted(amount: str) -> str:\n    return f"({amount}) * 0.9"\n'
    ),
    "tests/unit/test_order_totals.sql": (
        "TEST ();\n\nWITH\n__ref__stg_orders AS (\n  SELECT 1 AS order_id, 10.0 AS amount\n),\n"
        "__expected__order_totals AS (\n  SELECT 1 AS order_id, 9.0 AS net_amount\n)\n"
        "SELECT 1\n"
    ),
    "config/policy.yml": "strict: false\n",
    "rules/limits.py": "MAX_NAME_LENGTH: int = 30\n",
    "rules/memo.py": """from sqlbuild.rules import Finding, Model, RuleContext, rule

_SEEN_SQL: dict[str, str] = {}


@rule(code="XSQBRGOV005", message="Models must not repeat SQL", remediation="Deduplicate.")
def unique_sql(*, model: Model, ctx: RuleContext) -> list[Finding]:
    if not _SEEN_SQL:
        for item in ctx.project.models:
            _SEEN_SQL[item.name] = ctx.sql.for_model(item).expanded.source
    own: str = _SEEN_SQL[model.name]
    repeated: bool = sum(source == own for source in _SEEN_SQL.values()) > 1
    return [ctx.finding(subject=model)] if repeated else []
""",
    "rules/governance.py": """from sqlbuild.rules import Finding, Model, Project, RuleContext, rule

from rules import limits


@rule(code="XSQBRGOV001", message="Contracts must be enforced", remediation="Enforce it.")
def enforced_contracts(*, model: Model, ctx: RuleContext) -> list[Finding]:
    if "SELECT" not in ctx.sql.for_model(model).authored.source:
        return []
    return [] if ctx.contracts.enforced(model) else [ctx.finding(subject=model)]


@rule(code="XSQBRGOV002", message="Staging models need a consumer", remediation="Use it.")
def staging_consumers(*, model: Model, ctx: RuleContext) -> list[Finding]:
    if not model.name.startswith("stg_"):
        return []
    return [] if ctx.graph.dependents(model) else [ctx.finding(subject=model)]


@rule(code="XSQBRGOV003", message="Strict policy limits models", remediation="Relax it.")
def strict_policy(*, project: Project, ctx: RuleContext) -> list[Finding]:
    del project
    findings: list[Finding] = []
    if "strict: true" in ctx.project.tree.read_text("config/policy.yml"):
        findings.append(ctx.finding(subject="config/policy.yml", message="strict policy"))
    if len(ctx.project.tree.glob("config/*.yml")) > 1:
        findings.append(ctx.finding(subject="config/policy.yml", message="several policies"))
    if len(ctx.project.models) > 2:
        findings.append(ctx.finding(subject="config/policy.yml", message="too many models"))
    return findings


@rule(code="XSQBRGOV004", message="Model names are bounded", remediation="Shorten it.")
def bounded_names(*, model: Model, ctx: RuleContext) -> list[Finding]:
    names: list[str] = [test.name for test in ctx.tests.for_model(model)]
    too_long: bool = len(model.name) > limits.MAX_NAME_LENGTH
    unnamed: bool = any("discount" not in name for name in names)
    return [ctx.finding(subject=model)] if too_long or unnamed else []


@rule(code="XSQBRGOV006", message="Expanded SQL uses a flagged value", remediation="Review.")
def flagged_values(*, model: Model, ctx: RuleContext) -> list[Finding]:
    expanded: str = ctx.sql.for_model(model).expanded.source
    flagged: bool = any(value in expanded for value in ("12.5", "0.8", "0.2"))
    return [ctx.finding(subject=model)] if flagged else []
""",
}


BUILT_IN_RULES_PROJECT: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "warehouse.duckdb"\n\n'
        '[rules]\nselect = ["SQBR"]\ndomains = ["commerce"]\n\n'
        "[rules.thresholds]\nmin_tests_per_model = 1\n"
    ),
    "models/commerce/staging/commerce__stg__orders.sql": (
        'MODEL (description "Staged orders");\n\n'
        "SELECT CAST(1 AS INTEGER) AS order_id, CAST(10.5 AS DOUBLE) AS amount, "
        "'paid' AS status\n"
    ),
    "models/commerce/mart/commerce__mart__order_totals.sql": (
        'MODEL (description "Order totals");\n\n'
        'WITH orders AS (\n  SELECT * FROM __ref("commerce__stg__orders")\n)\n'
        "SELECT order_id, amount * 2 AS doubled_amount FROM orders "
        "WHERE status = 'paid' AND amount > 7\n"
    ),
    "models/commerce/mart/commerce__mart__customers.sql": (
        'MODEL (description "Customers");\n\nSELECT customer_id FROM warehouse.raw.customers\n'
    ),
    "tests/unit/test_commerce__mart__order_totals.sql": (
        "TEST ();\n\nWITH\n__ref__commerce__stg__orders AS (\n"
        "  SELECT 1 AS order_id, 10.0 AS amount, 'paid' AS status\n),\n"
        "__expected__commerce__mart__order_totals AS (\n"
        "  SELECT 1 AS order_id, 20.0 AS doubled_amount\n)\nSELECT 1\n"
    ),
}


_ORDER_SLICE_COUNT: int = 45
CUSTOM_RULES_EDIT_PROJECT: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "warehouse.duckdb"\n\n'
        '[rules]\nselect = ["XSQBR"]\n\n'
        "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n"
    ),
    "sources/raw.yml": (
        "sources:\n"
        "  - name: raw_orders\n    description: Raw orders.\n"
        '    expression: "(SELECT 1 AS id, CAST(10.5 AS DOUBLE) AS amount, 1 AS customer_id)"\n'
        "  - name: raw_customers\n    description: Raw customers.\n"
        '    expression: "(SELECT 1 AS customer_id)"\n'
        "  - name: raw_products\n    description: Raw products.\n"
        '    expression: "(SELECT 1 AS product_id)"\n'
    ),
    "models/staging/stg_orders.sql": (
        'MODEL (description "Staged orders");\n\n'
        'SELECT id AS order_id, amount, customer_id FROM __source("raw_orders")\n'
    ),
    "models/staging/stg_customers.sql": (
        'MODEL (description "Staged customers");\n\n'
        'SELECT customer_id FROM __source("raw_customers")\n'
    ),
    "models/marts/order_totals.sql": (
        'MODEL (description "Order totals");\n\n'
        'SELECT order_id, amount * 2 AS doubled_amount FROM __ref("stg_orders") WHERE amount > 7\n'
    ),
    **{
        f"models/intermediate/orders_{index:03d}.sql": (
            f'MODEL (description "Order slice {index}");\n\n'
            'SELECT order_id FROM __ref("stg_orders")\n'
        )
        for index in range(_ORDER_SLICE_COUNT)
    },
    "rules/approved_sources.yaml": "raw_orders\nraw_customers\n",
    "rules/orders.py": """from sqlbuild.rules import Finding, Model, Project, RuleContext, rule


@rule(code="XSQBRORD101", message="Filters hard-code an amount", remediation="Use a constant.")
def high_amount_filters(*, model: Model, ctx: RuleContext) -> list[Finding]:
    expanded: str = ctx.sql.for_model(model).expanded.source
    return [ctx.finding(subject=model)] if "amount > 9" in expanded else []


@rule(code="XSQBRORD102", message="Staging models need a consumer", remediation="Use it.")
def staging_consumers(*, model: Model, ctx: RuleContext) -> list[Finding]:
    if model.path.parent.name != "staging":
        return []
    return [] if ctx.graph.dependents(model) else [ctx.finding(subject=model)]


@rule(code="XSQBRORD103", message="Marts must enforce a contract", remediation="Enforce it.")
def mart_contracts(*, model: Model, ctx: RuleContext) -> list[Finding]:
    if model.path.parent.name != "marts":
        return []
    return [] if ctx.contracts.enforced(model) else [ctx.finding(subject=model)]


@rule(code="XSQBRORD201", message="Sources must be approved", remediation="Approve it.")
def approved_sources(*, project: Project, ctx: RuleContext) -> list[Finding]:
    del project
    approved: set[str] = set(ctx.project.tree.read_text("rules/approved_sources.yaml").split())
    names: list[str] = sorted(str(getattr(source, "name")) for source in ctx.project.sources)
    return [
        ctx.finding(subject="sources/raw.yml", message=f"Source {name} is not approved")
        for name in names
        if name not in approved
    ]


@rule(code="XSQBRORD202", message="Marts must stay few", remediation="Consolidate them.")
def few_marts(*, project: Project, ctx: RuleContext) -> list[Finding]:
    del project
    marts: list[Model] = [item for item in ctx.project.models if item.path.parent.name == "marts"]
    if len(marts) < 2:
        return []
    return [ctx.finding(subject=marts[0], message=f"{len(marts)} marts")]
""",
}
ONE_MODEL_EDIT_MODEL_COUNT: int = 6
# One cached result per model for the custom rule, plus the cached built-in rules request.
ONE_MODEL_EDIT_CACHED_RESULTS: int = ONE_MODEL_EDIT_MODEL_COUNT + 1
ONE_MODEL_EDIT_PROJECT: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n'
        '[rules]\nselect = ["XSQBRONE001"]\n\n'
        "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n"
    ),
    "rules/orders.py": (
        "from sqlbuild.rules import Finding, Model, RuleContext, rule\n\n\n"
        '@rule(code="XSQBRONE001", slug="literal-order", message="Order 3 is a literal", '
        'remediation="Select order 3 from a source.")\n'
        "def literal_order(*, model: Model, ctx: RuleContext) -> list[Finding]:\n"
        "    source: str = ctx.sql.for_model(model).expanded.source\n"
        '    return [ctx.finding(subject=model)] if "SELECT 3 " in source else []\n'
    ),
    **{
        f"models/orders_{index:03d}.sql": (
            f'MODEL (description "Orders {index}");\n\nSELECT {index} AS order_id\n'
        )
        for index in range(ONE_MODEL_EDIT_MODEL_COUNT)
    },
}


def custom_rules_edit_chain(
    *, root: Path, edits: tuple[Any, ...], environment: tuple[tuple[str, str], ...] = ()
) -> tuple[
    tuple[int, str, str], tuple[tuple[int, str, str], ...], tuple[tuple[int, str, str], ...]
]:
    """Compile the custom-rule project cold, then after each edit beside a cache-free oracle."""

    project_dir: Path = root / "orders"
    write_project_files(project_dir=project_dir, files=CUSTOM_RULES_EDIT_PROJECT)
    cold: tuple[int, str, str] = rules_compile_outcome(project_dir, environment=environment)
    edited: list[tuple[int, str, str]] = []
    oracles: list[tuple[int, str, str]] = []
    for step, edit_case in enumerate(edits):
        edit_case.edit(project_dir)
        edited.append(rules_compile_outcome(project_dir, environment=environment))
        oracle_dir: Path = root / f"oracle_{step}"
        shutil.copytree(project_dir, oracle_dir, ignore=shutil.ignore_patterns("target"))
        oracles.append(rules_compile_outcome(oracle_dir, "--no-cache"))
    return cold, tuple(edited), tuple(oracles)


def write_project_files(*, project_dir: Path, files: dict[str, str]) -> None:
    """Write authored project files beneath one project directory."""

    for relative_path, contents in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")


def rules_compile_outcome(
    project_dir: Path, *arguments: str, environment: tuple[tuple[str, str], ...] = ()
) -> tuple[int, str, str]:
    """Compile in a fresh process and return its exit code, diagnostics, and artifacts digest."""

    return rules_compile_run(project_dir, *arguments, environment=environment)[0]


def rules_compile_run(
    project_dir: Path, *arguments: str, environment: tuple[tuple[str, str], ...] = ()
) -> tuple[tuple[int, str, str], tuple[int, int]]:
    """Compile in a fresh process; return its comparable outcome and rule cache hits and misses."""

    result: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--no-color",
            "--project-dir",
            str(project_dir),
            "compile",
            "--json",
            *arguments,
        ],
        capture_output=True,
        text=True,
        check=False,
        env={**os.environ, **dict(environment)},
    )
    payload: dict[str, Any] = json.loads(result.stdout or "{}")
    artifacts: Any = hashlib.sha256()
    compiled: Path = project_dir / "target" / "compiled"
    for path in sorted(filter(Path.is_file, compiled.rglob("*"))):
        artifacts.update(path.relative_to(compiled).as_posix().encode())
        artifacts.update(path.read_bytes())
    timings: dict[str, int] = payload.get("compile_timings", {})
    return (
        (
            result.returncode,
            json.dumps(payload.get("diagnostics"), sort_keys=True),
            artifacts.hexdigest(),
        ),
        (timings.get("rule_cache_hits", 0), timings.get("rule_cache_misses", 0)),
    )


def rule_cache_counts(project_dir: Path) -> tuple[int, int]:
    """Return rule cache hits and misses of one fresh-process compile."""

    result: subprocess.CompletedProcess[str] = run_compile_cli(project_dir)
    timings: dict[str, int] = json.loads(result.stdout)["compile_timings"]
    return timings["rule_cache_hits"], timings["rule_cache_misses"]


def move_order_totals_description_to_path_defaults(root: Path) -> None:
    """Drop the mart model's header description and supply it from `[path_defaults]`."""

    replace_project_text(
        root,
        "models/marts/order_totals.sql",
        "MODEL (description 'Test model order_totals.',\n",
        "MODEL (\n",
    )
    config: Path = root / "sqlbuild_project.toml"
    config.write_text(
        config.read_text(encoding="utf-8")
        + '\n[path_defaults.marts]\ndescription = "Order totals per order"\n',
        encoding="utf-8",
    )


def write_bulk_rules_cache(*, root: Path, file_name: str, text: str) -> None:
    """Overwrite one bulk Rules cache file."""

    _ = rules_bulk_cache_path(project_dir=root, file_name=file_name).write_text(
        text, encoding="utf-8"
    )


def compile_diagnostics(result: subprocess.CompletedProcess[str]) -> list[dict[str, Any]]:
    """Return every diagnostic of one `sqb compile --json` result."""

    return list(json.loads(result.stdout)["diagnostics"])


def diagnostic_messages(result: subprocess.CompletedProcess[str], code: str) -> tuple[str, ...]:
    """Return the messages of one code's diagnostics, in report order."""

    return tuple(
        str(item["message"])
        for item in filter(lambda item: item["code"] == code, compile_diagnostics(result))
    )


def diagnostic_text(result: subprocess.CompletedProcess[str]) -> str:
    """Return every diagnostic message and help, followed by stderr."""

    lines: list[str] = [
        f"{item['message']}\n{item.get('help', '')}" for item in compile_diagnostics(result)
    ]
    return "\n".join((*lines, result.stderr))


def write_one_row_cross_join_project(
    *, project_dir: Path, toml: str, seed_rows: tuple[str, ...], files: tuple[tuple[str, str], ...]
) -> Path:
    """Write a project with the `order_policy` seed and model files; return the seed CSV."""

    (project_dir / "sqlbuild_project.toml").write_text(toml, encoding="utf-8")
    for relative, text in files:
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    seed: Path = project_dir / "seeds" / "order_policy.csv"
    seed.write_text("max_amount\n" + "".join(f"{row}\n" for row in seed_rows), encoding="utf-8")
    return seed


def write_model_files(*, project_dir: Path, toml: str, files: tuple[tuple[str, str], ...]) -> None:
    """Write `sqlbuild_project.toml` and each `(relative path, text)` file."""

    (project_dir / "sqlbuild_project.toml").write_text(toml, encoding="utf-8")
    for relative, text in files:
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")


def duckdb_rules_project_toml(*, project_dir: Path, rules_toml: str) -> str:
    """Return a plannable DuckDB project's `sqlbuild_project.toml` with the given `[rules]` body."""

    return (
        'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        f'[connection]\ndatabase = "{project_dir / "orders.duckdb"}"\n\n'
        '[targets.dev]\nschema = "dev"\n\n'
        f"[rules]\n{rules_toml}"
    )
