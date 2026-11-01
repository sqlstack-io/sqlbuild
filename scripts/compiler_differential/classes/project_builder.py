"""Build one seeded random project file by file."""

from __future__ import annotations

import random
from collections.abc import Callable
from dataclasses import replace

from scripts.compiler_differential.classes.analysis_features import AnalysisFeatureWriter
from scripts.compiler_differential.classes.discovery_features import (
    DiscoveryFeatureWriter,
    feature_blocks_for_seed,
)
from scripts.compiler_differential.classes.render_features import RenderFeatureWriter
from scripts.compiler_differential.constants import (
    DUCKDB_ADAPTER,
    GENERATOR_CONSTANT_KIND,
    GENERATOR_CROSS_DOMAIN_SHARE,
    GENERATOR_DIALECT_CONNECTIONS,
    GENERATOR_DIALECT_TARGETS,
    GENERATOR_DOMAINS,
    GENERATOR_ENUM_KIND,
    GENERATOR_FLOAT_VALUES,
    GENERATOR_HALF,
    GENERATOR_HOOK_KIND,
    GENERATOR_INVALID_CODES,
    GENERATOR_INVALID_SHARE,
    GENERATOR_LAYERS,
    GENERATOR_MACRO_KIND,
    GENERATOR_MISSING_ENV_VAR,
    GENERATOR_NON_ASCII_DESCRIPTION_SHARE,
    GENERATOR_NON_ASCII_LABELS,
    GENERATOR_PYTHON_MACRO_FOLDER,
    GENERATOR_RARE_FEATURE_BLOCKS,
    GENERATOR_REGION_ENV_VAR,
    GENERATOR_ROLES,
    GENERATOR_SOURCE_COLUMNS,
    GENERATOR_SQL_RULE_IGNORES,
    GENERATOR_STATUS_VALUES,
    GENERATOR_SUFFIXES,
    GENERATOR_VARIABLE_SHARE,
)
from scripts.compiler_differential.models import DeclarationPlan, GeneratedProject, ModelPlan

_STAGING: str = GENERATOR_LAYERS[0]
_MARTS: str = GENERATOR_LAYERS[1]


