"""Library and project logs during a command: what the console shows and what Recall keeps."""

import subprocess
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.recall._test_types import ThirdPartyLogTestCase
from tests.integration.src.sqlbuild.cli.recall.helpers import recall_rows, run, write_project

VENDOR_MODULE: str = (
    '"""A client library installed outside the project."""\n\n'
    "import logging\n\n\n"
    "def sync_orders():\n"
    '    """Log like a client library: progress, then a recovered failure."""\n\n'
    '    logger = logging.getLogger("vendor.client")\n'
    '    logger.info("connecting to the orders service")\n'
    "    try:\n"
    '        raise RuntimeError("token expired")\n'
    "    except RuntimeError:\n"
    '        logger.warning("token refresh failed", exc_info=True)\n'
)
PROJECT_HOOK: str = (
    '"""A project hook that logs and calls the client library."""\n\n'
    "import logging\n\n"
    "import vendor_client\n\n"
    "from sqlbuild.hooks import hook\n\n\n"
    "@hook\n"
    "def sync_orders(ctx):\n"
    '    """Log progress and a recovered failure, then sync through the library."""\n\n'
    '    logger = logging.getLogger("orders_shop.sync")\n'
    '    logger.info("starting the orders sync")\n'
    "    try:\n"
    '        raise ValueError("stale cursor")\n'
    "    except ValueError:\n"
    '        logger.warning("retrying the orders sync", exc_info=True)\n'
    "    vendor_client.sync_orders()\n"
)
ORDERS_MODEL: str = (
    'MODEL (\n  description "One row per order",\n  materialized table,\n'
    '  post_hooks [python("sync_orders")]\n);\n\n'
    "SELECT 1 AS order_id, CAST('2026-04-01' AS DATE) AS ordered_on\n"
)
TRACEBACK: str = "Traceback (most recent call last):"


@pytest.mark.parametrize(
    "test_case",
    (
        ThirdPartyLogTestCase(
            description="normal output keeps project logs and summarises the library",
            build_arguments=("build",),
            expected_present=(
                "[info] orders_shop.sync starting the orders sync",
                "[warning] orders_shop.sync retrying the orders sync",
                "ValueError: stale cursor",
                "[warning] vendor.client token refresh failed",
            ),
            expected_absent=("connecting to the orders service", "RuntimeError: token expired"),
            expected_traceback_count=1,
        ),
        ThirdPartyLogTestCase(
            description="debug output shows every record with its traceback",
            build_arguments=("--debug", "build"),
            expected_present=(
                "[info] orders_shop.sync starting the orders sync",
                "ValueError: stale cursor",
                "[info] vendor.client connecting to the orders service",
                "[warning] vendor.client token refresh failed",
                "RuntimeError: token expired",
            ),
            expected_absent=(),
            expected_traceback_count=2,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_project_and_library_logs_during_build_when_running_then_only_library_is_quiet(
    test_case: ThirdPartyLogTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")
    (project_dir / "sql" / "models" / "orders.sql").write_text(ORDERS_MODEL, encoding="utf-8")
    hook_path: Path = project_dir / "sql" / "hooks" / "python" / "nodes" / "sync.py"
    hook_path.parent.mkdir(parents=True)
    hook_path.write_text(PROJECT_HOOK, encoding="utf-8")
    library_dir: Path = tmp_path / "installed"
    library_dir.mkdir(parents=True)
    (library_dir / "vendor_client.py").write_text(VENDOR_MODULE, encoding="utf-8")

    built: subprocess.CompletedProcess[bytes] = run(
        project_dir, *test_case.build_arguments, PYTHONPATH=str(library_dir)
    )

    stderr: str = built.stderr.decode(errors="replace")
    assert built.returncode == 0, stderr
    assert stderr.count("[warning] vendor.client token refresh failed") == 1, stderr
    assert stderr.count(TRACEBACK) == test_case.expected_traceback_count, stderr
    fragment: str
    for fragment in test_case.expected_present:
        assert fragment in stderr, stderr
    for fragment in test_case.expected_absent:
        assert fragment not in stderr, stderr
    rows: list[list[object]] = recall_rows(
        project_dir,
        "SELECT logger, severity, message FROM diagnostics "
        "WHERE logger IN ('vendor.client', 'orders_shop.sync') ORDER BY occurred_at",
    )
    assert [row[:2] for row in rows] == [
        ["orders_shop.sync", "info"],
        ["orders_shop.sync", "warning"],
        ["vendor.client", "info"],
        ["vendor.client", "warning"],
    ]
    assert str(rows[1][2]).startswith("retrying the orders sync\n" + TRACEBACK)
    assert str(rows[3][2]).startswith("token refresh failed\n" + TRACEBACK)
    assert "RuntimeError: token expired" in str(rows[3][2])
