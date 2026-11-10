"""Macro-heavy projects, render runs and bridged-expansion outcomes."""

from __future__ import annotations

import random
import re
from collections.abc import Callable
from contextvars import Token
from dataclasses import dataclass
from functools import partial
from itertools import compress
from pathlib import Path
from typing import TYPE_CHECKING, cast

import pytest

from sqlbuild._native import (
    CompileAdapterContext,
    CompilerStage,
    DiscoveredProjectInputs,
    LoadedMacro,
    MacroExpansionResult,
    ResourceIdentity,
    ResourceKind,
    build_compile_inputs,
    compile_frontier,
    discover_project_inputs,
    expand_sql_macros,
    mask_capture_noise,
    render_stage_capture,
    resolve_effective_collection_rendering,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.exceptions import CompileInputError
from sqlbuild.compiler.compile.models import MacroContext
from sqlbuild.compiler.macro_bridge.classes.macro_bridge import MacroBridge
from sqlbuild.compiler.macro_bridge.constants import ACTIVE_MACRO_BRIDGE

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView

MACRO_CALL_LOG_ENV_VAR: str = "SQLBUILD_TEST_MACRO_CALL_LOG"
_RUN_ID_PATTERN: re.Pattern[str] = re.compile(r'"run_id": "[^"]*"')
_DIFFERENTIAL_FRAGMENTS: tuple[str, ...] = (
    "@py.m('a')", "@py.m(1)", "@py.m()", "@py.mm(2)", "@py.mm (2)", "@py.m (1)",
    "@py.n(@py.m('b'))", "@py.n(@py.m(@py.n()))",
    "@py.n(@py.mm(3), '@py.m(4)')", "@py.tag(__ref('orders'))", "@py.tag(__seed('rates'))",
    "@py.tag(__ref('customers'))", "@py.cols(__ref('orders'))", "@py.gen()", "@py.nope(1)",
    "@py.m(x)", "@py.ab c(1)", "@m(1)", "@sql.m(1)", "@py.m('é')", "'@py.m(1)'",
    "-- @py.m(1)\n", "/* @py.n() */", "$$@py.m()$$", "`@py.m()`", "@@v",
    "@", " ", "\n", "\u3000", "é", "'", "(", ")", "SELECT ", ", ", "\"q\"", "@py.sur()",
)  # fmt: skip
_DIFFERENTIAL_FILE: Path = Path("models/orders_summary.sql")

MACRO_BRIDGE_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "bridge_demo"\n'
        'adapter = "duckdb"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        'database = "bridge.duckdb"\n\n'
        "[vars]\n"
        'region = "north"\n\n'
        "[targets.dev]\n"
        'schema = "analytics"\n\n'
        "[settings]\n"
        "sql_analysis = false\n\n"
        "[scopes]\n"
        "enforce_placement = false\n"
    ),
    "macros/python/common.py": (
        '"""Shared macros for the bridge demo."""\n\n'
        "import os\n\n\n"
        "def cents(column: str) -> str:\n"
        '    """Convert an amount to cents."""\n'
        '    return f"{column} * 100"\n\n\n'
        "def wrap(expression: str) -> str:\n"
        '    """Parenthesise an expression."""\n'
        '    return f"({expression})"\n\n\n'
        "def region_filter(ctx) -> str:\n"
        '    """Compare the region column with the configured region."""\n'
        "    return f\"region = '{ctx.vars['region']}'\"\n\n\n"
        "def scale(ctx, column: str) -> str:\n"
        '    """Scale a column by every visible rate."""\n'
        '    return f"{column} * {sum(ctx.constants.values())}"\n\n\n'
        "def visible_constants(ctx) -> str:\n"
        '    """List the visible constants."""\n'
        "    return repr(', '.join(ctx.constants))\n\n\n"
        "def status_code(ctx) -> str:\n"
        '    """Render the placed status."""\n'
        "    return ctx.render_enum_member(enum_name='order_status', member_name='PLACED')\n\n\n"
        "def optional_label(ctx) -> str:\n"
        '    """Read an optional constant."""\n'
        "    return repr(ctx.constants.get('missing_label', 'none'))\n\n\n"
        "def columns_of(relation) -> str:\n"
        '    """Select every column of a relation."""\n'
        '    return f"SELECT * FROM {relation}"\n\n\n'
        "def ref_tag(relation) -> str:\n"
        '    """Expose the in-expansion relation placeholder text."""\n'
        "    return repr(str(relation).upper())\n\n\n"
        "def generated_join() -> str:\n"
        '    """Emit a reference instead of receiving it."""\n'
        "    return '(SELECT id FROM __ref(\"customers\"))'\n\n\n"
        "def logged(column: str) -> str:\n"
        '    """Record each execution in the test log."""\n'
        f'    path = os.environ.get("{MACRO_CALL_LOG_ENV_VAR}")\n'
        "    if path:\n"
        '        with open(path, "a", encoding="utf-8") as log:\n'
        '            log.write(column + "\\n")\n'
        "    return column\n"
    ),
    "enums/order_status.sql": (
        'ENUM (\n  name order_status,\n  members (PLACED "placed", SHIPPED "shipped"),\n);\n'
    ),
    "models/north/_sqlbuild/_macros/python/fmt.py": (
        'def fmt(value: str) -> str:\n    """Label a value."""\n    return f"\'north:{value}\'"\n'
    ),
    "constants/base_rate.sql": "CONSTANT (name base_rate, value 1);\n",
    "models/north/_sqlbuild/_constants/north_rate.sql": "CONSTANT (name north_rate, value 10);\n",
    "models/south/_sqlbuild/_constants/south_rate.sql": "CONSTANT (name south_rate, value 20);\n",
    "models/shared/customers.sql": (
        "MODEL (description \"Customers\");\n\nSELECT 1 AS id, 2 AS amount, 'north' AS region\n"
    ),
    "models/shared/orders_source.sql": (
        'MODEL (description "Orders source");\n\nSELECT 1 AS id, 3 AS amount\n'
    ),
    "hooks/sql/stamp.sql": (
        "HOOK (description \"Stamp a value\");\n\nSELECT @py.cents('2') AS stamped\n"
    ),
    "functions/sql/udf__cents_of.sql": (
        "FUNCTION (\n"
        '  description "Convert a value to cents",\n'
        "  arguments (v INTEGER),\n"
        "  returns INTEGER,\n"
        ");\n\n"
        '@py.cents("v")\n'
    ),
    "audits/generic/positive_cents.sql": (
        'AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE @py.cents("amount") < 0\n'
    ),
    "sources/raw.yml": (
        "sources:\n"
        "  - name: raw__payments\n"
        "    description: Payments\n"
        "    expression: |\n"
        "      SELECT @py.cents('5') AS amount_cents, @py.wrap(@py.cents('6')) AS wrapped\n"
    ),
    "tests/unit/test_cents_macro.sql": (
        'TEST (mode macro, name "converts_to_cents");\n\n'
        "WITH\n"
        "__macro_actual__ AS (\n"
        '  SELECT @py.cents("3") AS cents\n'
        "),\n"
        "__macro_expected__ AS (\n"
        "  SELECT 300 AS cents\n"
        ")\n"
        "SELECT 1\n"
    ),
}

