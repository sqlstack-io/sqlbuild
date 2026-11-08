#!/usr/bin/env bash
# Builds with maturin installed from PyPI, for when PyO3/maturin-action cannot download maturin
# from GitHub releases. The version is the one the action resolves for this repository. This runs
# on the host, so it never replaces a manylinux container build.
# Usage: scripts/maturin_from_pypi.sh build|sdist [maturin arguments...]; builds read TARGET.
set -euo pipefail

MATURIN_VERSION=1.16.0
command=$1
shift

python=python
command -v python > /dev/null || python=python3
"${python}" -m venv "${RUNNER_TEMP}/maturin-pypi"
venv_python="${RUNNER_TEMP}/maturin-pypi/bin/python"
[[ -x "${venv_python}" ]] || venv_python="${RUNNER_TEMP}/maturin-pypi/Scripts/python.exe"
"${venv_python}" -m pip install --disable-pip-version-check "maturin==${MATURIN_VERSION}"

if [[ "${command}" == sdist ]]; then
	exec "${venv_python}" -m maturin sdist "$@"
fi
case "${RUNNER_OS}-${TARGET}" in
	macOS-x86_64) triple=x86_64-apple-darwin ;;
	macOS-aarch64) triple=aarch64-apple-darwin ;;
	Windows-x86_64) triple=x86_64-pc-windows-msvc ;;
	*) echo "no host fallback for ${RUNNER_OS} ${TARGET}" >&2; exit 1 ;;
esac
rustup target add "${triple}"
exec "${venv_python}" -m maturin build --target "${triple}" "$@"
