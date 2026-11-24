"""Test helpers for native rules engine boundaries."""

import json
from dataclasses import dataclass, field
from operator import attrgetter
from pathlib import Path
from typing import Any, cast

import sqlbuild._native as native
from sqlbuild._native import (
    MIN_CUSTOM_RULE_TEST_CASES,
    CompiledDirectLogicSqlTestPayload,
    CompiledModelSqlTestPayload,
    CompiledObjectKey,
    CompiledResourceType,
    CompiledSqlTest,
    CompiledSqlTestResource,
    CompileSqlTestCte,
    CustomRulesOutcome,
    DiscoveredSqlTestBlock,
    DiscoveredSqlTestFile,
    Finding,
    NativeRulesRequest,
    Rule,
    RuleExemption,
    RuleIgnore,
    RulesCacheConfig,
    RulesConfig,
    RulesResult,
    ScopeIndex,
    build_rule_catalogue,
    install_rules_skills,
    project_with,
)
from sqlbuild._native import evaluate_custom_rules as evaluate_custom_rules_cached
from sqlbuild._native import evaluate_project_rules as evaluate
from sqlbuild.compiler.compile.types import CompiledProjectView, SqlTestMode
from tests.unit.src.sqlbuild.rule_engine._helpers.engine._test_types import CustomRuleTestCase
from tests.unit.src.sqlbuild.rule_engine.main.evaluate.helpers import build_project

RULES_SKILL_PATHS: tuple[str, ...] = (
    ".agents/skills/sqlbuild-rules/SKILL.md",
    ".claude/skills/sqlbuild-rules/SKILL.md",
    ".opencode/skills/sqlbuild-rules/SKILL.md",
)

_MODEL_SQL: str = "WITH final AS (SELECT 1 AS id) SELECT id FROM final"


def project_with_scope(*, index: ScopeIndex) -> CompiledProjectView:
    """Return a minimal compiled project carrying canonical scope facts."""

    project: CompiledProjectView = build_project(
        name="orders",
        relative_path="sql/models/orders.sql",
        sql="SELECT 1 AS id",
        config_values={},
    )
    return project_with(project=project, scope_index=index)


def present_metadata(scope_index: dict[str, Any]) -> dict[str, Any]:
    """`scope_index` with each declaration's absent metadata kinds dropped, as projected."""

    declarations: list[dict[str, Any]] = [
        {**declaration, "metadata": _present(declaration["metadata"])}
        for declaration in scope_index["declarations"]
    ]
    return {**scope_index, "declarations": declarations}


def _present(metadata: dict[str, Any]) -> dict[str, Any]:
    return dict(filter(lambda item: item[1] is not None, metadata.items()))


def captured_native_request(*, project: CompiledProjectView, project_dir: Path) -> dict[str, Any]:
    """Build the native rules request for `project` and return the facts it carries."""

    request: NativeRulesRequest = native.build_rules_request(
        project=project,
        config=RulesConfig(cache=RulesCacheConfig(enabled=False)),
        project_dir=project_dir,
        dialect="generic",
        initial_findings=(),
        custom_payloads=[],
        include_type_proof=False,
    )
    return cast(dict[str, Any], json.loads(request.sql_facts_json()))


def direct_sql_test(*, mode: SqlTestMode, name: str, block_index: int) -> CompiledSqlTest:
    test_file: DiscoveredSqlTestFile = DiscoveredSqlTestFile(
        file_path=Path(f"/private/project/tests/unit/{name}.sql"),
        relative_path=Path(f"tests/unit/{name}.sql"),
        contents="secret fixture value",
        blocks=(),
    )
    test_block: DiscoveredSqlTestBlock = DiscoveredSqlTestBlock(
        test_index=block_index,
        header_values={"name": name},
        sql_body="SELECT 'secret fixture value'",
        name=name,
        mode=mode,
    )
    return CompiledSqlTest(
        key=CompiledObjectKey(CompiledResourceType.SQL_TEST, name),
        scope_deps=(CompiledObjectKey(CompiledResourceType.MODEL, "orders"),),
        name=name,
        test_file=test_file,
        test_block=test_block,
        sql_body=test_block.sql_body,
        mode=mode,
        payload=CompiledDirectLogicSqlTestPayload(
            actual_cte=CompileSqlTestCte("__actual", "SELECT 1"),
            expected_cte=CompileSqlTestCte("__expected", "SELECT 1"),
            mode=mode,
            tested_resource_names=(name,),
        ),
        tested_resources=(CompiledSqlTestResource(kind=mode, name=name),),
    )