_REGION_MODEL_BODY: str = (
    "SELECT\n"
    '  @py.cents("amount") AS amount_cents,\n'
    '  @py.wrap(@py.cents("amount")) AS wrapped,\n'
    '  @py.scale("amount") AS scaled,\n'
    "  @py.visible_constants() AS visible,\n"
    "  {label} AS label,\n"
    "  @py.region_filter() AS region_check,\n"
    "  @py.status_code() AS status_value,\n"
    "  @py.optional_label() AS optional_label,\n"
    "  {ref_tags},\n"
    '  @py.logged("amount") AS logged_amount,\n'
    "  'quoted @py.cents(1)' AS quoted -- @py.cents(2)\n"
    'FROM (@py.columns_of(__ref("customers"))) AS c\n'
    "WHERE id IN @py.generated_join()\n"
)
_NORTH_REF_TAGS: str = '@py.ref_tag(__ref("orders_source")) AS first_tag'
_SOUTH_REF_TAGS: str = (
    '@py.ref_tag(__ref("customers")) AS first_tag, @py.ref_tag(__ref("orders_source")) AS '
    "second_tag"
)


def _region_model(*, description: str, hooks: str, ref_tags: str, label: str) -> str:
    return (
        f'MODEL (\n  description "{description}",\n{hooks}'
        "  audits [positive_cents ()],\n);\n\n"
        + _REGION_MODEL_BODY.format(ref_tags=ref_tags, label=label)
    )