class ProjectBuilder:
    """Accumulate the files of one generated project from a seeded random source."""

    def __init__(
        self, seed: int, *, blocks: tuple[str, ...] | None = None, dialect: str | None = None
    ) -> None:
        self._seed: int = seed
        self._forced_blocks: tuple[str, ...] | None = blocks
        self._dialect: str | None = dialect
        self._random: random.Random = random.Random(seed)
        self._files: dict[str, str] = {}
        self._features: set[str] = set()
        self._models: list[ModelPlan] = []
        self._sources: list[str] = []
        self._seeds: list[str] = []
        self._declarations: list[DeclarationPlan] = []
        self._model_lines: dict[str, list[str]] = {}
        self._model_filters: dict[str, list[str]] = {}
        self._private_constants: dict[str, list[str]] = {}
        self._hook_consumers: list[ModelPlan] = []

    def build(self) -> GeneratedProject:
        """Return the finished project, optionally carrying one deliberate invalid input."""

        domains: list[str] = self._random.sample(GENERATOR_DOMAINS, k=self._random.randint(1, 3))
        self._write_sources()
        self._write_seeds()
        self._plan_models(domains=domains)
        self._plan_enums()
        for index in range(self._random.randint(1, 3)):
            self._plan_constant(index=index)
        for index in range(self._random.randint(1, 3)):
            self._plan_macro(index=index)
        blocks: tuple[str, ...] = (
            feature_blocks_for_seed(seed=self._seed, rng=self._random)
            if self._forced_blocks is None
            else self._forced_blocks
        )
        invalid_kind: str | None = (
            self._random.choice(sorted(GENERATOR_INVALID_CODES))
            if self._random.random() < GENERATOR_INVALID_SHARE
            and not set(blocks) & set(GENERATOR_RARE_FEATURE_BLOCKS)
            and self._dialect is None
            else None
        )
        self._write_declarations()
        self._write_models()
        self._write_hooks()
        self._write_python_nodes()
        self._write_tests_and_scenarios()
        self._write_singular_audit()
        extras: DiscoveryFeatureWriter = DiscoveryFeatureWriter(
            blocks=blocks,
            rng=self._random,
            files=self._files,
            features=self._features,
            staging=[model for model in self._models if model.layer == _STAGING],
            label=self._label,
        )
        extras.write()
        rendering: RenderFeatureWriter = RenderFeatureWriter(
            blocks=blocks,
            rng=self._random,
            files=self._files,
            features=self._features,
            staging=[model for model in self._models if model.layer == _STAGING],
            sources=self._sources,
            label=self._label,
        )
        rendering.write()
        analysis: AnalysisFeatureWriter = AnalysisFeatureWriter(
            blocks=blocks,
            rng=self._random,
            files=self._files,
            features=self._features,
            staging=[model for model in self._models if model.layer == _STAGING],
            sources=self._sources,
        )
        analysis.write()
        self._write_config(
            domains=domains,
            adapter=extras.adapter,
            extra_lines=[*extras.config_lines, *rendering.config_lines, *analysis.config_lines],
            settings_lines=analysis.settings_lines,
            templated_target=rendering.templated_target,
        )
        if invalid_kind is not None:
            self._inject_invalid(kind=invalid_kind)
        return GeneratedProject(
            files=dict(sorted(self._files.items())),
            seed=self._seed,
            expected_error_code=(
                GENERATOR_INVALID_CODES[invalid_kind]
                if invalid_kind is not None
                else extras.expected_error_code
            ),
            features=tuple(sorted(self._features)),
            succeeding_commands=extras.succeeding_commands,
            extra_commands=analysis.commands,
        )

    def _target_schema(self, *, schema: str, templated: bool) -> str:
        if not templated:
            return schema
        return f"${{coalesce(ENV:{GENERATOR_MISSING_ENV_VAR}, '{schema}')}}"

    def _chance(self, share: float) -> bool:
        return self._random.random() < share

    def _label(self) -> str:
        return self._random.choice(GENERATOR_NON_ASCII_LABELS)

    def _write_config(
        self,
        *,
        domains: list[str],
        adapter: str | None,
        extra_lines: list[str],
        settings_lines: list[str],
        templated_target: bool,
    ) -> None:
        lines: list[str] = [
            f'name = "generated_{self._seed}"',
            f'adapter = "{self._dialect or adapter or DUCKDB_ADAPTER}"',
            'default_target = "dev"',
            "",
            "[connection]",
            *GENERATOR_DIALECT_CONNECTIONS.get(
                self._dialect or "", ('database = "generated.duckdb"',)
            ),
            "",
            "[vars]",
            f'region = "{self._random.choice(("north", "south", "east"))}"',
            f'discount_rate = "{self._random.choice(GENERATOR_FLOAT_VALUES)}"',
            f'site_label = "{self._label()}"',
            "",
            "[targets.dev]",
            'schema = "{}"'.format(
                self._target_schema(
                    schema=self._random.choice(("analytics", "main")), templated=templated_target
                )
            ),
            *GENERATOR_DIALECT_TARGETS.get(self._dialect or "", ()),
            "",
            "[settings]",
            f'default_audit_severity = "{self._random.choice(("warn", "error"))}"',
            *settings_lines,
            "",
            "[defaults]",
            f'materialized = "{self._random.choice(("table", "view"))}"',
            "",
            '[path_defaults."*/staging"]',
            'materialized = "view"',
            'tags = ["staging"]',
        ]
        for domain in domains:
            if self._chance(GENERATOR_HALF):
                self._features.add("path_defaults_literal")
                lines.extend(
                    [
                        "",
                        f'[path_defaults."{domain}/marts"]',
                        f'materialized = "{self._random.choice(("table", "view"))}"',
                        f'tags = ["{domain}", "marts"]',
                    ]
                )
        self._features.add("path_defaults_wildcard")
        lines.extend(extra_lines)
        lines.extend(GENERATOR_SQL_RULE_IGNORES)
        self._files["sqlbuild_project.toml"] = "\n".join(lines) + "\n"

    def _write_sources(self) -> None:
        entries: list[str] = []
        columns: str = "\n".join(
            f"      - name: {column}\n        type: {column_type}"
            for column, column_type in GENERATOR_SOURCE_COLUMNS
        )
        for index in range(self._random.randint(1, 3)):
            name: str = f"raw_events_{index}"
            self._sources.append(name)
            rows: str = " UNION ALL ".join(
                f"SELECT {row} AS id, CAST({row * 7 + index} AS DOUBLE) AS amount, "
                f"'{GENERATOR_STATUS_VALUES[row % len(GENERATOR_STATUS_VALUES)][1]}' AS status, "
                f"TIMESTAMP '2026-01-0{row + 1} 00:00:00' AS created_at"
                for row in range(1, 4)
            )
            entries.append(
                f"  - name: {name}\n"
                f"    description: Generated event feed {index}.\n"
                f'    expression: "({rows})"\n'
                f"    columns:\n{columns}"
            )
        self._files["sources/raw.yml"] = "sources:\n" + "\n".join(entries) + "\n"

    def _write_seeds(self) -> None:
        schemas: list[str] = []
        for index in range(self._random.randint(0, 2)):
            name: str = f"lookup_{index}"
            self._seeds.append(name)
            self._files[f"seeds/{name}.csv"] = f'id,label\n1,"{self._label()}"\n2,plain\n'
            schemas.append(
                f"  - name: {name}\n"
                f"    description: Generated lookup {index}.\n"
                "    columns:\n"
                "      - name: id\n        type: INTEGER\n"
                "      - name: label\n        type: VARCHAR"
            )
        if schemas:
            self._features.add("seeds")
            self._files["seeds/seeds.yml"] = "seeds:\n" + "\n".join(schemas) + "\n"

    def _plan_models(self, *, domains: list[str]) -> None:
        staging: list[ModelPlan] = []
        for domain in domains:
            staging.extend(
                ModelPlan(
                    name=f"stg_{domain}_{index}",
                    domain=domain,
                    layer=_STAGING,
                    inputs=(f"source:{self._random.choice(self._sources)}",),
                    description=self._description(f"Staged {domain} events {index}"),
                )
                for index in range(self._random.randint(1, 3))
            )
        self._models.extend(staging)
        for domain in domains:
            own: list[ModelPlan] = [model for model in staging if model.domain == domain]
            for index in range(self._random.randint(1, 3)):
                self._models.append(
                    ModelPlan(
                        name=f"{domain}_summary_{index}",
                        domain=domain,
                        layer=_MARTS,
                        inputs=self._mart_inputs(own=own, staging=staging),
                        description=self._description(f"Summary of {domain} activity {index}"),
                    )
                )

    def _mart_inputs(self, *, own: list[ModelPlan], staging: list[ModelPlan]) -> tuple[str, ...]:
        upstream: list[ModelPlan] = [self._random.choice(own)]
        other: ModelPlan = self._random.choice(staging)
        if other not in upstream and self._chance(GENERATOR_CROSS_DOMAIN_SHARE):
            upstream.append(other)
            self._features.add("cross_domain_ref")
        inputs: list[str] = [f"ref:{model.name}" for model in upstream]
        if self._seeds and self._chance(GENERATOR_HALF):
            inputs.append(f"seed:{self._random.choice(self._seeds)}")
        return tuple(inputs)

    def _description(self, text: str) -> str:
        if not self._chance(GENERATOR_NON_ASCII_DESCRIPTION_SHARE):
            return text
        self._features.add("non_ascii_description")
        return f"{text} ({self._label()})"

    def _consumers(self) -> tuple[ModelPlan, ...]:
        count: int = self._random.randint(1, min(3, len(self._models)))
        return tuple(
            sorted(self._random.sample(self._models, k=count), key=lambda model: model.name)
        )

    def _plan_enums(self) -> None:
        status: DeclarationPlan = DeclarationPlan(
            kind=GENERATOR_ENUM_KIND,
            name="event_status",
            consumers=self._consumers(),
            body=(
                "ENUM (\n  name event_status,\n  members (\n"
                + "".join(f'    {member} "{value}",\n' for member, value in GENERATOR_STATUS_VALUES)
                + "  ),\n);\n"
            ),
        )
        self._declarations.append(status)
        for model in status.consumers:
            member: str = self._random.choice(GENERATOR_STATUS_VALUES)[0]
            self._model_filters.setdefault(model.name, []).append(
                f'status <> @enum("event_status").{member}'
            )
        if not self._chance(GENERATOR_HALF):
            return
        self._features.add("integer_enum")
        shared_name: bool = self._chance(GENERATOR_HALF)
        name: str = "adjust_0" if shared_name else "event_priority"
        if shared_name:
            self._features.add("name_shared_across_namespaces")
        priority: DeclarationPlan = DeclarationPlan(
            kind=GENERATOR_ENUM_KIND,
            name=name,
            consumers=self._consumers(),
            body=f"ENUM (\n  name {name},\n  members (LOW 1, HIGH 3),\n);\n",
        )
        self._declarations.append(priority)
        for model in priority.consumers:
            self._model_lines.setdefault(model.name, []).append(f'@enum("{name}").HIGH AS priority')

    def _plan_constant(self, *, index: int) -> None:
        name: str = f"threshold_{index}"
        writers: dict[str, Callable[[str], tuple[str, str, str]]] = {
            "text": self._text_constant,
            "bool": self._bool_constant,
            "decimal": self._decimal_constant,
            "int": self._int_constant,
            "float": self._float_constant,
        }
        parts: tuple[str, str, str] = writers[self._random.choice(sorted(writers))](name)
        body: str = parts[0]
        literal: str = parts[1]
        expression: str = parts[2]
        declaration: DeclarationPlan = DeclarationPlan(
            kind=GENERATOR_CONSTANT_KIND, name=name, consumers=self._consumers(), body=body
        )
        if len(declaration.consumers) == 1 and self._chance(GENERATOR_HALF):
            self._features.add("model_private_value")
            declaration = replace(declaration, name=f"_{name}", private=True)
            expression = expression.replace(f'"{name}"', f'"_{name}"')
            self._private_constants.setdefault(declaration.consumers[0].name, []).append(
                f"    _{name} {literal},\n"
            )
        self._declarations.append(declaration)
        for model in declaration.consumers:
            self._model_lines.setdefault(model.name, []).append(expression)

    def _text_constant(self, name: str) -> tuple[str, str, str]:
        self._features.add("non_ascii_constant")
        literal: str = f'"{self._label()}"'
        return (
            f"CONSTANT (name {name}, value {literal});\n",
            literal,
            f'@const("{name}") AS {name}_label',
        )

    def _bool_constant(self, name: str) -> tuple[str, str, str]:
        literal: str = self._random.choice(("true", "false"))
        return (
            f"CONSTANT (name {name}, value {literal});\n",
            literal,
            f'@const("{name}") AS {name}_flag',
        )

    def _decimal_constant(self, name: str) -> tuple[str, str, str]:
        self._features.add("decimal_constant")
        return (
            f'CONSTANT (\n  name {name},\n  type decimal,\n  value "2.4700",\n);\n',
            "2.47",
            f'amount * @const("{name}") AS {name}_amount',
        )

    def _int_constant(self, name: str) -> tuple[str, str, str]:
        literal: str = str(self._random.randint(-5, 500))
        return (
            f"CONSTANT (name {name}, value {literal});\n",
            literal,
            f'amount + @const("{name}") AS {name}_amount',
        )

    def _float_constant(self, name: str) -> tuple[str, str, str]:
        self._features.add("float_constant")
        literal: str = self._random.choice(GENERATOR_FLOAT_VALUES)
        return (
            f"CONSTANT (name {name}, value {literal});\n",
            literal,
            f'amount * @const("{name}") AS {name}_amount',
        )

    def _plan_macro(self, *, index: int) -> None:
        name: str = f"adjust_{index}"
        writers: dict[str, Callable[[str], tuple[str, str]]] = {
            "ctx": self._context_macro,
            "env": self._environment_macro,
            "nested": self._nested_macro,
            "plain": self._plain_macro,
        }
        parts: tuple[str, str] = writers[self._random.choice(sorted(writers))](name)
        declaration: DeclarationPlan = DeclarationPlan(
            kind=GENERATOR_MACRO_KIND, name=name, consumers=self._consumers(), body=parts[0]
        )
        self._declarations.append(declaration)
        for model in declaration.consumers:
            self._model_lines.setdefault(model.name, []).append(parts[1])

    def _context_macro(self, name: str) -> tuple[str, str]:
        self._features.add("macro_reads_ctx")
        return (
            f"def {name}(ctx, expression: str) -> str:\n"
            '    """Scale an amount by the configured discount."""\n'
            '    rate = ctx.vars["discount_rate"]\n'
            '    return f"({expression} * (1 - {rate}))"\n',
            f'@py.{name}("amount") AS {name}_amount',
        )

    def _environment_macro(self, name: str) -> tuple[str, str]:
        self._features.add("macro_reads_env")
        return (
            "import os\n\n\n"
            f"def {name}(expression: str) -> str:\n"
            '    """Tag an amount with the deployment region."""\n'
            f'    region = os.environ.get("{GENERATOR_REGION_ENV_VAR}", "east")\n'
            "    return f\"({expression} + LENGTH('{region}'))\"\n",
            f'@py.{name}("amount") AS {name}_amount',
        )

    def _nested_macro(self, name: str) -> tuple[str, str]:
        self._features.add("nested_macro")
        return (
            f"def {name}_round(expression: str) -> str:\n"
            '    """Round an amount to two places."""\n'
            '    return f"ROUND({expression}, 2)"\n\n\n'
            f"def {name}(expression: str) -> str:\n"
            '    """Double and round an amount."""\n'
            f'    return {name}_round(f"({{expression}} * 2)")\n',
            f'@py.{name}_round(@py.{name}("amount")) AS {name}_amount',
        )

    def _plain_macro(self, name: str) -> tuple[str, str]:
        offset: str = name.rsplit("_", 1)[1]
        return (
            f"def {name}(expression: str) -> str:\n"
            '    """Offset an amount."""\n'
            f'    return f"({{expression}} + {offset} + 1)"\n',
            f'@py.{name}("amount") AS {name}_amount',
        )

    def _write_declarations(self) -> None:
        for declaration in self._declarations:
            if declaration.private:
                continue
            directory: str = self._placement(declaration)
            if declaration.kind == GENERATOR_MACRO_KIND:
                directory = f"{directory}/{GENERATOR_PYTHON_MACRO_FOLDER}"
            suffix: str = GENERATOR_SUFFIXES[declaration.kind]
            self._files[f"{directory}/{declaration.name}{suffix}"] = declaration.body

    def _placement(self, declaration: DeclarationPlan) -> str:
        role: str = GENERATOR_ROLES[declaration.kind]
        folders: set[str] = {model.folder for model in declaration.consumers}
        domains: set[str] = {model.domain for model in declaration.consumers}
        if len(folders) == 1:
            self._features.add("exact_owner_scope")
            return f"{folders.pop()}/_sqlbuild/_{role}"
        if len(domains) == 1:
            self._features.add("tree_scope")
            return f"models/{domains.pop()}/_sqlbuild/{role}"
        self._features.add("project_scope")
        return role

    def _write_models(self) -> None:
        for model in self._models:
            self._files[f"{model.folder}/{model.name}.sql"] = self._model_sql(model)

    def _model_sql(self, model: ModelPlan) -> str:
        header: list[str] = [f'  description "{model.description}"']
        if self._chance(GENERATOR_HALF):
            header.append("  columns (\n    id (audits [not_null]),\n  )")
            self._features.add("column_audits")
        private: list[str] = self._private_constants.get(model.name, [])
        if private:
            header.append("  constants (\n" + "".join(private) + "  )")
        header.extend(self._model_hooks(model))
        columns: list[str] = ["t.id", "t.amount", "t.status", "t.created_at"]
        columns.extend(
            line.replace("amount", "t.amount", 1) for line in self._model_lines.get(model.name, [])
        )
        joins: list[str] = []
        for position, extra in enumerate(model.inputs[1:], start=1):
            joins.append(f"LEFT JOIN {_relation(extra)} j{position} ON t.id = j{position}.id")
            columns.extend(
                f"j{position}.label AS lookup_label_{position}"
                for _ in range(int(extra.startswith("seed:")))
            )
        if self._chance(GENERATOR_VARIABLE_SHARE):
            self._features.add("project_variable")
            columns.append("'@@site_label' AS site_label")
        filters: list[str] = [
            condition.replace("status", "t.status", 1)
            for condition in self._model_filters.get(model.name, [])
        ]
        where: str = f"\nWHERE {' AND '.join(filters)}" if filters else ""
        return (
            "MODEL (\n"
            + ",\n".join(header)
            + ",\n);\n\nSELECT\n  "
            + ",\n  ".join(columns)
            + f"\nFROM {_relation(model.inputs[0])} t"
            + "".join(f"\n{join}" for join in joins)
            + where
            + "\n"
        )

    def _model_hooks(self, model: ModelPlan) -> list[str]:
        if model.layer != _MARTS or not self._chance(GENERATOR_HALF):
            return []
        self._features.add("sql_hooks")
        self._hook_consumers.append(model)
        return [
            "  post_hooks [\n"
            '    sql("record_refresh", relation: "@@CTX:destination.qualified", '
            f'label: "{self._label()}"),\n'
            "    inline_sql(\"SELECT '@@CTX:destination.qualified' AS refreshed\"),\n"
            "  ]"
        ]

    def _write_hooks(self) -> None:
        if not self._hook_consumers:
            return
        hook: DeclarationPlan = DeclarationPlan(
            kind=GENERATOR_HOOK_KIND, name="record_refresh", consumers=tuple(self._hook_consumers)
        )
        self._files[f"{self._placement(hook)}/sql/record_refresh.sql"] = (
            'HOOK (\n  description "Record a refresh for one relation"\n);\n\n'
            "SELECT @'relation' AS refreshed_relation, @'label' AS refresh_label\n"
        )

    def _write_python_nodes(self) -> None:
        if not self._chance(GENERATOR_HALF):
            return
        self._features.add("python_nodes")
        self._files["python/tasks/exports.py"] = (
            "from sqlbuild.tasks import TaskContext, task\n\n\n"
            "@task\n"
            "def collect_exports(ctx: TaskContext):\n"
            '    """Collect generated exports."""\n'
            '    return ctx.result(payload={"rows": 3})\n\n\n'
            "@task(depends_on=collect_exports)\n"
            "def publish_exports(ctx: TaskContext):\n"
            '    """Publish generated exports."""\n'
            '    return ctx.result(metadata={"published": True})\n'
        )

    def _write_tests_and_scenarios(self) -> None:
        model: ModelPlan = self._random.choice(
            [model for model in self._models if model.layer == _STAGING]
        )
        source: str = model.inputs[0].split(":", 1)[1]
        uses_enum: bool = model.name in self._model_filters
        if uses_enum:
            self._features.add("relationship_grant")
        expected_status: str = '@enum("event_status").PLACED' if uses_enum else "'placed'"
        self._features.add("unit_test")
        self._files[f"tests/unit/{model.domain}/test_{model.name}.sql"] = (
            "TEST();\n\nWITH\n"
            f"__source__{source} AS (\n"
            "  SELECT 1 AS id, CAST(10 AS DOUBLE) AS amount, 'placed' AS status,\n"
            "    TIMESTAMP '2026-01-01 00:00:00' AS created_at\n"
            "),\n"
            f"__expected__{model.name} AS (\n"
            f"  SELECT 1 AS id, {expected_status} AS status\n"
            ")\n"
            "SELECT 1\n"
        )
        if not self._chance(GENERATOR_HALF):
            return
        self._features.add("scenario")
        self._files[f"tests/scenarios/{model.name}_scenario.sql"] = (
            'SCENARIO (\n  description "Generated scenario for staged events"\n);\n\n'
            "WITH\n"
            f"__source__{source} AS (\n"
            "  SELECT 2 AS id, CAST(5 AS DOUBLE) AS amount, 'placed' AS status,\n"
            "    TIMESTAMP '2026-01-02 00:00:00' AS created_at\n"
            "),\n"
            f"__expected__{model.name} AS (\n"
            "  SELECT 2 AS id\n"
            ")\n"
            "SELECT 1\n"
        )

    def _write_singular_audit(self) -> None:
        by_folder: dict[str, list[ModelPlan]] = {}
        for model in self._models:
            if model.layer == _MARTS:
                by_folder.setdefault(model.folder, []).append(model)
        candidates: list[list[ModelPlan]] = [
            group for group in by_folder.values() if len(group) > 1
        ]
        if not candidates or not self._chance(GENERATOR_HALF):
            return
        self._features.add("singular_audit")
        first, second = self._random.sample(self._random.choice(candidates), k=2)
        self._files[f"{first.folder}/_sqlbuild/audits/singular/{first.name}_matches.sql"] = (
            f'AUDIT (name "{first.name}_matches");\n\n'
            f'SELECT a.id\nFROM __ref("{first.name}") a\n'
            f'LEFT JOIN __ref("{second.name}") b ON a.id = b.id\nWHERE b.id IS NULL\n'
        )

    def _inject_invalid(self, *, kind: str) -> None:
        self._features.add(f"invalid:{kind}")
        model: ModelPlan = self._random.choice(self._models)
        path: str = f"{model.folder}/{model.name}.sql"
        injections: dict[str, Callable[[ModelPlan], str]] = {
            "unknown_ref": lambda _: '(SELECT 1 FROM __ref("missing_model")) AS missing',
            "unknown_macro": lambda _: '@py.not_a_macro("id") AS bogus',
            "failing_macro": self._failing_macro_column,
            "unknown_enum_member": lambda _: '@enum("event_status").MISSING AS bogus',
            "unknown_constant": lambda _: '@const("no_such_value") AS bogus',
            "missing_description": self._drop_description,
            "duplicate_macro": self._duplicate_macro,
            "header_syntax": self._break_header,
            "over_broad_constant": self._over_broad_constant_column,
        }
        column: str = injections[kind](model)
        if column:
            self._files[path] = self._files[path].replace("SELECT\n", f"SELECT\n  {column},\n", 1)

    def _failing_macro_column(self, model: ModelPlan) -> str:
        self._files[f"{model.folder}/_sqlbuild/_macros/python/explode.py"] = (
            "def explode(expression: str) -> str:\n"
            '    """Refuse every input."""\n'
            '    raise ValueError(f"cannot adjust {expression}")\n'
        )
        return '@py.explode("id") AS bogus'

    def _drop_description(self, model: ModelPlan) -> str:
        path: str = f"{model.folder}/{model.name}.sql"
        self._files[path] = self._files[path].replace(f'  description "{model.description}",\n', "")
        self._files[path] = self._files[path].replace(f'  description "{model.description}"\n', "")
        return ""

    def _duplicate_macro(self, model: ModelPlan) -> str:
        _ = model
        self._files["macros/python/duplicate_adjust_0.py"] = (
            "def adjust_0(expression: str) -> str:\n"
            '    """Duplicate a macro name."""\n'
            "    return expression\n"
        )
        return ""

    def _break_header(self, model: ModelPlan) -> str:
        path: str = f"{model.folder}/{model.name}.sql"
        self._files[path] = self._files[path].replace(
            "MODEL (\n", "MODEL (\n  materialized (,\n", 1
        )
        return ""

    def _over_broad_constant_column(self, model: ModelPlan) -> str:
        _ = model
        self._files["constants/unused_threshold.sql"] = (
            "CONSTANT (name unused_threshold, value 3);\n"
        )
        return '@const("unused_threshold") AS narrow'


def _relation(reference: str) -> str:
    kind, name = reference.split(":", 1)
    return f'__{kind}("{name}")'
