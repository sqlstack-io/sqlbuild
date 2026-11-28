#!/usr/bin/env bash
# Fails when any distribution is too large for PyPI. PyPI rejects files over 100 MiB for this
# project, and an upload that hits the limit stops part way, leaving a release half published.
# The limit sits just under PyPI's so every file is checked before anything uploads.
set -euo pipefail

limit_mib=99
limit=$((limit_mib * 1024 * 1024))

if (( $# == 0 )); then
  echo "No distributions to check" >&2
  exit 1
fi

status=0
for file in "$@"; do
  bytes="$(wc -c < "${file}" | tr -d ' ')"
  echo "$(basename "${file}"): ${bytes} bytes"
  if (( bytes > limit )); then
    echo "::error file=${file}::$(basename "${file}") is ${bytes} bytes; distributions must be at most ${limit_mib} MiB (${limit} bytes) because PyPI rejects files over 100 MiB"
    status=1
  fi
done
exit "${status}"
