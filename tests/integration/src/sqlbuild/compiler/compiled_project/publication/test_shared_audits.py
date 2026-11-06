"""Audits sharing a name across attachments publish as one snapshot node per attachment."""

import json
import subprocess
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    SharedAuditPublicationTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    SHARED_AUDITS,
    audit_node_keys,
    create_playground,
    current_snapshot,
    publish_generation,
    snapshot_api,
    write_shared_audits,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SharedAuditPublicationTestCase(
            description="the default playground publishes every inline audit attachment",
            create_project=create_playground,
            target="dev",
            expected_audits=frozenset(
                {
                    "audit:not_null:model:stg_customers:customer_id",
                    "audit:not_null:model:stg_orders:order_id",
                    "audit:unique:model:stg_customers:customer_id",
                    "audit:unique:seed:waffle_types:waffle_type_id",
                    "audit:revenue_is_non_negative:model:daily_revenue",
                }
            ),
            expected_count=30,
        ),
        SharedAuditPublicationTestCase(
            description="DuckShop shared inline and named audits publish once per attachment",
            create_project=write_shared_audits,
            target="development",
            expected_audits=frozenset(json.loads(SHARED_AUDITS.read_text())["expected_audits"]),
            expected_count=5,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_audits_sharing_a_name_when_published_then_each_attachment_is_a_stable_node(
    test_case: SharedAuditPublicationTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    test_case.create_project(project)
    with snapshot_api(tmp_path / "control") as api:
        first: subprocess.CompletedProcess[str] = publish_generation(
            project, api, 1, test_case.target
        )
        assert first.returncode == 0, first.stderr
        audits: frozenset[str] = audit_node_keys(api, test_case.target)
        assert test_case.expected_audits <= audits
        assert len(audits) == test_case.expected_count
        second: subprocess.CompletedProcess[str] = publish_generation(
            project, api, 2, test_case.target
        )
        assert second.returncode == 0, second.stderr
        assert audit_node_keys(api, test_case.target) == audits
        assert current_snapshot(api, test_case.target)["changes"] == []
