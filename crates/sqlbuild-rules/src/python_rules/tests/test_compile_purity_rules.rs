use crate::tests::helpers::install_test_identity;
use serde_json::json;

use crate::python_rules::tests::helpers::{owned_faults, python_rule_faults};
use crate::python_rules::tests::test_types::{PythonRunFailureTestCase, PythonRunTestCase};

const PURE_FACTORY: &str = r#"from sqlbuild.assets import factory, asset

from python.inventory.snapshots.constants import WAREHOUSE_SPECS


@factory
def warehouse_snapshots():
    nodes = []
    for spec in WAREHOUSE_SPECS:

        @asset(name=spec)
        def snapshot(ctx):
            return requests.get(spec)

        nodes.append(snapshot)
    return nodes
"#;
const RUNTIME_ONLY: &str = r#"import psycopg
import requests

from sqlbuild.assets import asset


def main(ctx):
    return psycopg.connect("dsn").execute("SELECT 1")


@asset
def export(ctx):
    return requests.post("https://example.com/orders")


class CarrierApi:
    def setup(self):
        self.session = requests.get("https://example.com/token")
"#;
const QUERYING_FACTORY: &str = r#"import psycopg

from sqlbuild.assets import factory

from ._helpers.spec_loading import load_specs


@factory
def warehouse_snapshots():
    codes = psycopg.connect("dsn").execute("SELECT code FROM warehouses").fetchall()
    return load_specs(codes)
"#;
const SPEC_LOADING: &str = r#"import subprocess


def load_specs(codes):
    return list_specs(codes)


def list_specs(codes):
    return subprocess.run(["ls"], check=True)
"#;
const REGIONS: &str = r#"import requests

STOREFRONT_REGIONS = requests.get("https://example.com/regions").json()
"#;
const SENSOR: &str = r#"from orchestration.sensors.inventory._helpers import feed

LATEST = feed.latest()
"#;
const FEED: &str = r#"from python.utils.network import ping


def latest():
    return ping()


def again():
    return latest()
"#;
const NETWORK: &str = r#"import socket

from python.utils import network


def ping():
    network.pong()
    return socket.create_connection(("example.com", 443))


def pong():
    return ping()
"#;
const CLIENTS: &str = r#"import boto3
import snowflake.connector as connector
import urllib.request
from paramiko import SSHClient

BUCKET = boto3.client("s3")
SESSION = connector.connect(account="example")
PAGE = urllib.request.urlopen("https://example.com")
SHELL = SSHClient()
"#;
const DEFERRED: &str = r#"import subprocess

from python.utils.shell import deferred_runner

HANDLERS = {"ls": lambda: subprocess.run(["ls"])}
RUNNER = deferred_runner()
DEFAULT = (lambda code=subprocess.run(["id"]): code)

if __name__ == "__main__":
    subprocess.run(["ls"])
else:
    subprocess.run(["id"])
"#;
const SHELL: &str = r#"import subprocess


def deferred_runner():
    return lambda: subprocess.run(["ls"])
"#;

