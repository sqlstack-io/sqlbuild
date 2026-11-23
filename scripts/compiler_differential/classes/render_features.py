"""Write the optional project inputs that make generated seeds cover every render input kind."""

from __future__ import annotations

import random
from collections.abc import Callable

from scripts.compiler_differential.constants import (
    GENERATOR_CHANNEL_ENV_VAR,
    GENERATOR_MISSING_ENV_VAR,
    GENERATOR_RENDER_FOLDER,
    GENERATOR_RENDER_TEST_FOLDER,
)
from scripts.compiler_differential.models import ModelPlan


class RenderFeatureWriter:
    """Add self-contained rendering blocks to one generated project, chosen per seed."""

    def __init__(
        self,
        *,
        blocks: tuple[str, ...],
        rng: random.Random,
        files: dict[str, str],
        features: set[str],
        staging: list[ModelPlan],
        sources: list[str],
        label: Callable[[], str],
    ) -> None:
        self._blocks: tuple[str, ...] = blocks
        self._random: random.Random = rng
        self._files: dict[str, str] = files
        self._features: set[str] = features
        self._staging: list[ModelPlan] = staging
        self._sources: list[str] = sources
        self._label: Callable[[], str] = label
        self.config_lines: list[str] = []
        self.templated_target: bool = False

    def write(self) -> None:
        """Write the selected rendering blocks in their canonical order."""

        writers: dict[str, Callable[[], None]] = {
            "macro_context_reads": self._macro_context_reads,
            "typed_reference_macro": self._typed_reference_macro,
            "macro_generated_reference": self._macro_generated_reference,
            "table_function": self._table_function,
            "interpolation": self._interpolation,
            "cursor_bounds": self._cursor_bounds,
            "macros_across_resources": self._macros_across_resources,
            "enum_column_contract": self._enum_column_contract,
            "resource_audits": self._resource_audits,
            "model_config": self._model_config,
        }
        for block in self._blocks:
            writer: Callable[[], None] | None = writers.get(block)
            if writer is not None:
                writer()

    def _base(self) -> ModelPlan:
        return self._random.choice(self._staging)

    def _model(self, *, name: str, header: str, body: str, description: str) -> None:
        self._files[f"{GENERATOR_RENDER_FOLDER}/{name}.sql"] = (
            f'MODEL (\n  description "{description}",\n{header});\n\n{body}'
        )

    def _owned(self, *, role: str, name: str) -> str:
        return f"{GENERATOR_RENDER_FOLDER}/_sqlbuild/_{role}/{name}"

    def _macro_context_reads(self) -> None:
        self._features.add("macro_context_reads")
        base: ModelPlan = self._base()
        minimum: int = self._random.randint(1, 4)
        self._files[self._owned(role="constants", name="render_limits.sql")] = (
            f"CONSTANT (name minimum_render_amount, value {minimum});\n"
        )
        self._files[self._owned(role="enums", name="render_channel.sql")] = (
            'ENUM (\n  name render_channel,\n  members (WEB "web", STORE "store"),\n);\n'
        )
        self._files[self._owned(role="macros", name="python/nodes/policy.py")] = (
            "import os\n\n\n"
            "def minimum_amount_filter(ctx, column: str) -> str:\n"
            '    """Keep rows at or above the configured minimum amount."""\n'
            '    if ctx.constants["minimum_render_amount"] is None:\n'
            '        return "TRUE"\n'
            "    return f\"{column} >= {ctx.render_constant('minimum_render_amount')}\"\n\n\n"
            "def channel_label(ctx) -> str:\n"
            '    """Label rows with the web channel, target and adapter."""\n'
            '    channel = ctx.render_enum_member(enum_name="render_channel", member_name="WEB")\n'
            '    region = ctx.vars["region"]\n'
            "    return f\"{channel} || '/{ctx.target_name}/{ctx.adapter_name}/{region}'\"\n\n\n"
            "def with_deployment(expression: str) -> str:\n"
            '    """Suffix an expression with the deployment channel from the environment."""\n'
            f'    channel = os.environ.get("{GENERATOR_CHANNEL_ENV_VAR}", "none")\n'
            "    return f\"({expression} || '@{channel}')\"\n"
        )
        self._model(
            name=f"policy_{base.name}",
            description=f"Policy-filtered copy of {base.name}",
            header="",
            body=(
                "SELECT id, amount, @py.with_deployment(@py.channel_label()) AS channel,\n"
                '  @const("minimum_render_amount") AS minimum_amount,\n'
                '  @enum("render_channel").STORE AS fallback_channel\n'
                f'FROM __ref("{base.name}")\n'
                'WHERE @py.minimum_amount_filter("amount")\n'
            ),
        )

    def _typed_reference_macro(self) -> None:
        self._features.add("typed_reference_macro")
        first, second = (
            self._random.sample(self._staging, k=2)
            if len(self._staging) > 1
            else (self._staging * 2)
        )
        source: str = self._random.choice(self._sources)
        self._files[self._owned(role="macros", name="python/nodes/relations.py")] = (
            "from sqlbuild.refs import SqlResourceRef\n\n\n"
            "def union_ids(relations: list[SqlResourceRef]) -> str:\n"
            '    """Union the ids of several relations."""\n'
            '    selects = [f"SELECT id FROM {relation}" for relation in relations]\n'
            '    return " UNION ALL ".join(selects)\n'
            "\n\n"
            "def latest_rows(relation: SqlResourceRef, limit: int = 2) -> str:\n"
            '    """Return the newest rows of one relation."""\n'
            '    return f"(SELECT id, amount FROM {relation} ORDER BY id DESC LIMIT {limit})"\n'
        )
        self._model(
            name="unioned_event_ids",
            description="Ids from two staged relations",
            header="",
            body=(
                "SELECT id\n"
                f'FROM (@py.union_ids([__ref("{first.name}"), __ref("{second.name}")])) AS '
                "unioned\n"
            ),
        )
        self._model(
            name=f"latest_{source}",
            description=f"Newest rows of {source}",
            header="",
            body=(
                f'SELECT id, amount\nFROM @py.latest_rows(__source("{source}"), limit=2) AS '
                "latest\n"
            ),
        )

    def _macro_generated_reference(self) -> None:
        self._features.add("macro_generated_reference")
        base: ModelPlan = self._base()
        self.config_lines.extend(["", "[references]", "enforce_explicit = false"])
        self._files[self._owned(role="macros", name="python/nodes/legacy.py")] = (
            "def legacy_events() -> str:\n"
            '    """Return a staged relation the way a migrated project wrote it."""\n'
            f"    return '__ref(\"{base.name}\")'\n"
        )
        self._model(
            name="legacy_event_ids",
            description="Ids read through a macro-generated reference",
            header="",
            body="SELECT id\nFROM @py.legacy_events() AS legacy\n",
        )

    def _table_function(self) -> None:
        self._features.add("table_function")
        base: ModelPlan = self._base()
        self._files["sql/functions/sql/table_fn__events_above.sql"] = (
            "FUNCTION (\n"
            '  description "Events above an amount",\n'
            "  arguments (p_minimum DOUBLE),\n"
            "  returns table (\n    id INTEGER,\n    amount DOUBLE\n  ),\n);\n\n"
            f'SELECT id, amount\nFROM __ref("{base.name}")\nWHERE amount > p_minimum\n'
        )
        self._model(
            name="large_events",
            description="Events above a threshold from a table function",
            header="",
            body=(
                "SELECT id, amount\n"
                f'FROM __table_fn("table_fn__events_above")({self._random.choice(("1.5", "4"))})\n'
            ),
        )

    def _interpolation(self) -> None:
        self._features.add("interpolation")
        base: ModelPlan = self._base()
        self._files[self._owned(role="sql/hooks/sql", name="announce_refresh.sql")] = (
            'HOOK (\n  description "Announce one refreshed relation"\n);\n\n'
            "SELECT @'relation' AS refreshed_relation, @'label' AS refresh_label\n"
        )
        self._files[self._owned(role="macros", name="python/nodes/notes.py")] = (
            "def refresh_note(relation: str) -> str:\n"
            '    """Describe one refreshed relation."""\n'
            "    return f\"'refreshed ' || '{relation}'\"\n"
        )
        self._model(
            name=f"interpolated_{base.name}",
            description=f"Interpolated copy of {base.name}",
            header=(
                "  materialized table,\n"
                "  post_hooks [\n"
                '    sql("announce_refresh", relation: "@@CTX:destination.qualified", '
                f'label: "{self._label()}"),\n'
                "    inline_sql(\"SELECT @py.refresh_note('@@CTX:destination.qualified') AS "
                'note"),\n'
                "    inline_sql(\"SELECT '@@CTX:run.target' AS target, "
                "'@@CTX:model.name' AS model\"),\n"
                "  ],\n"
            ),
            body=(
                "SELECT id,\n"
                "  '@@site_label' AS site,\n"
                "  $$--@@site_label /* $$ AS site_note,\n"
                f"  '@@ENV:{GENERATOR_CHANNEL_ENV_VAR}' AS channel,\n"
                "  amount * @@discount_rate AS discounted\n"
                f'FROM __ref("{base.name}")\n'
            ),
        )
        self._deferred_placeholder(base)

    def _deferred_placeholder(self, base: ModelPlan) -> None:
        self._files["python/materializations/windowed_copy.py"] = (
            "from sqlbuild.executor.custom.models import MaterializationContext, "
            "MaterializationResult\n\n\n"
            "def materialize(ctx: MaterializationContext) -> MaterializationResult:\n"
            '    sql = ctx.sql.replace("@@@window_start", "DATE \'2026-01-01\'")\n'
            '    ctx.execute_sql(f"CREATE OR REPLACE TABLE {ctx.destination} AS {sql}")\n'
            "    return MaterializationResult(relation=ctx.destination, audit_results=())\n"
        )
        self._model(
            name=f"windowed_{base.name}",
            description=f"Runtime-windowed copy of {base.name}",
            header=(
                "  materialized windowed_copy,\n"
                "  placeholders (\n    window_start \"'2026-01-01'\",\n  ),\n"
                "  config (\n    window_column created_at,\n  ),\n"
            ),
            body=(
                f'SELECT id, created_at\nFROM __ref("{base.name}")\n'
                "WHERE CAST(created_at AS DATE) >= CAST(@@@window_start AS DATE)\n"
            ),
        )

    def _cursor_bounds(self) -> None:
        self._features.add("cursor_bounds")
        base: ModelPlan = self._base()
        self._model(
            name=f"bounded_{base.name}",
            description=f"Cursor-bounded copy of {base.name}",
            header=(
                "  materialized incremental,\n"
                "  incremental_strategy delete_insert,\n"
                "  unique_key [id],\n"
                "  cursor created_at,\n  cursor_type timestamp,\n  cursor_grain day,\n"
                "  incremental_mode microbatch,\n"
                "  microbatch_strategy rolling_window,\n"
                "  lookback 1d,\n"
                "  batch_size 1d,\n"
                f"  cursor_inputs (\n    {base.name} created_at,\n  ),\n"
            ),
            body=(
                "SELECT id, amount, created_at\n"
                f'FROM __ref("{base.name}")\n'
                "WHERE created_at >= __cursor_start()\n  AND created_at < __cursor_end()\n"
            ),
        )

    def _macros_across_resources(self) -> None:
        self._features.add("macros_across_resources")
        base: ModelPlan = self._base()
        source: str = base.inputs[0].split(":", 1)[1]
        self._files["sql/macros/python/labels.py"] = (
            "def tidy_label(expression: str) -> str:\n"
            '    """Trim and lower-case a label."""\n'
            '    return f"LOWER(TRIM({expression}))"\n'
        )
        self._files[self._owned(role="sql/audits/generic", name="tidy_status.sql")] = (
            'AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE @column <> '
            '@py.tidy_label("@column")\n'
        )
        self._files["sql/functions/sql/tidy_status.sql"] = (
            "FUNCTION (\n"
            '  description "Normalise a status label.",\n'
            "  arguments (raw_status STRING),\n"
            "  returns STRING,\n);\n\n"
            '@py.tidy_label("raw_status")\n'
        )
        self._model(
            name=f"labelled_{base.name}",
            description=f"Labelled copy of {base.name}",
            header="  columns (\n    status (audits [tidy_status]),\n  ),\n",
            body=(
                'SELECT id, @py.tidy_label("status") AS status,\n'
                '  __udf("tidy_status")(status) AS tidied\n'
                f'FROM __ref("{base.name}")\n'
            ),
        )
        self._files["sql/sources/labelled.yml"] = (
            "sources:\n"
            "  - name: labelled_channels\n"
            "    description: Channel labels normalised in the source expression.\n"
            '    expression: "(SELECT 1 AS id, @py.tidy_label(\\"\' Web \'\\") AS channel)"\n'
            "    columns:\n"
            "      - name: id\n        type: INTEGER\n"
            "      - name: channel\n        type: VARCHAR\n"
        )
        self._model(
            name="channel_labels",
            description="Normalised channel labels",
            header="",
            body='SELECT id, channel\nFROM __source("labelled_channels")\n',
        )
        self._files[
            f"{GENERATOR_RENDER_FOLDER}/_sqlbuild/audits/singular/labelled_channels_match.sql"
        ] = (
            'AUDIT (name "labelled_channels_match");\n\n'
            f'SELECT l.id\nFROM __ref("labelled_{base.name}") l\n'
            'JOIN __ref("channel_labels") c ON l.id = c.id\nWHERE l.status = c.channel\n'
        )
        self._files[f"tests/scenarios/labelled_{base.name}_scenario.sql"] = (
            'SCENARIO (\n  description "Labels are normalised end to end"\n);\n\n'
            "WITH\n"
            f"__source__{source} AS (\n"
            "  SELECT 3 AS id, CAST(1 AS DOUBLE) AS amount, ' Placed ' AS status,\n"
            "    TIMESTAMP '2026-01-03 00:00:00' AS created_at\n"
            "),\n"
            f"__expected__labelled_{base.name} AS (\n"
            "  SELECT 3 AS id, @py.tidy_label(\"' Placed '\") AS status\n"
            ")\nSELECT 1\n"
        )
        self._files[f"{GENERATOR_RENDER_TEST_FOLDER}/_sqlbuild/_macros/python/fixtures.py"] = (
            "def event_rows(count: int = 2) -> str:\n"
            '    """Return fixture event rows."""\n'
            '    return " UNION ALL ".join(\n'
            '        f"SELECT {index} AS id, CAST({index} AS DOUBLE) AS amount, "\n'
            "        f\"'placed' AS status, \"\n"
            "        f\"TIMESTAMP '2026-01-0{index} 00:00:00' AS created_at\"\n"
            "        for index in range(1, count + 1)\n"
            "    )\n"
        )
        self._files[f"{GENERATOR_RENDER_TEST_FOLDER}/test_labelled_{base.name}.sql"] = (
            "TEST();\n\nWITH\n"
            f"__source__{source} AS (\n  @py.event_rows(2)\n),\n"
            f"__expected__labelled_{base.name} AS (\n"
            "  SELECT 1 AS id, @py.tidy_label(\"'placed'\") AS status\n"
            "  UNION ALL\n"
            "  SELECT 2 AS id, 'placed' AS status\n"
            ")\nSELECT 1\n"
        )

    def _enum_column_contract(self) -> None:
        self._features.add("enum_column_contract")
        base: ModelPlan = self._base()
        self._files[self._owned(role="enums", name="render_state.sql")] = (
            "ENUM (\n  name render_state,\n  members (\n"
            '    PLACED "placed",\n    SHIPPED "expédié",\n    CANCELLED "取消",\n'
            '    RETURNED "returned ✓",\n  ),\n);\n'
        )
        self._model(
            name=f"stated_{base.name}",
            description=f"Enum-typed copy of {base.name}",
            header=(
                "  contract enforced,\n"
                "  columns (\n"
                "    id (type INTEGER),\n"
                "    status (type render_state),\n"
                "    tier (type VARCHAR),\n"
                "  ),\n"
                f"  constants (\n    _tier_floor {self._random.randint(1, 9)},\n  ),\n"
                "  enums (\n    _tier [LOW, HIGH],\n  ),\n"
            ),
            body=(
                "SELECT CAST(id AS INTEGER) AS id, status,\n"
                '  CASE WHEN amount >= @const("_tier_floor") THEN @enum("_tier").HIGH\n'
                '    ELSE @enum("_tier").LOW END AS tier\n'
                f'FROM __ref("{base.name}")\n'
            ),
        )

    def _resource_audits(self) -> None:
        self._features.add("resource_audits")
        self._files["sql/sources/audited.yml"] = (
            "sources:\n"
            "  - name: audited_events\n"
            "    description: Events with source audits.\n"
            '    expression: "(SELECT 1 AS id, CAST(@@discount_rate AS DOUBLE) AS amount)"\n'
            "    columns:\n"
            "      - name: id\n"
            "        type: INTEGER\n"
            "        audits:\n"
            "          - not_null\n"
            "          - unique\n"
            "          - accepted_values:\n"
            "              values: [1, 2]\n"
            "      - name: amount\n"
            "        type: DOUBLE\n"
        )
        self._files["sql/seeds/channel_codes.csv"] = f'id,label\n1,"{self._label()}"\n2,web\n'
        self._files["sql/seeds/channel_codes.yml"] = (
            "seeds:\n"
            "  - name: channel_codes\n"
            "    description: Channel codes with seed audits.\n"
            "    columns:\n"
            "      - name: id\n"
            "        type: INTEGER\n"
            "        audits:\n"
            "          - not_null\n"
            "      - name: label\n"
            "        type: VARCHAR\n"
        )
        self._files[self._owned(role="sql/audits/generic", name="amount_floor.sql")] = (
            'AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE amount < @minimum\n'
        )
        self._model(
            name="audited_event_totals",
            description="Totals of the audited events per channel",
            header=f"  audits [amount_floor (minimum {self._random.randint(-40, -2)})],\n",
            body=(
                "SELECT c.label, SUM(e.amount) AS amount\n"
                'FROM __source("audited_events") e\n'
                'JOIN __seed("channel_codes") c ON e.id = c.id\n'
                "GROUP BY c.label\n"
            ),
        )

    def _model_config(self) -> None:
        self._features.add("model_config")
        self.templated_target = True
        base: ModelPlan = self._base()
        self._files[self._owned(role="sql/audits/generic", name="configured_floor.sql")] = (
            'AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE amount < @minimum\n'
        )
        self._model(
            name=f"configured_{base.name}",
            description=f"Configured copy of {base.name} (réglé)",
            header=(
                "  materialized table,\n"
                f'  schema "${{coalesce(ENV:{GENERATOR_MISSING_ENV_VAR}, '
                f"'configured')}}_${{CTX:run.target}}\",\n"
                "  tags [\"${if(eq(CTX:run.target, 'dev'), 'development', 'release')}\"],\n"
                "  audits [\n"
                f"    configured_floor (minimum {self._random.randint(-40, -2)}, severity warn,\n"
                '      name configured_amount_floor, description "Amounts stay above the floor"),\n'
                "  ],\n"
                "  columns (\n"
                '    id (type INTEGER, nullable false, description "Row key",\n'
                "      audits [not_null (severity error, name configured_id_present),\n"
                "        unique (always_run true)]),\n"
                '    amount (type DOUBLE, description "Montant réglé"),\n'
                '    status (audits [accepted_values (values ["placed", "shipped"],\n'
                '      severity warn, description "Known statuses")]),\n'
                "  ),\n"
            ),
            body=f'SELECT id, amount, status\nFROM __ref("{base.name}")\n',
        )
        self._files["sql/sources/configured.yml"] = (
            "sources:\n"
            "  - name: configured_events\n"
            f'    description: "Configured feed ${{coalesce(ENV:{GENERATOR_MISSING_ENV_VAR}, '
            "'events')}\"\n"
            '    expression: "(SELECT 1 AS id, CAST(2.5 AS DOUBLE) AS amount)"\n'
            "    meta:\n"
            f"      owner: \"${{coalesce(ENV:{GENERATOR_MISSING_ENV_VAR}, 'orders team')}}\"\n"
            "    columns:\n"
            "      - name: id\n"
            "        type: INTEGER\n"
            f"        description: \"Key ${{coalesce(ENV:{GENERATOR_MISSING_ENV_VAR}, 'id')}}\"\n"
            "        audits:\n"
            "          - accepted_values:\n"
            f"              values: [\"${{coalesce(ENV:{GENERATOR_MISSING_ENV_VAR}, '1')}}\"]\n"
            "      - name: amount\n"
            "        type: DOUBLE\n"
        )
        self._files["sql/functions/sql/configured_label.sql"] = (
            "FUNCTION (\n"
            '  description "Label a configured status.",\n'
            f"  schema \"${{coalesce(ENV:{GENERATOR_MISSING_ENV_VAR}, 'udfs')}}\",\n"
            "  arguments (raw_status STRING),\n"
            "  returns STRING,\n);\n\n"
            "UPPER(raw_status)\n"
        )
