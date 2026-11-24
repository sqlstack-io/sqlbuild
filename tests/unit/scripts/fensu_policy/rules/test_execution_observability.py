from textwrap import dedent

import pytest
from fensu import RuleCase, RuleFile, RuleResult, evaluate_rule

from scripts.fensu_policy.rules.execution_observability import (
    adapter_public_execute_override,
    event_construction_ownership,
    event_exporter_location,
    raw_driver_execution_boundary,
)
from tests.unit.scripts.fensu_policy.rules._test_types import CustomRuleTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleTestCase(
            description="adapter public execute override faults",
            path="src/sqlbuild/adapters/example/client.py",
            source=(
                "from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter\n"
                "class ExampleAdapter(BaseAdapter):\n"
                "    def execute(self, *, connection, sql): ...\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="adapter super execute override still faults",
            path="src/sqlbuild/adapters/example/client.py",
            source=(
                "from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter\n"
                "class ExampleAdapter(BaseAdapter):\n"
                "    def execute(self, *, connection, sql):\n"
                "        return super().execute(connection=connection, sql=sql)\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="adapter protected hook passes",
            path="src/sqlbuild/adapters/example/client.py",
            source=(
                "from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter\n"
                "class ExampleAdapter(BaseAdapter):\n"
                "    def _execute(self, *, connection, sql): ...\n"
            ),
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="inherited execute passes",
            path="src/sqlbuild/adapters/example/client.py",
            source=(
                "from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter\n"
                "class ExampleAdapter(BaseAdapter):\n"
                "    pass\n"
            ),
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="contract template owns public execute",
            path="src/sqlbuild/adapter/contract/classes/connection.py",
            source="class ConnectionMixin:\n    def execute(self, *, connection, sql): ...\n",
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="registered aliased adapter base still faults",
            path="src/sqlbuild/adapters/foo/classes/foo_adapter.py",
            source=(
                "from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter as Parent\n"
                "class FooAdapter(Parent):\n"
                "    def execute(self, *, connection, sql): ...\n"
            ),
            expected_fault_count=1,
            files=(
                RuleFile(
                    path="src/sqlbuild/adapter/discovery/main/builtins.py",
                    source=(
                        "from sqlbuild.adapters.foo.classes.foo_adapter import FooAdapter\n"
                        "BUILTINS = {'foo': FooAdapter}\n"
                    ),
                ),
            ),
        ),
        CustomRuleTestCase(
            description="only proven aliased adapter faults beside HttpAdapter",
            path="src/sqlbuild/adapters/example/client.py",
            source=(
                "from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter as Parent\n"
                "class HttpAdapter:\n"
                "    def execute(self, request): ...\n"
                "class WarehouseAdapter(Parent):\n"
                "    def execute(self, *, connection, sql): ...\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="qualified adapter module alias faults",
            path="src/sqlbuild/adapters/example/client.py",
            source=(
                "import sqlbuild.adapter.contract.classes.base_adapter as contract\n"
                "class WarehouseAdapter(contract.BaseAdapter):\n"
                "    def execute(self, *, connection, sql): ...\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="package member adapter module alias faults",
            path="src/sqlbuild/adapters/example/client.py",
            source=(
                "from sqlbuild.adapter.contract.classes import base_adapter\n"
                "class WarehouseAdapter(base_adapter.BaseAdapter):\n"
                "    def execute(self, *, connection, sql): ...\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="fully qualified adapter base faults",
            path="src/sqlbuild/adapters/example/client.py",
            source=(
                "import sqlbuild.adapter.contract.classes.base_adapter\n"
                "class WarehouseAdapter("
                "sqlbuild.adapter.contract.classes.base_adapter.BaseAdapter):\n"
                "    def execute(self, *, connection, sql): ...\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="third party BaseAdapter in client module passes",
            path="src/sqlbuild/adapters/example/client.py",
            source=(
                "from third_party.adapters import BaseAdapter\n"
                "class HttpAdapter(BaseAdapter):\n"
                "    def execute(self, request): ...\n"
            ),
            expected_fault_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_declaration_when_checking_execute_owner_then_matches_contract(
    test_case: CustomRuleTestCase,
) -> None:
    result: RuleResult = evaluate_rule(
        rule=adapter_public_execute_override,
        test_case=RuleCase(
            description=test_case.description,
            source=test_case.source,
            expected_fault_count=test_case.expected_fault_count,
            path=test_case.path,
            scope=test_case.scope,
            scope_root=test_case.scope_root,
            files=test_case.files,
        ),
    )

    assert result.fault_count == test_case.expected_fault_count


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleTestCase(
            description="raw connection and cursor aliases fault",
            path="src/sqlbuild/example/main/work.py",
            source=dedent(
                """
                from duckdb import DuckDBPyConnection

                def run(raw_connection: DuckDBPyConnection):
                    connection_alias = raw_connection
                    cursor_alias = connection_alias.cursor()
                    connection_alias.execute("SELECT 1")
                    cursor_alias.executemany("INSERT", [])
                """
            ),
            expected_fault_count=2,
        ),
        CustomRuleTestCase(
            description="known driver import alias faults",
            path="src/sqlbuild/example/main/work.py",
            source=dedent(
                """
                import duckdb as driver

                connection = driver.connect(":memory:")
                cursor = connection.cursor()
                cursor.execute("SELECT 1")
                connection.executemany("INSERT", [])
                """
            ),
            expected_fault_count=2,
        ),
        CustomRuleTestCase(
            description="known imported factory alias faults",
            path="src/sqlbuild/example/main/work.py",
            source=dedent(
                """
                from psycopg import connect as open_database

                database = open_database("dsn")
                database.execute("SELECT 1")
                """
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="direct chained driver and cursor calls fault",
            path="src/sqlbuild/example/main/work.py",
            source=dedent(
                """
                import duckdb

                duckdb.connect(":memory:").execute("SELECT 1")
                duckdb.connect(":memory:").cursor().execute("SELECT 1")
                """
            ),
            expected_fault_count=2,
        ),
        CustomRuleTestCase(
            description="known module execution alias faults",
            path="src/sqlbuild/example/main/work.py",
            source=dedent(
                """
                import duckdb as driver

                driver.execute("SELECT 1")
                """
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="context manager driver alias faults",
            path="src/sqlbuild/example/main/work.py",
            source=dedent(
                """
                from psycopg import connect

                with connect("dsn") as database:
                    database.cursor().execute("SELECT 1")
                """
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="approved observed executor passes",
            path="src/sqlbuild/adapter/contract/classes/observed_cursor.py",
            source=(
                "class ObservedCursor:\n"
                "    def execute(self, sql):\n"
                "        return self.raw_cursor.execute(sql)\n"
            ),
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="adapter and lifecycle execute methods pass",
            path="src/sqlbuild/example/main/work.py",
            source=dedent(
                """
                adapter.execute(connection=connection, sql="SELECT 1")
                operation_lifecycle.execute()
                domain.execute(command)
                """
            ),
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="unrelated HTTP and domain clients pass",
            path="src/sqlbuild/example/main/work.py",
            source=dedent(
                """
                http_client.query("/health")
                api_client.execute(request)
                raw_connection.execute(command)
                """
            ),
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="raw provenance does not leak into sibling function parameters",
            path="src/sqlbuild/example/main/work.py",
            source=dedent(
                """
                import duckdb

                def raw_query():
                    connection = duckdb.connect(":memory:")
                    connection.execute("SELECT 1")

                def domain_query(connection):
                    connection.execute(command)
                """
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="raw provenance is killed by domain reassignment",
            path="src/sqlbuild/example/main/work.py",
            source=dedent(
                """
                import duckdb

                connection = duckdb.connect(":memory:")
                connection.execute("SELECT 1")
                connection = domain_connection
                connection.execute(command)
                """
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="tests are excluded by scope classification",
            path="tests/unit/test_database.py",
            source="raw_connection.execute('SELECT 1')\n",
            expected_fault_count=0,
            scope="test",
            scope_root="tests",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_execution_call_when_checking_raw_boundary_then_matches_evidence(
    test_case: CustomRuleTestCase,
) -> None:
    result: RuleResult = evaluate_rule(
        rule=raw_driver_execution_boundary,
        test_case=RuleCase(
            description=test_case.description,
            source=test_case.source,
            expected_fault_count=test_case.expected_fault_count,
            path=test_case.path,
            scope=test_case.scope,
            scope_root=test_case.scope_root,
            files=test_case.files,
        ),
    )

    assert result.fault_count == test_case.expected_fault_count


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleTestCase(
            description="direct lifecycle constructor alias faults",
            path="src/sqlbuild/example/main/work.py",
            source=(
                "from sqlbuild.observability import LifecycleEvent as Event\n"
                "event = Event(event_type='run_started')\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="direct lifecycle factory alias faults",
            path="src/sqlbuild/example/main/work.py",
            source=(
                "from sqlbuild.observability import create_lifecycle_event as make_event\n"
                "event = make_event(event_type='run_started')\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="qualified lifecycle module constructor faults",
            path="src/sqlbuild/example/main/work.py",
            source=(
                "import sqlbuild.observability as events\n"
                "event = events.LifecycleEvent(event_type='run_started')\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="from package observability alias faults",
            path="src/sqlbuild/example/main/work.py",
            source=(
                "from sqlbuild import observability as events\n"
                "event = events.LifecycleEvent(event_type='run_started')\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="fully qualified observability constructor faults",
            path="src/sqlbuild/example/main/work.py",
            source=(
                "import sqlbuild.observability\n"
                "event = sqlbuild.observability.LifecycleEvent(event_type='run_started')\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="unrelated same named symbols pass",
            path="src/sqlbuild/example/main/work.py",
            source=dedent(
                """
                from third_party.events import LifecycleEvent, DiagnosticLog
                from third_party.factory import create_lifecycle_event
                from third_party.models import LifecycleEventDefinition

                LifecycleEvent()
                DiagnosticLog()
                create_lifecycle_event()
                LifecycleEventDefinition.create()
                """
            ),
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="unauthorized observability sibling factory faults",
            path="src/sqlbuild/runtime/observability/_helpers/rogue.py",
            source=(
                "from sqlbuild.observability import create_lifecycle_event\n"
                "create_lifecycle_event(event_type='run_started')\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="operation lifecycle public emission passes",
            path="src/sqlbuild/example/main/work.py",
            source=(
                "from sqlbuild.observability import OperationLifecycle\n"
                "with OperationLifecycle(operation_kind='x', operation_name='y'):\n"
                "    run()\n"
            ),
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="event type annotation and read pass",
            path="src/sqlbuild/example/main/work.py",
            source=(
                "from sqlbuild.observability import LifecycleEvent\n"
                "def consume(event: LifecycleEvent) -> str:\n"
                "    return event.event_type\n"
            ),
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="lifecycle owner constructor passes",
            path="src/sqlbuild/runtime/observability/_helpers/factory.py",
            source=(
                "from sqlbuild.runtime.observability.models import LifecycleEvent\n"
                "event = LifecycleEvent(event_type='run_started')\n"
            ),
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="diagnostic constructor outside owner faults",
            path="src/sqlbuild/example/main/work.py",
            source=(
                "from sqlbuild.observability import DiagnosticLog\n"
                "log = DiagnosticLog(message='unsafe')\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="catalog definition and declaration outside owner fault",
            path="src/sqlbuild/example/main/catalog.py",
            source=dedent(
                """
                from sqlbuild.runtime.observability.models import LifecycleEventDefinition

                LIFECYCLE_EVENT_CATALOG_V2 = {
                    "custom": LifecycleEventDefinition.create(allowed=frozenset())
                }
                """
            ),
            expected_fault_count=2,
        ),
        CustomRuleTestCase(
            description="qualified catalog definition outside owner faults",
            path="src/sqlbuild/example/main/catalog.py",
            source=(
                "import sqlbuild.runtime.observability.models as models\n"
                "definition = models.LifecycleEventDefinition.create()\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="fully qualified catalog definition outside owner faults",
            path="src/sqlbuild/example/main/catalog.py",
            source=(
                "import sqlbuild.runtime.observability.models\n"
                "definition = sqlbuild.runtime.observability.models."
                "LifecycleEventDefinition.create()\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="fully qualified third party lifecycle symbols pass",
            path="src/sqlbuild/example/main/work.py",
            source=(
                "import third_party.observability\n"
                "event = third_party.observability.LifecycleEvent()\n"
            ),
            expected_fault_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_event_usage_when_checking_event_owner_then_matches_contract(
    test_case: CustomRuleTestCase,
) -> None:
    result: RuleResult = evaluate_rule(
        rule=event_construction_ownership,
        test_case=RuleCase(
            description=test_case.description,
            source=test_case.source,
            expected_fault_count=test_case.expected_fault_count,
            path=test_case.path,
            scope=test_case.scope,
            scope_root=test_case.scope_root,
            files=test_case.files,
        ),
    )

    assert result.fault_count == test_case.expected_fault_count


@pytest.mark.parametrize(
    "test_case",
    [
        CustomRuleTestCase(
            description="recursive project exporter declaration passes",
            path="python/event_sinks/private/audit.py",
            source=(
                "from sqlbuild.event_sinks import lifecycle_event_sink as exporter\n"
                "@exporter\n"
                "def audit(event): ...\n"
            ),
            expected_fault_count=0,
            scope_root=".",
        ),
        CustomRuleTestCase(
            description="misplaced project exporter declaration faults",
            path="plugins/audit.py",
            source=(
                "from sqlbuild.event_sinks import lifecycle_event_sink\n"
                "@lifecycle_event_sink(name='audit')\n"
                "def audit(event): ...\n"
            ),
            expected_fault_count=1,
            scope_root=".",
        ),
        CustomRuleTestCase(
            description="qualified misplaced exporter declaration faults",
            path="plugins/audit.py",
            source=(
                "import sqlbuild.event_sinks as exporters\n"
                "@exporters.lifecycle_event_sink\n"
                "def audit(event): ...\n"
            ),
            expected_fault_count=1,
            scope_root=".",
        ),
        CustomRuleTestCase(
            description="from package exporter module alias faults",
            path="plugins/audit.py",
            source=(
                "from sqlbuild import event_sinks as ex\n"
                "@ex.lifecycle_event_sink\n"
                "def audit(event): ...\n"
            ),
            expected_fault_count=1,
            scope_root=".",
        ),
        CustomRuleTestCase(
            description="fully qualified exporter decorator faults",
            path="plugins/audit.py",
            source=(
                "import sqlbuild.event_sinks\n"
                "@sqlbuild.event_sinks.lifecycle_event_sink\n"
                "def audit(event): ...\n"
            ),
            expected_fault_count=1,
            scope_root=".",
        ),
        CustomRuleTestCase(
            description="unrelated third party exporter decorator passes",
            path="plugins/audit.py",
            source=(
                "from third_party.exporters import event_exporter\n"
                "@lifecycle_event_sink\n"
                "def audit(event): ...\n"
            ),
            expected_fault_count=0,
            scope_root=".",
        ),
        CustomRuleTestCase(
            description="core public exporter facade passes",
            path="src/sqlbuild/event_sinks.py",
            source=(
                "from sqlbuild.runtime.event_exporting.models import LifecycleEventSinkDefinition\n"
            ),
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="compiler integration seam passes",
            path="src/sqlbuild/compiler/discovery/models.py",
            source=(
                "from sqlbuild.runtime.event_exporting.models import LifecycleEventSinkDefinition\n"
            ),
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="compiler integration neighbor faults",
            path="src/sqlbuild/compiler/discovery/main/neighbor.py",
            source=(
                "from sqlbuild.runtime.event_exporting.models import LifecycleEventSinkDefinition\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="private exporter import outside seam faults",
            path="src/sqlbuild/example/main/work.py",
            source=(
                "from sqlbuild.runtime.event_exporting.classes.dispatcher import EventExporterDispatcher\n"
            ),
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="misplaced core exporter implementation faults",
            path="src/sqlbuild/example/main/dispatcher.py",
            source="class EventExporterDispatcher:\n    pass\n",
            expected_fault_count=1,
        ),
        CustomRuleTestCase(
            description="core exporter implementation owner passes",
            path="src/sqlbuild/runtime/event_exporting/classes/dispatcher.py",
            source="class EventExporterDispatcher:\n    pass\n",
            expected_fault_count=0,
        ),
        CustomRuleTestCase(
            description="public exporter types remain consumable",
            path="src/sqlbuild/example/main/work.py",
            source="from sqlbuild.event_sinks import LifecycleEventSinkDefinition\n",
            expected_fault_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_exporter_usage_when_checking_location_then_matches_contract(
    test_case: CustomRuleTestCase,
) -> None:
    result: RuleResult = evaluate_rule(
        rule=event_exporter_location,
        test_case=RuleCase(
            description=test_case.description,
            source=test_case.source,
            expected_fault_count=test_case.expected_fault_count,
            path=test_case.path,
            scope=test_case.scope,
            scope_root=test_case.scope_root,
            files=test_case.files,
        ),
    )

    assert result.fault_count == test_case.expected_fault_count
