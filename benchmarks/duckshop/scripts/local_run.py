"""Run DuckShop's unit tests locally on DuckDB through SQLQuack, by default and in strict mode.

    python benchmarks/duckshop/scripts/local_run.py --sqlquack <binary> --policy <sqlquack.toml>

`--sqlquack` defaults to `$SQLBUILD_SQLQUACK` and `--policy` to `$SQLQUACK_POLICY`; the policy is
SQLQuack's reviewed `sqlquack.toml` from the same SQLQuack checkout as the binary. The project is
copied to a temporary directory without `sqlbuild_local.toml` or `target/`, so no warehouse
connection is read and the tracked project is never changed. Both runs use `--target edge`, whose
pinned session parameters the converter is given.

Each run writes `<out>/<mode>.json` and `<out>/<mode>.log` and prints pass, refused (of which
refused by a run-time guard), error and wrong counts. A wrong result is a test that converted, ran
and returned different rows than Snowflake recorded. The exit code is 1 when any test is wrong, 2
when the inputs are missing or a run produced no report, and 0 otherwise.
"""

from __future__ import annotations

import argparse
import collections
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

PROJECT = pathlib.Path(__file__).resolve().parent.parent / "project"
DEFAULT_OUT = PROJECT / "target" / "local-run"
SQLQUACK_ENV = "SQLBUILD_SQLQUACK"
POLICY_ENV = "SQLQUACK_POLICY"
POLICY_FILE = "sqlquack.toml"
LEFT_OUT = ("sqlbuild_local.toml", "target", "logs", POLICY_FILE)
TARGET = "edge"
MODES: dict[str, tuple[str, ...]] = {"default": (), "strict": ("--strict",)}
PASS = "pass"
REFUSED = "refused"
ERROR = "error"
WRONG = "fail"
RUN_TIME_GUARD = "refused at run time"
EXIT_WRONG = 1
EXIT_USAGE = 2


def main() -> int:
    """Run both modes and report the counts; see the module docstring for the exit codes."""

    arguments = _arguments()
    sqlquack: str = arguments.sqlquack or os.environ.get(SQLQUACK_ENV, "")
    policy: str = arguments.policy or os.environ.get(POLICY_ENV, "")
    problem: str | None = _missing_input(sqlquack, policy, arguments.sqb)
    if problem is not None:
        print(f"local_run: {problem}", file=sys.stderr)
        return EXIT_USAGE
    out: pathlib.Path = arguments.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    wrong: int = 0
    with tempfile.TemporaryDirectory(prefix="duckshop-local-") as scratch:
        project: pathlib.Path = pathlib.Path(scratch) / "project"
        shutil.copytree(PROJECT, project, ignore=shutil.ignore_patterns(*LEFT_OUT))
        shutil.copyfile(policy, project / POLICY_FILE)
        for mode, flags in MODES.items():
            report: pathlib.Path = out / f"{mode}.json"
            report.unlink(missing_ok=True)
            code: int = _run(arguments, (project, sqlquack), (flags, report, out / f"{mode}.log"))
            if not report.is_file():
                print(
                    f"local_run: {mode} wrote no report (exit {code}); see {mode}.log",
                    file=sys.stderr,
                )
                return EXIT_USAGE
            wrong += _summarise(mode, json.loads(report.read_text(encoding="utf-8")))
    return EXIT_WRONG if wrong else 0


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--sqlquack", help=f"the sqlquack executable (default: ${SQLQUACK_ENV})")
    parser.add_argument("--policy", help=f"SQLQuack's sqlquack.toml (default: ${POLICY_ENV})")
    parser.add_argument("--sqb", default="sqb", help="the sqb executable (default: sqb on PATH)")
    parser.add_argument("--out", type=pathlib.Path, default=DEFAULT_OUT, help="report directory")
    parser.add_argument("--select", help="a selector to limit the tests, as for sqb test")
    parser.add_argument("--exclude", nargs="+", help="tests to leave out, as for sqb test")
    return parser.parse_args()


def _missing_input(sqlquack: str, policy: str, sqb: str) -> str | None:
    if not sqlquack:
        return f"no SQLQuack executable; pass --sqlquack or set {SQLQUACK_ENV}"
    if not pathlib.Path(sqlquack).is_file():
        return f"the SQLQuack executable {sqlquack} does not exist"
    if not policy:
        return (
            f"no SQLQuack policy; pass --policy or set {POLICY_ENV} to the sqlquack.toml of the "
            "SQLQuack checkout the executable was built from"
        )
    if not pathlib.Path(policy).is_file():
        return f"the SQLQuack policy {policy} does not exist"
    if shutil.which(sqb) is None:
        return f"no sqb executable at {sqb}"
    return None


def _run(
    arguments: argparse.Namespace,
    where: tuple[pathlib.Path, str],
    outputs: tuple[tuple[str, ...], pathlib.Path, pathlib.Path],
) -> int:
    project, sqlquack = where
    flags, report, log = outputs
    command: list[str] = [arguments.sqb, "--no-color", "test", "--local", *flags]
    command += ["--target", TARGET, "--json-output", str(report)]
    if arguments.select:
        command += ["--select", arguments.select]
    if arguments.exclude:
        command += ["--exclude", *arguments.exclude]
    environment: dict[str, str] = {**os.environ, SQLQUACK_ENV: sqlquack}
    with log.open("w", encoding="utf-8") as output:
        completed = subprocess.run(
            command,
            cwd=project,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=output,
            stderr=subprocess.STDOUT,
            check=False,
        )
    return completed.returncode


def _summarise(mode: str, report: dict[str, list[dict[str, object]]]) -> int:
    checks: list[dict[str, object]] = report.get("checks") or []
    statuses = collections.Counter(str(check.get("status")) for check in checks)
    guarded: int = sum(
        check.get("status") == REFUSED and RUN_TIME_GUARD in str(check.get("error_message"))
        for check in checks
    )
    other: int = len(checks) - sum(statuses[status] for status in (PASS, REFUSED, ERROR, WRONG))
    print(
        f"{mode}: tests={len(checks)} pass={statuses[PASS]} refused={statuses[REFUSED]} "
        f"(run-time guard {guarded}) error={statuses[ERROR]} wrong={statuses[WRONG]} other={other}"
    )
    for check in checks:
        if check.get("status") == WRONG:
            print(f"  WRONG {check.get('name')}")
    return statuses[WRONG]


if __name__ == "__main__":
    sys.exit(main())