_NORTH_HOOKS: str = (
    "  pre_hooks [\n"
    "    inline_sql(\"SELECT @py.cents('1') AS hook_value\"),\n"
    '    sql("stamp"),\n'
    "  ],\n"
)
MACRO_BRIDGE_PROJECT_FILES.update(
    {
        "models/north/orders_north.sql": _region_model(
            description="North orders",
            hooks=_NORTH_HOOKS,
            ref_tags=_NORTH_REF_TAGS,
            label='@py.fmt("x")',
        ),
        "models/north/orders_north_copy.sql": _region_model(
            description="North orders copy",
            hooks=_NORTH_HOOKS,
            ref_tags=_NORTH_REF_TAGS,
            label='@py.fmt("x")',
        ),
        "models/south/orders_south.sql": _region_model(
            description="South orders", hooks="", ref_tags=_SOUTH_REF_TAGS, label="'south'"
        ),
    }
)


def write_project(*, root: Path, files: dict[str, str]) -> Path:
    """Write project files below `root` and return it."""

    for relative_path, content in files.items():
        path: Path = root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(content, encoding="utf-8")
    return root


def render_compile_inputs(*, project_dir: Path) -> CompileInputsView:
    """Render the project's compile inputs through the frontier."""

    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    adapter: DuckDbAdapter = DuckDbAdapter()
    return compile_frontier(
        until=CompilerStage.COMPILE_PROJECT_INPUTS,
        python_stage=partial(
            build_compile_inputs,
            discovered_inputs=discovered,
            adapter_context=CompileAdapterContext(
                value_renderer=adapter,
                collection_rendering=resolve_effective_collection_rendering(
                    project_config=discovered.project_config, declaration_override=None
                ),
                python_functions_inherit_default_namespace=(
                    adapter.python_functions_inherit_default_namespace()
                ),
                sql_lexical_syntax=adapter.sql_lexical_syntax,
                sql_analysis_dialect=adapter.sql_analysis_dialect_name,
            ),
            resolved_connection={},
            defer_model_sql_validation=True,
            no_cache=True,
        ),
    )


def comparable_capture(inputs: CompileInputsView) -> str:
    """Render compile inputs as canonical JSON without the per-run identifier."""

    return mask_capture_noise(
        _RUN_ID_PATTERN.sub('"run_id": "<run>"', render_stage_capture(inputs))
    )


def render_error(*, project_dir: Path) -> tuple[type[BaseException], str, type[object]]:
    """Render the project and describe the error it raises."""

    with pytest.raises(CompileInputError) as raised:
        _ = render_compile_inputs(project_dir=project_dir)
    return type(raised.value), str(raised.value), type(raised.value.__cause__)


def _wrap_m(value: object = "") -> str:
    return f"m<{value}>"


def _wrap_n(value: object = "", label: object = "") -> str:
    return f"n[{value}{label}]"


def _tag(relation: object) -> str:
    return repr(str(relation).upper())


def _cols(relation: object) -> str:
    return f"SELECT * FROM {relation}"


def _surrogate() -> str:
    return "x\udcff"


def _generated() -> str:
    return '(SELECT id FROM __ref("orders"))'


def _loaded_macro(*, name: str, function: Callable[..., object]) -> LoadedMacro:
    path: Path = Path(f"macros/python/{name}.py")
    return LoadedMacro(
        name=name, file_path=path, relative_path=path, raw_source="", function=function
    )


DIFFERENTIAL_MACROS: dict[str, LoadedMacro] = {
    name: _loaded_macro(name=name, function=function)
    for name, function in (
        ("m", _wrap_m),
        ("mm", _wrap_m),
        ("n", _wrap_n),
        ("tag", _tag),
        ("cols", _cols),
        ("gen", _generated),
        ("sur", _surrogate),
    )
}
_DIFFERENTIAL_CONTEXT: MacroContext = MacroContext(
    adapter_name="duckdb", sql_analysis_enabled=False, target_name=None
)