def model_sql_test() -> CompiledSqlTest:
    name: str = "orders: keeps paid orders"
    test_file: DiscoveredSqlTestFile = DiscoveredSqlTestFile(
        file_path=Path("/private/project/tests/unit/test_orders__keeps_paid.sql"),
        relative_path=Path("tests/unit/test_orders__keeps_paid.sql"),
        contents="secret model fixture",
        blocks=(),
    )
    test_block: DiscoveredSqlTestBlock = DiscoveredSqlTestBlock(
        test_index=1,
        header_values={"name": name},
        sql_body="SELECT 1",
        name=name,
        mode=SqlTestMode.MODEL,
    )
    return CompiledSqlTest(
        key=CompiledObjectKey(CompiledResourceType.SQL_TEST, name),
        scope_deps=(),
        name=name,
        test_file=test_file,
        test_block=test_block,
        sql_body=test_block.sql_body,
        mode=SqlTestMode.MODEL,
        payload=CompiledModelSqlTestPayload(),
        expected_model_names=("orders",),
        assertion_names=("paid",),
        assertion_target_model_names=("orders",),
        target_model_names=("orders",),
    )


def write_rule(
    *,
    root: Path,
    body: str,
    enabled_by_default: bool = False,
    project_wide: bool = False,
    filename: str = "custom.py",
    code: str = "XSQBRT101",
    check_name: str = "check",
    module_import: str = "",
) -> Path:
    path: Path = root / "rules" / filename
    path.parent.mkdir(parents=True, exist_ok=True)
    subject_signature: str = {
        False: f"def {check_name}(*, model: Model, ctx: RuleContext) -> list[Finding]:",
        True: f"def {check_name}(*, project: Project, ctx: RuleContext) -> list[Finding]:",
    }[project_wide]
    resolved_body: str = {
        False: body,
        True: body.replace("model", "project"),
    }[project_wide]
    path.write_text(
        "\n".join(
            (
                "from sqlbuild.rules import Finding, Model, Project, RuleContext, rule",
                module_import,
                "",
                "@rule(",
                f'    code="{code}",',
                '    slug="test-rule",',
                '    message="test rule fault",',
                '    remediation="Fix this model at its sql/models/<domain>/ path.",',
                f"    enabled_by_default={enabled_by_default},",
                ")",
                subject_signature,
                f"    {resolved_body}",
                "",
            )
        ),
        encoding="utf-8",
    )
    return path.relative_to(root)


def write_project_file_rule(
    *, root: Path, file_path: str, contents: str, body: str, module_import: str
) -> Path:
    """Write one project file plus a custom rule and return the absolute file path."""

    path: Path = root / file_path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents, encoding="utf-8")
    _ = write_rule(root=root, body=body, module_import=module_import)
    return path


def load_custom_rule(*, root: Path, configured_path: Path) -> Rule:
    del configured_path
    config: RulesConfig = RulesConfig()
    catalogue: tuple[Rule, ...] = build_rule_catalogue(
        config=config, project_dir=root, include_custom=True
    )
    custom_rules: tuple[Rule, ...] = tuple(filter(attrgetter("custom"), catalogue))
    return custom_rules[0]


def custom_rule_inputs(
    *, tmp_path: Path, test_case: CustomRuleTestCase
) -> tuple[CompiledProjectView, RulesConfig]:
    _ = write_rule(
        root=tmp_path,
        body=test_case.body,
        enabled_by_default=test_case.enabled_by_default,
        project_wide=test_case.project_wide,
    )
    (tmp_path / "rules" / "input.json").write_text("{}", encoding="utf-8")
    project: CompiledProjectView = build_project(
        name="commerce__mart__orders",
        relative_path="sql/models/mart/commerce__mart__orders.sql",
        sql=_MODEL_SQL,
        config_values={},
    )
    config: RulesConfig = RulesConfig(
        select=test_case.select,
        thresholds={MIN_CUSTOM_RULE_TEST_CASES: test_case.minimum_custom_rule_cases},
        cache=RulesCacheConfig(enabled=True),
    )
    return project, config


def custom_rules_with_imports(
    *, project_dir: Path, module_import: str, extra_files: tuple[tuple[str, str], ...]
) -> tuple[Rule, ...]:
    """Write one custom rule with an import line plus extra files and load its catalogue."""

    _ = write_rule(
        root=project_dir, body="del model, ctx\n    return []", module_import=module_import
    )
    for relative_path, source in extra_files:
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(source, encoding="utf-8")
    catalogue: tuple[Rule, ...] = build_rule_catalogue(
        config=RulesConfig(select=("XSQBRT101",)), project_dir=project_dir, include_custom=True
    )
    return tuple(filter(lambda rule: rule.custom, catalogue))


def two_model_project(
    *, customers_sql: str, customers_config: dict[str, object]
) -> CompiledProjectView:
    """Return an orders and customers project with configurable customers facts."""

    orders: CompiledProjectView = build_project(
        name="orders",
        relative_path="sql/models/orders.sql",
        sql="SELECT 1 AS order_id",
        config_values={},
    )
    customers: CompiledProjectView = build_project(
        name="customers",
        relative_path="sql/models/customers.sql",
        sql=customers_sql,
        config_values=customers_config,
    )
    return project_with(project=orders, models=(*orders.models, *customers.models))


