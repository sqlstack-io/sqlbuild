from __future__ import annotations

from pathlib import Path

import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import (
    CliUserError,
    PlaygroundCommandRequest,
    SkillMaintenanceResult,
    maintain_sqlbuild_skills,
    run_playground_command,
)
from tests.unit.src.sqlbuild.cli.commands.main.playground._test_types import (
    CreatePlaygroundProjectTestCase,
    RunPlaygroundTestCase,
    WrapperPlaygroundSkillsTestCase,
)
from tests.unit.src.sqlbuild.presentation.helpers import use_color_terminal


@pytest.mark.parametrize(
    "test_case",
    [
        CreatePlaygroundProjectTestCase(
            description="creates clean waffle shop playground from packaged template",
            target_relative_path=Path("waffle_shop_playground"),
            expected_files=(
                Path("README.md"),
                Path("sqlbuild_project.toml"),
                Path("models/staging/stg_orders.sql"),
                Path("models/marts/fact_orders.sql"),
                Path("seeds/waffle_types.csv"),
                Path("sources/raw.yml"),
                Path("tests/unit/test_daily_revenue_chain.sql"),
                Path("models/marts/_sqlbuild/_audits/generic/expression_is_true.sql"),
                Path("functions/sql/udf__is_completed_order.sql"),
                Path("functions/sql/table_fn__customer_orders.sql"),
                Path("python/loaders/waffle_sources.py"),
                Path("macros/python/currency.py"),
                Path("models/marts/_sqlbuild/_macros/python/currency.py"),
                Path("models/marts/_sqlbuild/_macros/python/datetime.py"),
                Path("materializations/partition_tracked.py"),
            ),
            unexpected_paths=(
                Path("target"),
                Path("sqlbuild_local.toml"),
                Path("waffle_shop_control.duckdb"),
                Path("macros/__pycache__"),
                Path("models/marts/_macros"),
            ),
            expected_file_fragments=(
                (
                    Path("sqlbuild_project.toml"),
                    ("[connections.developer]", 'connection = "developer"'),
                ),
                (
                    Path("materializations/partition_tracked.py"),
                    ("from sqlbuild.adapter.contract.types import FrameworkType",),
                ),
            ),
        ),
        CreatePlaygroundProjectTestCase(
            description="creates Dagster playground from waffle shop template",
            target_relative_path=Path("dagster_playground"),
            template="dagster",
            expected_files=(
                Path("README.md"),
                Path("definitions.py"),
                Path("waffle_shop/README.md"),
                Path("waffle_shop/sqlbuild_project.toml"),
                Path("waffle_shop/models/marts/fact_orders.sql"),
                Path("waffle_shop/sources/raw.yml"),
                Path("waffle_shop/python/loaders/waffle_sources.py"),
            ),
            unexpected_paths=(
                Path("target"),
                Path("sqlbuild_project.toml"),
                Path("dagster"),
                Path("waffle_shop/dagster"),
                Path("waffle_shop/definitions.py"),
            ),
            expected_file_fragments=(
                (Path("waffle_shop/sqlbuild_project.toml"), ('default_target = "dev"',)),
                (
                    Path("waffle_shop/sources/raw.yml"),
                    ("managed: true",),
                ),
                (
                    Path("waffle_shop/python/loaders/waffle_sources.py"),
                    ("def raw__customers(ctx", "def raw__orders(ctx"),
                ),
                (
                    Path("definitions.py"),
                    ('PROJECT_DIR = Path(__file__).resolve().parent / "waffle_shop"',),
                ),
                (Path("README.md"), ("dagster dev -f definitions.py",)),
            ),
        ),
        CreatePlaygroundProjectTestCase(
            description="creates Rivers playground from waffle shop template",
            target_relative_path=Path("rivers_playground"),
            template="rivers",
            expected_files=(
                Path("README.md"),
                Path("definitions.py"),
                Path("waffle_shop/sqlbuild_project.toml"),
                Path("waffle_shop/models/marts/fact_orders.sql"),
                Path("waffle_shop/sources/raw.yml"),
                Path("waffle_shop/python/loaders/waffle_sources.py"),
            ),
            unexpected_paths=(
                Path("target"),
                Path("sqlbuild_project.toml"),
                Path("rivers_pipeline"),
                Path("waffle_shop/rivers_pipeline"),
                Path("waffle_shop/definitions.py"),
            ),
            expected_file_fragments=(
                (
                    Path("definitions.py"),
                    (
                        'PROJECT_DIR = Path(__file__).resolve().parent / "waffle_shop"',
                        "import rivers as rs",
                        "from sqlbuild.integrations.rivers import",
                        "@sqlbuild_assets(project=SQLBUILD_PROJECT)",
                        "repo = rs.CodeRepository",
                        'rs.Job(\n            name="waffle_shop"',
                    ),
                ),
                (
                    Path("README.md"),
                    ("rivers dev definitions", "`waffle_shop` job"),
                ),
            ),
        ),
        CreatePlaygroundProjectTestCase(
            description="creates loader-focused waffle shop playground from packaged template",
            target_relative_path=Path("loader_waffle_shop_playground"),
            template="loader_waffle_shop",
            expected_files=(
                Path("README.md"),
                Path("sqlbuild_project.toml"),
                Path("models/fact_waffle_orders.sql"),
                Path("models/customer_revenue.sql"),
                Path("sources/raw.yml"),
                Path("python/loaders/waffle_loaders.py"),
            ),
            unexpected_paths=(Path("target"), Path("sqlbuild_local.toml")),
            expected_file_fragments=(
                (
                    Path("sqlbuild_project.toml"),
                    ("[connections.developer]", 'connection = "developer"'),
                ),
                (Path("sources/raw.yml"), ("managed: true",)),
                (
                    Path("python/loaders/waffle_loaders.py"),
                    ("def raw_orders(ctx):", "def raw_customers(ctx):"),
                ),
            ),
            unexpected_file_fragments=(
                (Path("sources/raw.yml"), ("loader:",)),
                (
                    Path("python/loaders/waffle_loaders.py"),
                    ("def load_raw_orders(ctx):", "def load_raw_customers(ctx):"),
                ),
            ),
        ),
        CreatePlaygroundProjectTestCase(
            description="creates Python nodes playground from generated template",
            target_relative_path=Path("python_nodes_playground"),
            template="python_nodes",
            expected_files=(
                Path("README.md"),
                Path("sqlbuild_project.toml"),
                Path("sources/raw.yml"),
                Path("models/fact_orders.sql"),
                Path("python/assets/orders_inputs.py"),
                Path("python/loaders/orders.py"),
                Path("python/assets/orders_export.py"),
                Path("python/checks/orders_export.py"),
            ),
            unexpected_paths=(Path("target"), Path("sqlbuild_local.toml")),
            expected_file_fragments=(
                (
                    Path("sqlbuild_project.toml"),
                    ("[connections.developer]", 'connection = "developer"'),
                ),
                (
                    Path("python/assets/orders_inputs.py"),
                    (
                        "from sqlbuild.assets import SkipMode, asset",
                        "def prepare_raw_orders",
                        "SkipMode.SOFT",
                        "def export_window",
                    ),
                ),
                (
                    Path("python/loaders/orders.py"),
                    ("@loader(depends_on=(prepare_raw_orders,))", "def raw_orders"),
                ),
                (
                    Path("python/assets/orders_export.py"),
                    (
                        'ctx.relation(model("fact_orders"))',
                        "materialized=False",
                        "optional_partner_feed",
                    ),
                ),
                (
                    Path("python/checks/orders_export.py"),
                    (
                        "@check(depends_on=orders_export",
                        "ctx.result_of(node_function=orders_export)",
                    ),
                ),
            ),
            unexpected_file_fragments=(
                (Path("python/assets/orders_inputs.py"), ("sqlbuild.compiler",)),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_target_when_creating_playground_then_it_copies_clean_template(
    test_case: CreatePlaygroundProjectTestCase,
    tmp_path: Path,
) -> None:
    target_dir: Path = tmp_path / test_case.target_relative_path

    _ = run_playground_command(
        PlaygroundCommandRequest(
            project_dir=tmp_path,
            target_path=str(test_case.target_relative_path),
            template=test_case.template,
        )
    )

    expected_file: Path
    for expected_file in test_case.expected_files:
        assert (target_dir / expected_file).is_file()
    unexpected_path: Path
    for unexpected_path in test_case.unexpected_paths:
        assert not (target_dir / unexpected_path).exists()
    expected_file: Path
    expected_fragments: tuple[str, ...]
    for expected_file, expected_fragments in test_case.expected_file_fragments:
        file_text: str = (target_dir / expected_file).read_text(encoding="utf-8")
        expected_fragment: str
        for expected_fragment in expected_fragments:
            assert expected_fragment in file_text
    unexpected_file: Path
    unexpected_fragments: tuple[str, ...]
    for unexpected_file, unexpected_fragments in test_case.unexpected_file_fragments:
        file_text = (target_dir / unexpected_file).read_text(encoding="utf-8")
        unexpected_fragment: str
        for unexpected_fragment in unexpected_fragments:
            assert unexpected_fragment not in file_text


@pytest.mark.parametrize(
    "test_case",
    [
        CreatePlaygroundProjectTestCase(
            description="raises when playground target already exists",
            target_relative_path=Path("existing"),
            expected_files=(),
            unexpected_paths=(),
            expected_error_fragment="playground target already exists",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_existing_target_when_creating_playground_then_it_raises_user_error(
    test_case: CreatePlaygroundProjectTestCase,
    tmp_path: Path,
) -> None:
    target_dir: Path = tmp_path / test_case.target_relative_path
    target_dir.mkdir()

    with pytest.raises(CliUserError) as exc_info:
        _ = run_playground_command(
            PlaygroundCommandRequest(
                project_dir=tmp_path, target_path=str(test_case.target_relative_path)
            )
        )

    assert test_case.expected_error_fragment in str(exc_info.value)


@pytest.mark.parametrize(
    "test_case",
    [
        RunPlaygroundTestCase(
            description="prints next steps after creating playground",
            target_path="demo_shop",
            expected_stdout_fragments=(
                "SQLBuild playground created",
                "Project: demo_shop",
                "Adapter: DuckDB",
                "sqb compile",
                "sqb build",
            ),
        ),
        RunPlaygroundTestCase(
            description="prints Dagster next steps after creating Dagster playground",
            target_path="demo_dagster_shop",
            template="dagster",
            expected_stdout_fragments=(
                "SQLBuild playground created",
                "Project: demo_dagster_shop",
                "Example: waffle shop + Dagster",
                "cd demo_dagster_shop/waffle_shop",
                "cd ..",
                "dagster dev -f definitions.py",
            ),
            project_subdir="waffle_shop",
        ),
        RunPlaygroundTestCase(
            description="prints Rivers next steps after creating Rivers playground",
            target_path="demo_rivers_shop",
            template="rivers",
            expected_stdout_fragments=(
                "SQLBuild playground created",
                "Project: demo_rivers_shop",
                "Example: waffle shop + Rivers",
                "cd demo_rivers_shop/waffle_shop",
                "cd ..",
                "rivers dev definitions",
            ),
            project_subdir="waffle_shop",
        ),
        RunPlaygroundTestCase(
            description="prints loader-focused waffle shop next steps",
            target_path="demo_loader_shop",
            template="loader_waffle_shop",
            expected_stdout_fragments=(
                "SQLBuild playground created",
                "Project: demo_loader_shop",
                "Example: loader-focused waffle shop",
                "sqb build",
            ),
        ),
        RunPlaygroundTestCase(
            description="prints Python nodes next steps",
            target_path="demo_python_nodes",
            template="python_nodes",
            expected_stdout_fragments=(
                "SQLBuild playground created",
                "Project: demo_python_nodes",
                "Example: Python nodes demo",
                "sqb plan --select +fact_orders --select +orders_export",
                "sqb build --select +fact_orders --select +orders_export",
                "sqb check --select check_orders_export",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_playground_command_when_running_then_it_prints_next_steps(
    test_case: RunPlaygroundTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    exit_code: int = run_playground_command(
        PlaygroundCommandRequest(
            project_dir=tmp_path,
            target_path=test_case.target_path,
            template=test_case.template,
        )
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == 0
    expected_fragment: str
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in captured.out
    project_dir: Path = tmp_path / test_case.target_path / test_case.project_subdir
    assert (project_dir / "sqlbuild_project.toml").is_file()
    assert (project_dir / ".agents/skills/sqlbuild/SKILL.md").is_file()


@pytest.mark.parametrize(
    "test_case",
    [
        RunPlaygroundTestCase(
            description="styles key playground next step elements",
            target_path="demo_color_shop",
            expected_stdout_fragments=(),
            expected_color_fragments=(
                "\033[34m\033[1mSQLBuild playground created\033[0m",
                "  \033[34mProject\033[0m: demo_color_shop",
                "\033[34m\033[1mTry\033[0m:",
                "\033[2m  \033[0msqb compile",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_color_terminal_when_running_playground_command_then_it_styles_key_elements(
    test_case: RunPlaygroundTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    use_color_terminal(monkeypatch)

    exit_code: int = run_playground_command(
        PlaygroundCommandRequest(
            project_dir=tmp_path,
            target_path=test_case.target_path,
            template=test_case.template,
        )
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == 0
    expected_fragment: str
    for expected_fragment in test_case.expected_color_fragments:
        assert expected_fragment in captured.out


@pytest.mark.parametrize(
    "test_case",
    [
        WrapperPlaygroundSkillsTestCase(
            description="dagster playground installs skills maintained from the nested project",
            template="dagster",
            project_subdir="waffle_shop",
            expected_skill_file=".agents/skills/sqlbuild/SKILL.md",
        ),
        WrapperPlaygroundSkillsTestCase(
            description="rivers playground installs skills maintained from the nested project",
            template="rivers",
            project_subdir="waffle_shop",
            expected_skill_file=".claude/skills/sqlbuild/SKILL.md",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_wrapper_playground_when_maintaining_skills_then_nested_project_sees_installed_skills(
    test_case: WrapperPlaygroundSkillsTestCase,
    tmp_path: Path,
) -> None:
    exit_code: int = run_playground_command(
        PlaygroundCommandRequest(
            project_dir=tmp_path, target_path="orchestrator", template=test_case.template
        )
    )
    wrapper_dir: Path = tmp_path / "orchestrator"
    project_dir: Path = wrapper_dir / test_case.project_subdir

    fresh: SkillMaintenanceResult = maintain_sqlbuild_skills(project_dir=project_dir)
    installed_skill: Path = project_dir / test_case.expected_skill_file
    installed_skill.write_text("edited\n", encoding="utf-8")
    stale: SkillMaintenanceResult = maintain_sqlbuild_skills(project_dir=project_dir)

    assert exit_code == 0
    assert not (wrapper_dir / test_case.expected_skill_file).exists()
    assert fresh.message == ""
    assert test_case.expected_stale_message in stale.message