class ExpansionFailureCapture:
    """Swallow and keep the error a block raises."""

    def __init__(self) -> None:
        self.failure: BaseException | None = None

    def __enter__(self) -> ExpansionFailureCapture:
        return self

    def __exit__(self, error_type: object, error: BaseException | None, traceback: object) -> bool:
        self.failure = error
        return isinstance(error, Exception)


def random_macro_sql(*, rng: random.Random) -> str:
    """Return SQL text mixing valid and malformed macro calls, quotes, comments and Unicode."""

    return "".join(rng.choice(_DIFFERENTIAL_FRAGMENTS) for _ in range(rng.randint(0, 12)))


def expansion_outcome(*, sql: str, bridge: MacroBridge | None) -> tuple[object, ...]:
    """Expand `sql` with or without the bridge; return the result facts or the error."""

    token: Token[object | None] = ACTIVE_MACRO_BRIDGE.set(bridge)
    capture: ExpansionFailureCapture = ExpansionFailureCapture()
    rendered: list[tuple[object, ...]] = [()]
    with capture:
        result: MacroExpansionResult = expand_sql_macros(
            sql=sql,
            file_path=_DIFFERENTIAL_FILE,
            loaded_macros=DIFFERENTIAL_MACROS,
            macro_context=_DIFFERENTIAL_CONTEXT,
            consumer=ResourceIdentity(ResourceKind.MODEL, "orders_summary"),
        )
        rendered[0] = (
            result.sql,
            result.spans,
            result.dependencies,
            result.usages,
            result.argument_references,
        )
    ACTIVE_MACRO_BRIDGE.reset(token)
    return {
        True: rendered[0],
        False: (type(capture.failure).__name__, str(capture.failure)),
    }[capture.failure is None]


_FAILING_MACRO_LOG: str = (
    "def _log(value: object) -> None:\n"
    f'    path = os.environ.get("{MACRO_CALL_LOG_ENV_VAR}")\n'
    "    if path:\n"
    '        with open(path, "a", encoding="utf-8") as log:\n'
    '            log.write(f"{value}\\n")\n\n\n'
)
_FAILING_MACROS: str = (
    '"""Macros that render, raise after a side effect or return values that are not SQL."""\n\n'
    "import os\n\n\n" + _FAILING_MACRO_LOG + "def cents(column: str) -> str:\n"
    '    """Convert an amount to cents."""\n'
    "    _log(column)\n"
    '    return f"{column} * 100"\n\n\n'
    "def wrap(expression: str) -> str:\n"
    '    """Parenthesise an expression."""\n'
    '    _log("wrap " + expression)\n'
    '    return f"({expression})"\n\n\n'
    "def explode(column: str) -> str:\n"
    '    """Fail after logging the call."""\n'
    "    _log(column)\n"
    '    raise ValueError(f"cannot render {column}")\n\n\n'
    "def number(column: str) -> int:\n"
    '    """Return a value that is not SQL."""\n'
    "    _log(column)\n"
    "    return 1\n\n\n"
    "def _hidden(column: str) -> str:\n"
    "    return column\n"
)
_LOCAL_MACROS: str = (
    '"""Macros only the marts models can see."""\n\n'
    "import os\n\n\n" + _FAILING_MACRO_LOG + "def mart_only(column: str) -> str:\n"
    '    """Qualify a column for marts."""\n'
    "    _log(column)\n"
    '    return f"marts_{column}"\n'
)
_CYCLIC_MACROS: str = (
    '"""Macros that call each other forever."""\n\n\n'
    "def ping(column: str) -> str:\n"
    '    """Call pong."""\n'
    "    return pong(column)\n\n\n"
    "def pong(column: str) -> str:\n"
    '    """Call ping."""\n'
    "    return ping(column)\n"
)
_FAILING_CALLS: tuple[str, ...] = (
    *("@py.cents('{arg}')",) * 24, *("@py.wrap(@py.cents('{arg}'))",) * 6, "@py.mart_only('{arg}')",
    "@py.mart_only('{arg}')", "@py.explode('{arg}')",
    "@py.wrap(@py.explode('{arg}'))", "@py.number('{arg}')", "@py.cents('{arg}', 'extra')",
    "@py.cents('{arg}',,)", "@py.cents({arg})", "@py.nope('{arg}')", "@py._hidden('{arg}')",
    "@py.cents('{arg}'", "@cents x('{arg}')",
)  # fmt: skip
_MODEL_FOLDERS: tuple[str, ...] = ("staging", "marts")
_ERROR_SPECIFICS: re.Pattern[str] = re.compile(r"'[^']*'|c\d+_\d+")
_CYCLE_CHANCE: tuple[bool, ...] = (False,) * 11 + (True,)


