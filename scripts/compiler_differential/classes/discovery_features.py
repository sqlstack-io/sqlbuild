"""Write the optional project inputs that make generated seeds cover every discovery input kind."""

from __future__ import annotations

import random
from collections.abc import Callable
from functools import partial

from scripts.compiler_differential.constants import (
    GENERATOR_BOM,
    GENERATOR_CRLF,
    GENERATOR_DBT_MANIFEST,
    GENERATOR_DBT_MODEL,
    GENERATOR_DBT_REF_ERROR_CODE,
    GENERATOR_FEATURE_BLOCKS,
    GENERATOR_FEATURE_FOLDER,
    GENERATOR_FEATURE_STRIDE,
    GENERATOR_MISSING_ENV_VAR,
    GENERATOR_OPTIONAL_FEATURE_SHARE,
    GENERATOR_PROJECT_ADAPTER,
    GENERATOR_RARE_FEATURE_BLOCKS,
    GENERATOR_RARE_FEATURE_PERIOD,
    PLAN_LABEL,
)
from scripts.compiler_differential.models import ModelPlan

_UNIQUE_KEY: str = "  unique_key [id],\n"
_CURSOR_HEADER: str = "  cursor created_at,\n  cursor_type timestamp,\n  cursor_grain day,\n"


class DiscoveryFeatureWriter:
    """Add self-contained feature blocks to one generated project, chosen per seed."""

    def __init__(
        self,
        *,
        blocks: tuple[str, ...],
        rng: random.Random,
        files: dict[str, str],
        features: set[str],
        staging: list[ModelPlan],
        label: Callable[[], str],
    ) -> None:
        self._blocks: tuple[str, ...] = blocks
        self._random: random.Random = rng
        self._files: dict[str, str] = files
        self._features: set[str] = features
        self._staging: list[ModelPlan] = staging
        self._label: Callable[[], str] = label
        self.config_lines: list[str] = []
        self.adapter: str | None = None
        self.expected_error_code: str | None = None
        self.succeeding_commands: tuple[str, ...] = ()

    def write(self) -> None:
        """Write the selected blocks in their canonical order."""

        writers: dict[str, Callable[[], None]] = {
            "incremental_append": partial(self._incremental, strategy="append", unique_key=""),
            "incremental_delete_insert": partial(
                self._incremental, strategy="delete_insert", unique_key=_UNIQUE_KEY
            ),
            "incremental_merge": partial(
                self._incremental, strategy="merge", unique_key=_UNIQUE_KEY
            ),
            "microbatch_watermark": partial(
                self._microbatch,
                strategy="watermark",
                cursor_input="(column created_at, roles [filter, watermark])",
                options="  cursor_watermark_mode all,\n",
            ),
            "microbatch_rolling_window": partial(
                self._microbatch,
                strategy="rolling_window",
                cursor_input="created_at",
                options="  lookback 2d,\n",
            ),
            "snapshot_timestamp": partial(
                self._snapshot,
                strategy="timestamp",
                options="  updated_at created_at,\n",
                columns="status, created_at",
            ),
            "snapshot_check": partial(
                self._snapshot,
                strategy="check",
                options="  check_columns [status, amount],\n",
                columns="status, amount",
            ),
            "python_hook": self._python_hook,
            "managed_source_loader": self._managed_source_loader,
            "asset_and_check": self._asset_and_check,
            "discovered_set": self._discovered_set,
            "audit_factory": self._audit_factory,
            "provider": self._provider,
            "lifecycle_sink": self._lifecycle_sink,
            "command_output_event_sink": self._command_output_sink,
            "custom_materialization": self._custom_materialization,
            "project_adapter": self._project_adapter,
            "model_schema": self._model_schema,
            "list_constant": self._list_constant,
            "cross_file_macro_import": self._cross_file_macro_import,
            "macro_test_mode": self._macro_test_mode,
            "parameterized_test": self._parameterized_test,
            "generic_audit": self._generic_audit,
            "functions": self._functions,
            "local_config": self._local_config,
            "target_override": self._target_override,
            "dbt_ref": self._dbt_ref,
            "line_endings": self._line_endings,
        }
        for block in self._blocks:
            writer: Callable[[], None] | None = writers.get(block)
            if writer is not None:
                writer()

    def _base(self) -> ModelPlan:
        return self._random.choice(self._staging)

    def _model(self, *, name: str, header: str, body: str, description: str) -> str:
        path: str = f"{GENERATOR_FEATURE_FOLDER}/{name}.sql"
        self._files[path] = f'MODEL (\n  description "{description}",\n{header});\n\n{body}'
        return path

    def _incremental(self, *, strategy: str, unique_key: str) -> None:
        self._features.add(f"incremental_{strategy}")
        base: ModelPlan = self._base()
        _ = self._model(
            name=f"{strategy}_{base.name}",
            description=f"Incremental {strategy} copy of {base.name}",
            header=(
                "  materialized incremental,\n"
                f"  incremental_strategy {strategy},\n"
                f"{unique_key}{_CURSOR_HEADER}"
                f"  cursor_inputs (\n    {base.name} created_at,\n  ),\n"
            ),
            body=f'SELECT id, amount, status, created_at\nFROM __ref("{base.name}")\n',
        )

    def _microbatch(self, *, strategy: str, cursor_input: str, options: str) -> None:
        self._features.add(f"microbatch_{strategy}")
        base: ModelPlan = self._base()
        _ = self._model(
            name=f"batched_{strategy}_{base.name}",
            description=f"Microbatched {strategy} totals of {base.name}",
            header=(
                "  materialized incremental,\n"
                "  incremental_strategy delete_insert,\n"
                f"{_CURSOR_HEADER}"
                "  incremental_mode microbatch,\n"
                f"  microbatch_strategy {strategy},\n"
                f"{options}"
                "  batch_size 1d,\n"
                f"  cursor_inputs (\n    {base.name} {cursor_input},\n  ),\n"
            ),
            body=(
                "SELECT CAST(created_at AS DATE) AS created_at, SUM(amount) AS amount\n"
                f'FROM __ref("{base.name}")\nGROUP BY CAST(created_at AS DATE)\n'
            ),
        )

    def _snapshot(self, *, strategy: str, options: str, columns: str) -> None:
        self._features.add(f"snapshot_{strategy}")
        base: ModelPlan = self._base()
        _ = self._model(
            name=f"{strategy}_history_{base.name}",
            description=f"{strategy.capitalize()} history of {base.name}",
            header=(
                "  materialized snapshot,\n  unique_key [id],\n"
                f"  snapshot_strategy {strategy},\n{options}"
            ),
            body=f'SELECT id, {columns}\nFROM __ref("{base.name}")\n',
        )

    def _python_hook(self) -> None:
        self._features.add("python_hook")
        base: ModelPlan = self._base()
        self._files[f"{GENERATOR_FEATURE_FOLDER}/_sqlbuild/_hooks/python/notify.py"] = (
            "from sqlbuild.hooks import hook\n\n\n"
            "@hook\n"
            'def note_refresh(ctx, channel="#builds"):\n'
            '    """Log one refresh."""\n'
            '    ctx.log.info(f"{channel}: {ctx.model_name} refreshed during {ctx.phase}")\n'
        )
        _ = self._model(
            name=f"hooked_{base.name}",
            description=f"Hooked copy of {base.name}",
            header=(
                "  materialized table,\n"
                f'  post_hooks [python("note_refresh", channel: "{self._label()}")],\n'
            ),
            body=f'SELECT id, amount\nFROM __ref("{base.name}")\n',
        )

    def _managed_source_loader(self) -> None:
        self._features.add("managed_source_loader")
        self._files["sql/sources/managed.yml"] = (
            "sources:\n"
            "  - name: loaded_products\n"
            "    description: Products loaded by a project loader.\n"
            "    managed: true\n"
            "    write_strategy: table\n"
            "    columns:\n"
            "      - name: id\n        type: INTEGER\n"
            "      - name: label\n        type: VARCHAR\n"
        )
        self._files["python/nodes/loaders/products.py"] = (
            "from sqlbuild.loaders import loader\n\n\n"
            "@loader\n"
            "def loaded_products(ctx):\n"
            '    """Load the product list."""\n'
            f'    return [{{"id": 1, "label": "{self._label()}"}}, {{"id": 2, "label": "plain"}}]\n'
        )
        _ = self._model(
            name="product_labels",
            description="Product labels from the loader",
            header="  materialized view,\n",
            body='SELECT id, label\nFROM __source("loaded_products")\n',
        )

    def _asset_and_check(self) -> None:
        self._features.add("asset_and_check")
        base: ModelPlan = self._base()
        self._files["python/nodes/assets/exports.py"] = (
            "from sqlbuild.assets import asset\n"
            "from sqlbuild.checks import check\n"
            "from sqlbuild.refs import model\n\n\n"
            f'@asset(depends_on=model("{base.name}"), tags=("export",))\n'
            f"def export_{base.name}(ctx):\n"
            '    """Export one staged model."""\n'
            '    return ctx.result(metadata={"rows": 3}, materialized=True)\n\n\n'
            f'@check(depends_on=export_{base.name}, severity="warn")\n'
            f"def export_{base.name}_ready(ctx):\n"
            '    """Check that the export ran."""\n'
            "    return ctx.pass_()\n"
        )

    def _discovered_set(self) -> None:
        self._features.add("discovered_set")
        self._files["python/nodes/assets/steps.py"] = (
            "from sqlbuild.assets import asset\n"
            "from sqlbuild.discovery import Discovered, Item, discover\n\n\n"
            '@discover(check_every="60s")\n'
            "def step_plan():\n"
            '    """List the steps of an external plan; only a location calls this."""\n'
            "    return Discovered(\n"
            '        items=(Item(id="load", args={"step": "load"}), '
            'Item(id="fit", depends_on=("load",), args={"step": "fit"})),\n'
            '        tags={"release": "r1"},\n'
            "    )\n\n\n"
            '@asset(for_each=step_plan, tags=("steps",))\n'
            "def run_step(ctx, step):\n"
            '    """Run one discovered step."""\n'
            '    return ctx.result(metadata={"step": step}, materialized=True)\n'
        )

    def _audit_factory(self) -> None:
        self._features.add("audit_factory")
        base: ModelPlan = self._base()
        self._files["python/nodes/audits/quality.py"] = (
            "from sqlbuild.audits import AuditCase, audit_factory\n\n\n"
            "@audit_factory\n"
            "def event_quality():\n"
            "    return [\n"
            '        AuditCase(name="id_present", definition="not_null", '
            'arguments={"column": "id"}),\n'
            "    ]\n"
        )
        _ = self._model(
            name=f"audited_{base.name}",
            description=f"Factory-audited copy of {base.name}",
            header="  audit_factories [event_quality],\n",
            body=f'SELECT id, amount\nFROM __ref("{base.name}")\n',
        )

    def _provider(self) -> None:
        self._features.add("provider")
        self._files["python/providers/export_settings.py"] = (
            "from sqlbuild.providers import Provider\n\n\n"
            "class ExportSettings(Provider):\n"
            '    """Settings for generated exports."""\n\n'
            f'    region: str = "{self._random.choice(("north", "south"))}"\n'
            "    batch_size: int = 50\n"
        )
        self._files["python/nodes/tasks/provided.py"] = (
            "from sqlbuild.assets import asset\n\n\n"
            "@asset\n"
            "def provided_export(ctx, export_settings):\n"
            '    """Export with provider settings."""\n'
            '    return ctx.result(payload={"region": export_settings.region})\n'
        )

    def _lifecycle_sink(self) -> None:
        self._features.add("lifecycle_sink")
        self._files["python/event_sinks/lifecycle.py"] = (
            "from sqlbuild.event_sinks import LifecycleEvent, LifecycleEventKind, lifecycle_event_sink\n"
            "\n\n"
            "@lifecycle_event_sink(\n"
            "    event_kinds={LifecycleEventKind.RUN, LifecycleEventKind.RESOURCE}\n"
            ")\n"
            "def ignore_lifecycle(event: LifecycleEvent) -> None:\n"
            "    return None\n"
        )

    def _command_output_sink(self) -> None:
        self._features.add("command_output_event_sink")
        self._files["python/event_sinks/output.py"] = (
            "from sqlbuild.event_sinks import CommandOutputRecord, command_output_event_sink\n\n\n"
            '@command_output_event_sink(streams={"stderr"})\n'
            "def ignore_output(record: CommandOutputRecord) -> None:\n"
            "    return None\n"
        )

    def _custom_materialization(self) -> None:
        self._features.add("custom_materialization")
        base: ModelPlan = self._base()
        self._files["python/materializations/copy_table.py"] = (
            "from sqlbuild.executor.custom.models import MaterializationContext, "
            "MaterializationResult\n\n\n"
            "def materialize(ctx: MaterializationContext) -> MaterializationResult:\n"
            '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS {ctx.sql}")\n'
            "    return MaterializationResult(relation=ctx.destination, audit_results=())\n"
        )
        _ = self._model(
            name=f"copied_{base.name}",
            description=f"Custom copy of {base.name}",
            header="  materialized copy_table,\n",
            body=f'SELECT id, status\nFROM __ref("{base.name}")\n',
        )

    def _project_adapter(self) -> None:
        self._features.add("project_adapter")
        self.adapter = GENERATOR_PROJECT_ADAPTER
        self._files["python/adapters/generated_duckdb.py"] = (
            "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
            "class GeneratedDuckDbAdapter(DuckDbAdapter):\n"
            f'    adapter_name = "{GENERATOR_PROJECT_ADAPTER}"\n'
        )
        self._files["python/adapter.py"] = (
            "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
            "class ProjectOverrideAdapter(DuckDbAdapter):\n"
            '    """Project adapter override."""\n\n'
            f'    adapter_name = "{GENERATOR_PROJECT_ADAPTER}_override"\n'
        )

    def _model_schema(self) -> None:
        self._features.add("model_schema")
        base: ModelPlan = self._base()
        self._files[f"{GENERATOR_FEATURE_FOLDER}/_sqlbuild/_schemas/event_shape.sql"] = (
            "SCHEMA (\n  name event_shape,\n"
            '  description "Canonical event shape",\n'
            "  columns (\n"
            "    id (type INTEGER, audits [not_null]),\n"
            "    status (type VARCHAR),\n"
            "  ),\n);\n"
        )
        _ = self._model(
            name=f"shaped_{base.name}",
            description=f"Schema-shaped copy of {base.name}",
            header="  model_schema event_shape,\n  contract enforced,\n",
            body=f'SELECT CAST(id AS INTEGER) AS id, status\nFROM __ref("{base.name}")\n',
        )

    def _list_constant(self) -> None:
        self._features.add("list_constant")
        base: ModelPlan = self._base()
        values: list[str] = self._random.sample(["placed", "expédié", "取消", "returned ✓"], k=2)
        self._files[f"{GENERATOR_FEATURE_FOLDER}/_sqlbuild/_constants/statuses.sql"] = (
            "CONSTANT (\n  name kept_statuses,\n  value ["
            + ", ".join(f'"{value}"' for value in values)
            + "],\n);\n"
            "CONSTANT (\n  name amount_weights,\n  value [0.5, 1.25, 2500.75],\n);\n"
        )
        _ = self._model(
            name=f"kept_{base.name}",
            description=f"Statuses kept from {base.name}",
            header="",
            body=(
                f'SELECT id, status, amount * 0.5 AS weighted\nFROM __ref("{base.name}")\n'
                'WHERE status IN @const("kept_statuses")\n'
                '  AND amount * 1.0 IN @const("amount_weights") OR id > 0\n'
            ),
        )

    def _cross_file_macro_import(self) -> None:
        self._features.add("cross_file_macro_import")
        base: ModelPlan = self._base()
        self._files["sql/macros/python/rounding.py"] = (
            "def round_amount(expression: str, places: int = 2) -> str:\n"
            '    """Round an amount."""\n'
            '    return f"ROUND({expression}, {places})"\n'
        )
        self._files["sql/macros/python/tax.py"] = (
            "from macros.python.rounding import round_amount\n\n\n"
            "def taxed(expression: str) -> str:\n"
            '    """Add tax to an amount, rounded."""\n'
            '    return round_amount(f"{expression} * 1.2")\n'
        )
        _ = self._model(
            name=f"taxed_{base.name}",
            description=f"Taxed copy of {base.name}",
            header="",
            body=(
                'SELECT id, @py.taxed("amount") AS taxed, @py.round_amount("amount", 1) AS '
                "rounded\n"
                f'FROM __ref("{base.name}")\n'
            ),
        )
        self._files["tests/unit/test_taxed_macro.sql"] = (
            'TEST (mode macro, name "taxes_amounts");\n\n'
            "WITH\ninput_values AS (\n  SELECT 10 AS amount\n),\n"
            '__macro_actual__ AS (\n  SELECT @py.taxed("amount") AS taxed\n  FROM '
            "input_values\n),\n"
            "__macro_expected__ AS (\n  SELECT 12.0 AS taxed\n)\nSELECT 1\n"
        )

    def _macro_test_mode(self) -> None:
        self._features.add("macro_test_mode")
        self._files["tests/unit/_macros/python/scaling.py"] = (
            "def doubled(expression: str) -> str:\n"
            '    """Double an amount."""\n'
            '    return f"({expression} * 2)"\n'
        )
        self._files["tests/unit/test_doubled_macro.sql"] = (
            'TEST (mode macro, name "doubles_amounts");\n\n'
            "WITH\ninput_values AS (\n  SELECT 4 AS amount\n),\n"
            '__macro_actual__ AS (\n  SELECT @py.doubled("amount") AS doubled\n'
            "  FROM input_values\n),\n"
            "__macro_expected__ AS (\n  SELECT 8 AS doubled\n)\nSELECT 1\n"
        )

    def _parameterized_test(self) -> None:
        self._features.add("parameterized_test")
        base: ModelPlan = self._base()
        source: str = base.inputs[0].split(":", 1)[1]
        self._files[f"tests/unit/test_{base.name}_cases.sql"] = (
            "TEST (\n"
            f'  name "{base.name}_keeps_status",\n'
            "  parameters (\n    status_value string,\n    amount_value float,\n  ),\n"
            "  cases (\n"
            f'    placed (status_value "placed", amount_value 1.5),\n'
            f'    labelled (status_value "{self._label()}", amount_value 2.25),\n'
            "  ),\n);\n\n"
            "WITH\n"
            f"__source__{source} AS (\n"
            '  SELECT 1 AS id, CAST(@param("amount_value") AS DOUBLE) AS amount,\n'
            '    @param("status_value") AS status,\n'
            "    TIMESTAMP '2026-01-01 00:00:00' AS created_at\n"
            "),\n"
            f"__expected__{base.name} AS (\n"
            "  SELECT 1 AS id\n"
            ")\nSELECT 1\n"
        )

    def _generic_audit(self) -> None:
        self._features.add("generic_audit")
        base: ModelPlan = self._base()
        self._files[f"{GENERATOR_FEATURE_FOLDER}/_sqlbuild/_audits/generic/amount_at_least.sql"] = (
            'AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE @column < @minimum\n'
        )
        _ = self._model(
            name=f"guarded_{base.name}",
            description=f"Generic-audited copy of {base.name}",
            header="  columns (\n    amount (audits [amount_at_least (minimum 0)]),\n  ),\n",
            body=f'SELECT id, amount\nFROM __ref("{base.name}")\n',
        )

    def _functions(self) -> None:
        self._features.add("functions")
        self._files["sql/functions/sql/is_placed.sql"] = (
            "FUNCTION (\n"
            '  description "Whether a status is placed.",\n'
            "  arguments (event_status STRING),\n"
            "  returns BOOLEAN,\n);\n\n"
            "event_status = 'placed'\n"
        )
        self._files["sql/functions/python/is_large.py"] = (
            "from sqlbuild.functions import udf\n\n\n"
            '@udf(arguments={"amount": "DOUBLE"}, returns="BOOLEAN", runtime_version="3.11")\n'
            "def main(amount: float | None) -> bool:\n"
            '    """Whether an amount is large."""\n'
            "    return amount is not None and amount > 10\n"
        )
        self._files["tests/unit/test_is_placed_udf.sql"] = (
            'TEST (mode udf, name "detects_placed_events");\n\n'
            "WITH\ninput_values AS (\n  SELECT 'placed' AS event_status\n),\n"
            "__udf_actual__ AS (\n"
            '  SELECT event_status, __udf("is_placed")(event_status) AS is_placed\n'
            "  FROM input_values\n),\n"
            "__udf_expected__ AS (\n  SELECT 'placed' AS event_status, TRUE AS is_placed\n)\n"
            "SELECT 1\n"
        )
        base: ModelPlan = self._base()
        _ = self._model(
            name=f"flagged_{base.name}",
            description=f"Function-flagged copy of {base.name}",
            header="",
            body=(
                'SELECT id, __udf("is_placed")(status) AS is_placed,\n'
                '  __udf("is_large")(amount) AS is_large\n'
                f'FROM __ref("{base.name}")\n'
            ),
        )

    def _local_config(self) -> None:
        self._features.add("local_config")
        self._files["sqlbuild_local.toml"] = (
            "[vars]\n"
            f'site_label = "{self._label()}"\n\n'
            "[settings]\n"
            f"query_change_tracking = {self._random.choice(('true', 'false'))}\n"
        )

    def _target_override(self) -> None:
        self._features.add("target_override")
        self.config_lines.extend(
            [
                "",
                "[targets.ci]",
                f"schema = \"${{coalesce(ENV:{GENERATOR_MISSING_ENV_VAR}, 'ci_checks')}}\"",
                "",
                "[targets.ci.vars]",
                'region = "west"',
            ]
        )
        self._files.setdefault("sqlbuild_local.toml", "")
        self._files["sqlbuild_local.toml"] = (
            'target = "ci"\n\n' + self._files["sqlbuild_local.toml"]
        )

    def _dbt_ref(self) -> None:
        self._features.add("dbt_ref")
        self.expected_error_code = GENERATOR_DBT_REF_ERROR_CODE
        self.succeeding_commands = (PLAN_LABEL,)
        self.config_lines.extend(
            ["", "[dbt]", 'project_dir = "dbt"', 'target_path = "dbt/artifacts"']
        )
        self._files["dbt/dbt_project.yml"] = "name: upstream\nversion: '1.0.0'\nprofile: upstream\n"
        self._files["dbt/artifacts/manifest.json"] = GENERATOR_DBT_MANIFEST
        _ = self._model(
            name="dbt_order_counts",
            description="Counts from the upstream dbt model",
            header="",
            body=(
                "SELECT customer_id, COUNT(*) AS order_count\n"
                f'FROM __dbt_ref("upstream", "{GENERATOR_DBT_MODEL}")\nGROUP BY customer_id\n'
            ),
        )

    def _line_endings(self) -> None:
        self._features.add("line_endings")
        base: ModelPlan = self._base()
        path: str = self._model(
            name=f"spaced_{base.name}",
            description=f"Copy of {base.name} with {self._label()}",
            header='  tags ["línea", "東京"],\n',
            body=(
                f"-- Généré pour {self._label()}\n"
                "SELECT\n"
                "  id,\n"
                f"  '{self._label()}' AS label, -- étiquette ✓\n"
                "  amount\n"
                f'FROM __ref("{base.name}")\n'
            ),
        )
        self._files[path] = self._files[path].replace("\n  ", "\n\t").replace("\n", GENERATOR_CRLF)
        self._files["sql/sources/raw.yml"] = GENERATOR_BOM + self._files["sql/sources/raw.yml"].replace(
            "\n", GENERATOR_CRLF
        )


def feature_blocks_for_seed(*, seed: int, rng: random.Random) -> tuple[str, ...]:
    """Return stride-forced blocks plus a random share of the rest; rare blocks are residue-only."""

    blocks: list[str] = []
    for index, block in enumerate(GENERATOR_FEATURE_BLOCKS):
        rare_residue: int | None = GENERATOR_RARE_FEATURE_BLOCKS.get(block)
        if rare_residue is not None:
            selected: bool = seed % GENERATOR_RARE_FEATURE_PERIOD == rare_residue
        else:
            selected = (
                index % GENERATOR_FEATURE_STRIDE == seed % GENERATOR_FEATURE_STRIDE
                or rng.random() < GENERATOR_OPTIONAL_FEATURE_SHARE
            )
        if selected:
            blocks.append(block)
    return tuple(blocks)
