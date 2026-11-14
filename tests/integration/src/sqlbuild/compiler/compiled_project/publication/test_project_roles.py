"""Installed publication compiles roles.toml; person grants follow selector coverage per snapshot."""

import subprocess
from pathlib import Path
from typing import Any
from uuid import uuid4

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ProjectRoleRunTestCase,
    ProjectRolesTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    INTERMEDIATE_LOADER_FILES,
    publish_generation,
    read_person_sql,
    snapshot_api,
    tagged_model,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    person_login,
    person_request,
    provision_person,
    request_person_run,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectRolesTestCase(
            description="roles.toml grants follow tags and every publication applies at once",
            roles_file="benchmarks/duckshop/edge_cases/orchestration/project_roles.toml",
            expected_warning=(
                "Warning: role orders_support grant 2 selects no nodes (tag:support,tag:pii); "
                "it grants nothing in this snapshot"
            ),
            expected_denial="missing view.sql permission for this target",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_roles_when_published_then_person_sql_follows_selector_coverage_at_once(
    test_case: ProjectRolesTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    roles: str = Path(test_case.roles_file).read_text()
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\n'
                '[connections.local]\ndatabase = "warehouse.duckdb"\n'
                '[targets.development]\nschema = "main"\n'
            ),
            "models/orders.sql": tagged_model("orders", "support"),
            "models/customers.sql": tagged_model("customers", "pii"),
            "roles.toml": roles,
        },
    )
    with snapshot_api(
        tmp_path / "control",
        server_arguments=("--password-origin", "https://control.example.com"),
    ) as api:
        first: subprocess.CompletedProcess[str] = publish_generation(project, api, 1)
        assert first.returncode == 0, first.stderr
        assert test_case.expected_warning in first.stderr
        admin: dict[str, Any] = provision_person(
            tmp_path / "control", "create-user", "orders-admin"
        )
        person: dict[str, Any] = provision_person(
            tmp_path / "control", "create-principal", "orders-support"
        )
        admin_session: tuple[str, str] = person_login(api, admin["name"])
        session: tuple[str, str] = person_login(api, person["name"])
        assert person_request(
            api,
            "/api/admin/roles",
            body={
                "principal_id": person["principal_id"],
                "role": "orders_support",
                "location_id": api.location_id,
                "target": "development",
            },
            session=admin_session,
        ) == {"applied": True}
        assert read_person_sql(api, session, "model:orders") == (200, "model:orders")
        assert read_person_sql(api, session, "model:customers") == (403, test_case.expected_denial)
        write_files(
            project_dir=project,
            files={"models/customers.sql": tagged_model("customers", "pii, support")},
        )
        grown: subprocess.CompletedProcess[str] = publish_generation(project, api, 2)
        assert grown.returncode == 0, grown.stderr
        assert "Warning:" not in grown.stderr
        assert read_person_sql(api, session, "model:customers") == (200, "model:customers")
        write_files(
            project_dir=project,
            files={"roles.toml": roles.replace('nodes = ["tag:support"]', 'nodes = ["tag:pii"]')},
        )
        rewritten: subprocess.CompletedProcess[str] = publish_generation(project, api, 3)
        assert rewritten.returncode == 0, rewritten.stderr
        assert read_person_sql(api, session, "model:customers") == (200, "model:customers")
        assert read_person_sql(api, session, "model:orders")[0] == 403


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectRoleRunTestCase(
            description="a role granting the loader chain runs the job",
            role="shipments_runner",
            expected_response=(200, {"applied": True}),
        ),
        ProjectRoleRunTestCase(
            description="a role missing the intermediate loader is refused",
            role="orders_runner",
            expected_response=(403, "missing job.run permission for this target"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_job_loading_an_intermediate_loader_when_a_role_runs_it_then_the_grant_must_cover_it(
    test_case: ProjectRoleRunTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\n'
                '[connections.local]\ndatabase = "warehouse.duckdb"\n'
                '[targets.development]\nschema = "main"\n'
            ),
            "roles.toml": Path(
                "benchmarks/duckshop/edge_cases/orchestration/intermediate_loader_roles.toml"
            ).read_text(),
        }
        | INTERMEDIATE_LOADER_FILES,
    )
    with snapshot_api(
        tmp_path / "control",
        server_arguments=("--password-origin", "https://control.example.com"),
    ) as api:
        published: subprocess.CompletedProcess[str] = publish_generation(project, api, 1)
        assert published.returncode == 0, published.stderr
        admin: dict[str, Any] = provision_person(
            tmp_path / "control", "create-user", "orders-admin"
        )
        person: dict[str, Any] = provision_person(
            tmp_path / "control", "create-principal", "orders-runner"
        )
        assert person_request(
            api,
            "/api/admin/roles",
            body={
                "principal_id": person["principal_id"],
                "role": test_case.role,
                "location_id": api.location_id,
                "target": "development",
            },
            session=person_login(api, admin["name"]),
        ) == {"applied": True}
        request: dict[str, Any] = {
            "run_id": str(uuid4()),
            "location_id": api.location_id,
            "target": "development",
            "job_key": "annual_orders",
            "reason": "Refresh shipments",
            "priority": 0,
        }
        session: tuple[str, str] = person_login(api, person["name"])
        assert request_person_run(api, session, request) == test_case.expected_response