def generated_failing_macro_project(*, rng: random.Random) -> dict[str, str]:
    """Return a project whose models call macros that may raise, mistype or be unreachable."""

    models: dict[str, str] = dict(
        _generated_model(rng=rng, model=model) for model in range(rng.randint(2, 6))
    )
    cyclic: dict[str, str] = {True: {"macros/python/cyclic.py": _CYCLIC_MACROS}, False: {}}[
        rng.choice(_CYCLE_CHANCE)
    ]
    return {
        "sqlbuild_project.toml": MACRO_BRIDGE_PROJECT_FILES["sqlbuild_project.toml"],
        "macros/python/common.py": _FAILING_MACROS,
        "models/marts/_sqlbuild/macros/python/local.py": _LOCAL_MACROS,
        **cyclic,
        **models,
    }


def _generated_model(*, rng: random.Random, model: int) -> tuple[str, str]:
    calls: list[str] = [
        f"{rng.choice(_FAILING_CALLS).format(arg=f'c{model}_{call}')} AS c{call}"
        for call in range(rng.randint(1, 3))
    ]
    return (
        f"models/{rng.choice(_MODEL_FOLDERS)}/m{model}.sql",
        f'MODEL (description "Generated {model}");\n\nSELECT {", ".join(calls)}\n',
    )


@dataclass(frozen=True)
class FailingMacroParity:
    """How rendering generated failing-macro projects failed."""

    failures: int
    successes: int
    failures_after_macro_runs: int
    distinct_errors: list[str]


def failing_macro_parity(
    *,
    projects: list[dict[str, str]],
    root: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> FailingMacroParity:
    """Render every project; summarize its errors and macro runs."""

    project_dirs: list[Path] = [
        write_project(root=root / f"project_{index}", files=files)
        for index, files in enumerate(projects)
    ]
    python_outcomes: list[object] = [
        render_outcome(
            project_dir=project_dir, log_path=root / "macros.log", monkeypatch=monkeypatch
        )
        for project_dir in project_dirs
    ]
    failed: list[bool] = [_is_failure(outcome) for outcome in python_outcomes]
    failures: list[tuple[tuple[object, ...], tuple[str, ...]]] = cast(
        list[tuple[tuple[object, ...], tuple[str, ...]]], list(compress(python_outcomes, failed))
    )
    return FailingMacroParity(
        failures=len(failures),
        successes=len(python_outcomes) - len(failures),
        failures_after_macro_runs=sum(bool(runs) for _, runs in failures),
        distinct_errors=sorted(
            {_ERROR_SPECIFICS.sub("''", str(error[1])) for error, _ in failures}
        ),
    )


def _is_failure(outcome: object) -> bool:
    return isinstance(cast(tuple[object, ...], outcome)[0], tuple)


def render_outcome(
    *, project_dir: Path, log_path: Path, monkeypatch: pytest.MonkeyPatch
) -> tuple[object, tuple[str, ...]]:
    """Render; return the error's full shape or the capture, and macro runs."""

    _ = log_path.write_text("", encoding="utf-8")
    monkeypatch.setenv(MACRO_CALL_LOG_ENV_VAR, str(log_path))
    capture: ExpansionFailureCapture = ExpansionFailureCapture()
    rendered: list[object] = [None]
    with capture:
        rendered[0] = comparable_capture(render_compile_inputs(project_dir=project_dir))
    failure: BaseException | None = capture.failure
    outcome: object = {
        True: rendered[0],
        False: (
            type(failure).__name__,
            str(failure),
            getattr(failure, "code", None),
            getattr(failure, "help", None),
            type(getattr(failure, "__cause__", None)).__name__,
            str(getattr(failure, "__cause__", None)),
        ),
    }[failure is None]
    return outcome, tuple(log_path.read_text(encoding="utf-8").splitlines())
