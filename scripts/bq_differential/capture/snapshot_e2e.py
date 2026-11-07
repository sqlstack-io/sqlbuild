"""Pytest plugin: snapshot each project a BigQuery e2e test hands to sqb, then skip the test.

No warehouse: the credentials check sees a placeholder project, every warehouse helper is a no-op
and the BigQuery client is a stub, so setup before the first sqb call passes.
"""
import os, re, shutil, sys
from pathlib import Path
import pytest

OUT = Path(os.environ["BQ_SNAPSHOT_OUT"])
os.environ.setdefault("SQB_TEST_BIGQUERY_PROJECT", "example-project")
os.environ.setdefault("SQB_TEST_BIGQUERY_LOCATION", "US")
_seen = {}
WAREHOUSE = re.compile(r"^(ensure|fetch|execute|cleanup|drop|wait|truncate)_.*|.*bigquery.*(rows|dataset|table|ready|cleanup)$")

class _Stub:
    def __init__(self, *a, **k): pass
    def __getattr__(self, name): return _Stub()
    def __call__(self, *a, **k): return _Stub()
    def __iter__(self): return iter(())
    def __enter__(self): return self
    def __exit__(self, *a): return False
    def result(self, *a, **k): return []

def pytest_configure(config):
    try:
        from google.cloud import bigquery
        bigquery.Client = _Stub
    except Exception:
        pass

def _snapshot(*args, project_dir=None, **kwargs):
    if project_dir is None:
        project_dir = kwargs.get("cwd")
    test = re.sub(r"\W+", "_", _seen.get("current", "unknown"))[-110:]
    target = OUT / f"{_seen['n']:02d}_{test}"
    if project_dir is not None and Path(project_dir).is_dir() and not target.exists():
        shutil.copytree(project_dir, target, ignore=shutil.ignore_patterns("target", ".sqlbuild", "*.duckdb"))
    pytest.skip("project snapshotted")

def _noop(*args, **kwargs):
    return []

@pytest.hookimpl(tryfirst=True)
def pytest_runtest_setup(item):
    _seen["current"] = item.nodeid.split("::", 1)[-1]
    _seen["n"] = _seen.get("n", 0) + 1
    for module in list(sys.modules.values()):
        name = getattr(module, "__name__", "") or ""
        path = str(getattr(module, "__file__", "") or "")
        if not (name.startswith("tests.") or "/tests/" in path):
            continue
        for attr in list(vars(module)):
            if attr in ("run_sqb", "run_sqb_with_pty"):
                setattr(module, attr, _snapshot)
            elif WAREHOUSE.match(attr) and callable(getattr(module, attr)) and not isinstance(getattr(module, attr), type):
                setattr(module, attr, _noop)

import subprocess as _subprocess
_original_run = _subprocess.run

def _run(args, *a, **k):
    argv = [str(x) for x in args] if isinstance(args, (list, tuple)) else str(args).split()
    if "sqb" in argv and "--project-dir" in argv:
        return _snapshot(project_dir=argv[argv.index("--project-dir") + 1])
    if "sqb" in argv:
        return _snapshot(project_dir=k.get("cwd"))
    return _original_run(args, *a, **k)

_subprocess.run = _run
