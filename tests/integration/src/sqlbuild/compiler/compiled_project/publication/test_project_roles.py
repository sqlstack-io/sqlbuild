"""Installed publication compiles roles.toml; person grants follow selector coverage per snapshot."""

import subprocess
from pathlib import Path
from typing import Any
from urllib.error import HTTPError

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ProjectRolesTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    publish_generation,
    read_person_sql,
    snapshot_api,
    tagged_model,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    person_login,
    person_request,
    provision_person,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectRolesTestCase(
            description="roles.toml grants follow tags, rewrites wait, removals apply at once",
            roles_file="benchmarks/duckshop/edge_cases/orchestration/project_roles.toml",
            expected_warning=(
                "Warning: role orders_support grant 2 selects no nodes (tag:support,tag:pii); "
                "it grants nothing in this snapshot"
            ),
            expected_denial=(
                "missing view.sql on nodes model:customers; all requested nodes denied"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_roles_when_published_and_approved_then_person_sql_follows_selector_coverage(
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
        activation: dict[str, Any] = {
            "location_id": api.location_id,
            "target": "development",
            "role_key": "orders_support",
            "reason": "Support answers order questions",
        }

        def approve() -> Any:
            requested: dict[str, Any] = person_request(
                api, "/api/admin/role-activations", body=activation, session=admin_session
            )
            return person_request(
                api,
                "/api/admin/role-approvals",
                body={
                    "request_id": requested["request_id"],
                    "decision": "approved",
                    "reason": "Reviewed the support grants",
                    "confirm_self_approval": True,
                },
                session=admin_session,
            )

        assert read_person_sql(api, session, "model:orders")[0] == 403
        assert approve() == {"applied": True}
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
        with pytest.raises(HTTPError) as unchanged:
            person_request(
                api, "/api/admin/role-activations", body=activation, session=admin_session
            )
        assert unchanged.value.code == 400
        write_files(
            project_dir=project,
            files={"roles.toml": roles.replace('nodes = ["tag:support"]', 'nodes = ["tag:pii"]')},
        )
        rewritten: subprocess.CompletedProcess[str] = publish_generation(project, api, 3)
        assert rewritten.returncode == 0, rewritten.stderr
        assert read_person_sql(api, session, "model:orders")[0] == 403
        assert read_person_sql(api, session, "model:customers")[0] == 403
        assert approve() == {"applied": True}
        assert read_person_sql(api, session, "model:customers") == (200, "model:customers")
        assert read_person_sql(api, session, "model:orders")[0] == 403