def evaluate_cached_custom_rules(
    *, project: CompiledProjectView, project_dir: Path, cache_enabled: bool
) -> CustomRulesOutcome:
    """Evaluate the selected custom rule through the incremental read-tracking cache."""

    config: RulesConfig = RulesConfig(
        select=("XSQBRT101",), cache=RulesCacheConfig(enabled=cache_enabled)
    )
    rules: tuple[Rule, ...] = tuple(
        filter(
            attrgetter("custom"),
            build_rule_catalogue(config=config, project_dir=project_dir, include_custom=True),
        )
    )
    return evaluate_custom_rules_cached(
        project=project, config=config, project_dir=project_dir, rules=rules, dialect="duckdb"
    )


def evaluate_contract_rule(
    *, config_values: dict[str, object], project_dir: Path, cache_enabled: bool
) -> RulesResult:
    """Evaluate one built-in contract rule over a single mart model."""

    config: RulesConfig = RulesConfig(
        select=("SQBRCONTRACT101",), cache=RulesCacheConfig(enabled=cache_enabled)
    )
    project: CompiledProjectView = build_project(
        name="commerce__mart__orders",
        relative_path="sql/models/mart/commerce__mart__orders.sql",
        sql="WITH orders AS (SELECT id FROM source_orders) SELECT id FROM orders",
        config_values=config_values,
    )
    return evaluate(project=project, config=config, project_dir=project_dir)


@dataclass(frozen=True)
class IncrementalModelSpec:
    """One synthetic compiled model in an incremental built-in rules step."""

    name: str
    relative_path: str
    sql: str
    config_values: dict[str, object] = field(default_factory=dict)


@dataclass(frozen=True)
class IncrementalRulesStep:
    """One project and configuration state evaluated after the previous step."""

    description: str
    models: tuple[IncrementalModelSpec, ...]
    select: tuple[str, ...] = ("SQBR",)
    ignore: tuple[str, ...] = ()
    thresholds: dict[str, int] = field(default_factory=dict)
    rule_ignores: tuple[RuleIgnore, ...] = ()
    rule_exceptions: tuple[RuleExemption, ...] = ()
    dialect: str = "duckdb"


def evaluate_incremental_step(
    *,
    step: IncrementalRulesStep,
    project_dir: Path,
    cache_enabled: bool,
    defer_suppressions: bool,
    initial_codes: tuple[str, ...] = (),
) -> RulesResult:
    """Write the step's model files and evaluate built-in rules natively over its models."""

    for model in step.models:
        path: Path = project_dir / model.relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(model.sql, encoding="utf-8")
    config: RulesConfig = RulesConfig(
        select=step.select,
        ignore=step.ignore,
        thresholds=step.thresholds,
        rule_ignores=step.rule_ignores,
        rule_exceptions=step.rule_exceptions,
        cache=RulesCacheConfig(enabled=cache_enabled),
    )
    return native.evaluate_native_rules(
        project=_incremental_project(step.models),
        config=config,
        project_dir=project_dir,
        catalogue=build_rule_catalogue(
            config=config, project_dir=project_dir, include_custom=False
        ),
        dialect=step.dialect,
        initial_findings=tuple(
            Finding(
                code=code,
                path=Path(step.models[0].relative_path),
                line=1,
                column=1,
                message=f"{code} reported before built-in rules",
                remediation="Review the authored SQL.",
            )
            for code in initial_codes
        ),
        defer_suppressions=defer_suppressions,
    )


def _incremental_project(models: tuple[IncrementalModelSpec, ...]) -> CompiledProjectView:
    projects: list[CompiledProjectView] = [
        build_project(
            name=model.name,
            relative_path=model.relative_path,
            sql=model.sql,
            config_values=model.config_values,
        )
        for model in models
    ]
    return project_with(
        project=projects[0], models=tuple(project.models[0] for project in projects)
    )


def sql_test_summary(fact: dict[str, Any]) -> tuple[object, ...]:
    """The SQL test fact fields the native rules request carries, in a compact comparable form."""

    return (
        fact["source_path"],
        fact["ownership_root"],
        fact["block_index"],
        fact["name"],
        fact["explicit_name"],
        fact["mode"],
        (
            fact["expected_model_names"],
            fact["assertion_names"],
            fact["assertion_target_model_names"],
            fact["target_model_names"],
        ),
        [(resource["kind"], resource["name"]) for resource in fact["tested_resources"]],
        (
            fact["authored_ctes"],
            fact["expected_ctes"],
            fact["assertion_ctes"],
            fact["has_macro_mocks"],
            fact["has_model_query_overrides"],
        ),
        fact["case_name"],
    )


def scenario_summary(fact: dict[str, Any]) -> tuple[object, ...]:
    """The SQL scenario fact fields the native rules request carries, in field order."""

    return (
        fact["source_path"],
        fact["ownership_root"],
        fact["name"],
        fact["description"],
        fact["expected_model_names"],
        fact["assertion_names"],
        fact["assertion_target_model_names"],
        fact["target_model_names"],
    )


def installed_skill(*, config: RulesConfig, project_dir: Path) -> str:
    """Install the generated rules skill and return the first target's content."""

    _ = install_rules_skills(config=config, project_dir=project_dir, check=False)
    return (project_dir / RULES_SKILL_PATHS[0]).read_text(encoding="utf-8")