#[test]
fn given_compile_time_python_when_evaluating_compile_time_purity_then_reports_calls_reaching_external_systems()
 {
    install_test_identity();
    let test_cases = [
        PythonRunTestCase {
            description: "static specs, node bodies, main() and provider methods report nothing",
            files: &[
                ("python/inventory/snapshots/node.py", PURE_FACTORY),
                (
                    "python/inventory/snapshots/constants.py",
                    "WAREHOUSE_SPECS = (\"north\", \"south\")\n",
                ),
                ("python/orders/export/main.py", RUNTIME_ONLY),
            ],
            config: json!({ "select": ["SQBRPYTHON022"] }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "module scope, factory bodies and the project functions they call are followed to each external call",
            files: &[
                ("python/inventory/snapshots/node.py", QUERYING_FACTORY),
                (
                    "python/inventory/snapshots/_helpers/spec_loading.py",
                    SPEC_LOADING,
                ),
                ("python/orders/storefront_import/constants.py", REGIONS),
                ("orchestration/sensors/inventory/stock_feed.py", SENSOR),
                ("orchestration/sensors/inventory/_helpers/feed.py", FEED),
                ("python/utils/network.py", NETWORK),
                ("providers/carrier_api/constants.py", CLIENTS),
            ],
            config: json!({ "select": ["SQBRPYTHON022"] }),
            expected_faults: vec![
                (
                    "SQBRPYTHON022",
                    "orchestration/sensors/inventory/stock_feed.py",
                    3,
                    10,
                ),
                ("SQBRPYTHON022", "providers/carrier_api/constants.py", 6, 10),
                ("SQBRPYTHON022", "providers/carrier_api/constants.py", 7, 11),
                ("SQBRPYTHON022", "providers/carrier_api/constants.py", 8, 8),
                ("SQBRPYTHON022", "providers/carrier_api/constants.py", 9, 9),
                (
                    "SQBRPYTHON022",
                    "python/inventory/snapshots/node.py",
                    10,
                    13,
                ),
                (
                    "SQBRPYTHON022",
                    "python/inventory/snapshots/node.py",
                    11,
                    12,
                ),
                (
                    "SQBRPYTHON022",
                    "python/orders/storefront_import/constants.py",
                    3,
                    22,
                ),
            ],
        },
        PythonRunTestCase {
            description: "lambda bodies and the __main__ block never run on import, but a lambda default and an else branch do",
            files: &[
                ("python/orders/export/constants.py", DEFERRED),
                ("python/utils/shell.py", SHELL),
            ],
            config: json!({ "select": ["SQBRPYTHON022"] }),
            expected_faults: vec![
                ("SQBRPYTHON022", "python/orders/export/constants.py", 7, 24),
                ("SQBRPYTHON022", "python/orders/export/constants.py", 12, 5),
            ],
        },
        PythonRunTestCase {
            description: "an approved loader boundary stops the walk at exactly that function",
            files: &[
                ("python/inventory/snapshots/node.py", QUERYING_FACTORY),
                (
                    "python/inventory/snapshots/_helpers/spec_loading.py",
                    SPEC_LOADING,
                ),
            ],
            config: json!({
                "select": ["SQBRPYTHON022"],
                "rule_options": { "SQBRPYTHON022": { "approved_loader_boundaries": [
                    "python.inventory.snapshots._helpers.spec_loading.load_specs",
                ] } },
            }),
            expected_faults: vec![(
                "SQBRPYTHON022",
                "python/inventory/snapshots/node.py",
                10,
                13,
            )],
        },
        PythonRunTestCase {
            description: "an approved factory runs its body unchecked",
            files: &[
                ("python/inventory/snapshots/node.py", QUERYING_FACTORY),
                (
                    "python/inventory/snapshots/_helpers/spec_loading.py",
                    SPEC_LOADING,
                ),
            ],
            config: json!({
                "select": ["SQBRPYTHON022"],
                "rule_options": { "SQBRPYTHON022": { "approved_loader_boundaries": [
                    "python.inventory.snapshots.node.warehouse_snapshots",
                ] } },
            }),
            expected_faults: vec![],
        },
    ];
    for test_case in test_cases {
        let faults = python_rule_faults(test_case.files, test_case.config).expect("rules run");
        assert_eq!(
            faults,
            owned_faults(&test_case.expected_faults),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_boundary_naming_a_module_when_evaluating_compile_time_purity_then_the_run_fails() {
    install_test_identity();
    let test_cases = [PythonRunFailureTestCase {
        description: "a module is not a loader boundary",
        files: &[
            ("python/inventory/snapshots/node.py", QUERYING_FACTORY),
            (
                "python/inventory/snapshots/_helpers/spec_loading.py",
                SPEC_LOADING,
            ),
        ],
        config: json!({
            "select": ["SQBRPYTHON022"],
            "rule_options": { "SQBRPYTHON022": { "approved_loader_boundaries": [
                "python.inventory.snapshots._helpers.spec_loading",
            ] } },
        }),
        expected_error: "names the module python.inventory.snapshots._helpers.spec_loading",
    }];
    for test_case in test_cases {
        let faults = python_rule_faults(test_case.files, test_case.config);

        assert!(
            faults
                .as_ref()
                .is_err_and(|error| error.contains(test_case.expected_error)),
            "{}: {faults:?}",
            test_case.description
        );
    }
}
