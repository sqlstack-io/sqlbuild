"""Write the optional project inputs that make generated seeds cover every analysis input kind."""

from __future__ import annotations

import random
from collections.abc import Callable

from scripts.compiler_differential.constants import (
    GENERATOR_ANALYSIS_FOLDER,
    GENERATOR_ANALYSIS_MODE_COMMANDS,
    GENERATOR_ANALYSIS_MODEL,
    GENERATOR_ANALYSIS_TEST_FOLDER,
    GENERATOR_OPEN_SOURCE,
    GENERATOR_SELECTED_MODEL,
)
from scripts.compiler_differential.models import DifferentialCommand, ModelPlan


class AnalysisFeatureWriter:
    """Add self-contained SQL analysis blocks to one generated project, chosen per seed."""

    def __init__(
        self,
        *,
        blocks: tuple[str, ...],
        rng: random.Random,
        files: dict[str, str],
        features: set[str],
        staging: list[ModelPlan],
        sources: list[str],
    ) -> None:
        self._blocks: tuple[str, ...] = blocks
        self._random: random.Random = rng
        self._files: dict[str, str] = files
        self._features: set[str] = features
        self._staging: list[ModelPlan] = staging
        self._sources: list[str] = sources
        self.config_lines: list[str] = []
        self.settings_lines: list[str] = []
        self.commands: tuple[DifferentialCommand, ...] = ()

    def write(self) -> None:
        """Write the selected analysis blocks in their canonical order."""

        writers: dict[str, Callable[[], None]] = {
            "column_shapes": self._column_shapes,
            "contract_chain": self._contract_chain,
            "udf_signatures": self._udf_signatures,
            "dynamic_columns": self._dynamic_columns,
            "quoted_identifiers": self._quoted_identifiers,
            "metadata_checks": self._metadata_checks,
            "sql_test_mocks": self._sql_test_mocks,
            "analysis_opt_out": self._analysis_opt_out,
            "python_sql": self._python_sql,
            "analysis_modes": self._analysis_modes,
        }
        for block in self._blocks:
            writer: Callable[[], None] | None = writers.get(block)
            if writer is not None:
                writer()

    def _base(self) -> ModelPlan:
        return self._random.choice(self._staging)

    def _model(self, *, name: str, header: str, body: str, description: str) -> None:
        self._files[f"{GENERATOR_ANALYSIS_FOLDER}/{name}.sql"] = (
            f'MODEL (\n  description "{description}",\n{header});\n\n{body}'
        )

    def _column_shapes(self) -> None:
        self._features.add("column_shapes")
        base: ModelPlan = self._base()
        other: ModelPlan = self._base()
        source: str = self._random.choice(self._sources)
        self._model(
            name=f"typed_{base.name}",
            description=f"Typed and untyped outputs of {base.name}",
            header="",
            body=(
                "SELECT id, amount * 2 AS doubled_amount, CAST(status AS VARCHAR) AS status_text,\n"
                "  created_at, NULL AS missing_note\n"
                f'FROM __ref("{base.name}")\n'
            ),
        )
        self._model(
            name=f"every_{source}",
            description=f"Every column of {source}",
            header="",
            body=f'SELECT *\nFROM __source("{source}")\n',
        )
        self._model(
            name=f"every_{base.name}",
            description=f"Every column of {base.name}",
            header="",
            body=f'SELECT *\nFROM __ref("{base.name}")\n',
        )
        self._files["sources/open.yml"] = (
            "sources:\n"
            f"  - name: {GENERATOR_OPEN_SOURCE}\n"
            "    description: Events whose columns are only known to the warehouse.\n"
        )
        self._model(
            name="every_open_event",
            description="Every column of an open source",
            header="",
            body=f'SELECT *\nFROM __source("{GENERATOR_OPEN_SOURCE}")\n',
        )
        self._model(
            name=f"combined_{base.name}",
            description=f"Recent and older rows of {base.name} and {other.name}",
            header="",
            body=(
                "WITH recent AS (\n"
                f'  SELECT id, created_at FROM __ref("{base.name}") WHERE amount > 0\n'
                "),\nolder AS (\n"
                f'  SELECT id, created_at FROM __ref("{other.name}")\n'
                ")\n"
                "SELECT id, created_at FROM recent\nUNION ALL\nSELECT id, created_at FROM older\n"
            ),
        )
        self._model(
            name=f"passed_{base.name}",
            description=f"Columns of {base.name} passed through a CTE",
            header="",
            body=(
                f'WITH passthrough AS (\n  SELECT id, created_at FROM __ref("{base.name}")\n)\n'
                "SELECT id, created_at\nFROM passthrough\n"
            ),
        )

    def _contract_chain(self) -> None:
        self._features.add("contract_chain")
        base: ModelPlan = self._base()
        width: int = self._random.randint(16, 40)
        source: str = base.inputs[0].split(":", 1)[1]
        contracted: str = f"contracted_{base.name}"
        self._model(
            name=contracted,
            description=f"Contracted copy of {source}",
            header=(
                "  materialized table,\n"
                "  contract enforced,\n"
                "  columns (\n"
                "    id (type INTEGER, nullable false),\n"
                f"    label (type VARCHAR({width})),\n"
                "    amount (type DOUBLE),\n"
                "  ),\n"
            ),
            body=(
                f"SELECT CAST(id AS INTEGER) AS id, CAST(status AS VARCHAR({width})) AS label,\n"
                "  amount\n"
                f'FROM __source("{source}")\n'
            ),
        )
        upstream: str = contracted
        for level, expression in enumerate(("label", "UPPER(label) AS label", "label"), start=1):
            name: str = f"chain_{level}_{base.name}"
            self._model(
                name=name,
                description=f"Level {level} below the contracted copy of {base.name}",
                header="",
                body=f'SELECT id, {expression}, amount\nFROM __ref("{upstream}")\n',
            )
            upstream = name
        self.settings_lines.append('table_promotion_mode = "staged"')

    def _udf_signatures(self) -> None:
        self._features.add("udf_signatures")
        base: ModelPlan = self._base()
        factor: int = self._random.randint(2, 9)
        self._files["functions/sql/scaled_amount.sql"] = (
            "FUNCTION (\n"
            '  description "Scale an amount by an integer factor",\n'
            "  arguments (p_amount DOUBLE, p_factor INTEGER),\n"
            "  returns DOUBLE,\n);\n\n"
            "p_amount * p_factor\n"
        )
        self._files["functions/sql/table_fn__amount_bands.sql"] = (
            "FUNCTION (\n"
            '  description "Band staged amounts above a floor",\n'
            "  arguments (p_floor DOUBLE),\n"
            "  returns table (\n    id INTEGER,\n    band VARCHAR\n  ),\n);\n\n"
            "SELECT CAST(id AS INTEGER) AS id,\n"
            "  CASE WHEN amount > p_floor THEN 'high' ELSE 'low' END AS band\n"
            f'FROM __ref("{base.name}")\n'
        )
        self._model(
            name=f"scaled_{base.name}",
            description=f"Scaled amounts of {base.name}",
            header="",
            body=(
                f'SELECT id, __udf("scaled_amount")(amount, {factor}) AS scaled_amount\n'
                f'FROM __ref("{base.name}")\n'
            ),
        )
        self._model(
            name="banded_amounts",
            description="Amount bands from a table function",
            header="",
            body=(
                "SELECT bands.id, bands.band\n"
                f'FROM __table_fn("table_fn__amount_bands")({factor}.5) AS bands\n'
            ),
        )

    def _dynamic_columns(self) -> None:
        self._features.add("dynamic_columns")
        base: ModelPlan = self._base()
        pivot_input: str = f"pivot_input_{base.name}"
        self._model(
            name=pivot_input,
            description=f"Contracted pivot input from {base.name}",
            header=(
                "  materialized table,\n"
                "  contract enforced,\n"
                "  columns (\n"
                "    id (type INTEGER),\n"
                "    status (type VARCHAR),\n"
                "    amount (type DOUBLE),\n"
                "  ),\n"
            ),
            body=(
                "SELECT CAST(id AS INTEGER) AS id, CAST(status AS VARCHAR) AS status, amount\n"
                f'FROM __ref("{base.name}")\n'
            ),
        )
        self._model(
            name=f"pivoted_{base.name}",
            description=f"Amounts of {base.name} pivoted by status",
            header=(
                "  materialized table,\n"
                "  columns (id (type INTEGER)),\n"
                "  dynamic_columns (\n"
                "    status_amounts (\n"
                "      pivot_column status,\n"
                "      value_column amount,\n"
                "      aggregate MAX,\n"
                "      type DOUBLE\n"
                "    )\n"
                "  ),\n"
            ),
            body=f'PIVOT __ref("{pivot_input}")\nON status\nUSING MAX(amount)\nGROUP BY id\n',
        )

    def _quoted_identifiers(self) -> None:
        self._features.add("quoted_identifiers")
        base: ModelPlan = self._base()
        self._model(
            name=f"quoted_{base.name}",
            description=f"Quoted identifiers over {base.name}",
            header="",
            body=(
                'SELECT "ID" AS "OrderKey", "Amount", "Status" AS "Status"\n'
                f'FROM __ref("{base.name}")\n'
            ),
        )

    def _metadata_checks(self) -> None:
        self._features.add("metadata_checks")
        base: ModelPlan = self._base()
        self._files[
            f"{GENERATOR_ANALYSIS_FOLDER}/_sqlbuild/_audits/generic/expression_is_true.sql"
        ] = 'AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE NOT (@expression)\n'
        self._model(
            name=f"checked_{base.name}",
            description=f"Metadata-checked copy of {base.name}",
            header=(
                "  materialized table,\n"
                "  unique_key [id],\n"
                "  audits [\n"
                '    expression_is_true (expression "amount >= -1000", severity warn),\n'
                "  ],\n"
                "  columns (\n"
                f'    id (audits [relationships (to __ref("{base.name}"), field id)]),\n'
                "    status (audits [accepted_values (values "
                '["placed", "expédié", "取消", "returned ✓"], severity warn)]),\n'
                "  ),\n"
            ),
            body=f'SELECT id, amount, status\nFROM __ref("{base.name}")\n',
        )

    def _sql_test_mocks(self) -> None:
        self._features.add("sql_test_mocks")
        base: ModelPlan = self._base()
        region: str = self._random.choice(("north", "south", "east"))
        self._files[f"{GENERATOR_ANALYSIS_FOLDER}/_sqlbuild/_macros/python/regions.py"] = (
            "def analysis_region() -> str:\n"
            '    """Return the region rows are assigned to."""\n'
            f"    return \"'{region}'\"\n"
        )
        model: str = f"regional_{base.name}"
        self._model(
            name=model,
            description=f"Regional copy of {base.name}",
            header="",
            body=f'SELECT id, amount, @py.analysis_region() AS region\nFROM __ref("{base.name}")\n',
        )
        self._files[f"{GENERATOR_ANALYSIS_TEST_FOLDER}/test_{model}.sql"] = (
            "TEST();\n\nWITH\n"
            "__macro__analysis_region AS (\n  SELECT '''west'''\n),\n"
            f"__ref__{base.name} AS (\n"
            "  SELECT 1 AS id, CAST(4 AS DOUBLE) AS amount, 'placed' AS status,\n"
            "    TIMESTAMP '2026-01-01 00:00:00' AS created_at\n"
            "),\n"
            f"__expected__{model} AS (\n"
            "  SELECT 1 AS id, CAST(4 AS DOUBLE) AS amount, 'west' AS region\n"
            ")\nSELECT 1\n"
        )

    def _analysis_opt_out(self) -> None:
        self._features.add("analysis_opt_out")
        base: ModelPlan = self._base()
        self._model(
            name=f"unanalysed_{base.name}",
            description=f"Copy of {base.name} that SQL analysis cannot parse",
            header="  sql_analysis false,\n",
            body=(
                "SELECT a.id, a.amount\n"
                f'FROM __ref("{base.name}") AS a\n'
                f'JOIN __ref("{base.name}") AS b NEAREST 1 BY DISTANCE abs(a.amount - b.amount)\n'
            ),
        )
        self.settings_lines.append("require_sql_analysis = true")

    def _python_sql(self) -> None:
        self._features.add("python_sql")
        self._files["python/tasks/flag_exports.py"] = (
            "from sqlbuild.tasks import TaskContext, task\n\n\n"
            "@task\n"
            "def count_flagged_events(ctx: TaskContext):\n"
            '    """Count externally flagged events."""\n'
            '    ctx.query("SELECT COUNT(*) FROM main.external_event_flags")\n'
            "    return ctx.result()\n"
        )

    def _analysis_modes(self) -> None:
        self._features.add("analysis_modes")
        base: ModelPlan = self._base()
        self._model(
            name=f"cte_facts_{base.name}",
            description=f"Contracted CTE reads of {base.name}",
            header=(
                "  contract enforced,\n  columns (\n"
                + "".join(
                    f'    {column} (description "Recovered {column}"),\n'
                    for column in ("id", "amount", "kind", "status")
                )
                + "  ),\n"
            ),
            body=(
                "WITH base AS (\n"
                "  SELECT id, CAST(amount AS DECIMAL(12, 2)) AS amount, 'event' AS kind,\n"
                "    COALESCE(status, 'unknown') AS status\n"
                f'  FROM __ref("{base.name}")\n'
                "  WHERE id IS NOT NULL\n"
                ")\n"
                "SELECT id + 0 AS id, amount, UPPER(kind) AS kind, UPPER(status) AS status\n"
                "FROM base\n"
            ),
        )
        self.commands = tuple(
            _selected_command(
                label=label,
                arguments=arguments,
                model=base.name,
                analysis_model=f"cte_facts_{base.name}",
            )
            for label, arguments in GENERATOR_ANALYSIS_MODE_COMMANDS
        )


def _selected_command(
    *, label: str, arguments: tuple[str, ...], model: str, analysis_model: str
) -> DifferentialCommand:
    """Return one analysis-mode command with the selected and analysis models substituted."""

    return DifferentialCommand(
        label=label,
        arguments=tuple(
            argument.replace(GENERATOR_ANALYSIS_MODEL, analysis_model).replace(
                GENERATOR_SELECTED_MODEL, model
            )
            for argument in arguments
        ),
    )
