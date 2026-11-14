#!/usr/bin/env bash
# Check that every collected test runs in exactly one shard.
#
# usage: check_test_shards.sh --root DIR [--root DIR ...] --markers EXPR
#          --shard NAME ARG [ARG ...] [--shard ...]
#
# A shard's ARGs are the pytest path arguments it runs: paths under a root, and
# --ignore=PATH for a path under one of them that another shard runs instead.

set -euo pipefail

declare -a collection_roots=()
markers=""
declare -a shard_names=()
declare -a root_names=()
declare -a root_shards=()
declare -A seen_shards=()
declare -A seen_roots=()
declare -A shard_ignores=()

usage() {
  echo "usage: $0 --root DIR [--root DIR ...] --markers EXPR --shard NAME ARG [ARG ...] [--shard ...]" >&2
  exit 2
}

under() {
  [[ "$1" == "$2" || "$1" == "$2/"* ]]
}

under_collection_root() {
  local root
  for root in "${collection_roots[@]}"; do
    if under "$1" "$root"; then
      return 0
    fi
  done
  return 1
}

while (( $# > 0 )) && [[ "$1" != "--shard" ]]; do
  case "$1" in
    --root)
      (( $# >= 2 )) || usage
      collection_roots+=("$2")
      shift 2
      ;;
    --markers)
      (( $# >= 2 )) || usage
      markers="$2"
      shift 2
      ;;
    *)
      usage
      ;;
  esac
done
if (( ${#collection_roots[@]} == 0 )) || [[ -z "$markers" ]] || (( $# == 0 )); then
  usage
fi

while (( $# > 0 )); do
  if [[ "$1" != "--shard" || $# -lt 3 ]]; then
    usage
  fi

  shard="$2"
  shift 2
  if [[ -n "${seen_shards[$shard]:-}" ]]; then
    echo "duplicate shard name: $shard" >&2
    exit 1
  fi
  seen_shards[$shard]=1
  shard_names+=("$shard")
  shard_ignores[$shard]=""

  path_count=0
  declare -a ignores=()
  while (( $# > 0 )) && [[ "$1" != "--shard" ]]; do
    argument="$1"
    shift
    if [[ "$argument" == --ignore=* ]]; then
      ignores+=("${argument#--ignore=}")
      continue
    fi
    root="$argument"
    ((path_count += 1))
    if [[ -n "${seen_roots[$root]:-}" ]]; then
      echo "path is declared more than once: $root" >&2
      exit 1
    fi
    if ! under_collection_root "$root"; then
      echo "path is outside ${collection_roots[*]}: $root" >&2
      exit 1
    fi
    if [[ ! -e "$root" ]]; then
      echo "path does not exist: $root" >&2
      exit 1
    fi
    seen_roots[$root]=1
    root_names+=("$root")
    root_shards+=("$shard")
  done
  if (( path_count == 0 )); then
    echo "shard has no paths: $shard" >&2
    exit 1
  fi
  for ignored in "${ignores[@]}"; do
    if [[ ! -e "$ignored" ]]; then
      echo "ignored path does not exist: $ignored" >&2
      exit 1
    fi
    shard_ignores[$shard]+="$ignored"$'\n'
  done
  unset ignores
done

mkdir -p /tmp/opencode
collection_file=$(mktemp /tmp/opencode/check-test-shards-XXXXXX.log)
trap 'rm -f "$collection_file"' EXIT

if ! uv run pytest "${collection_roots[@]}" --collect-only -q --color=no \
  -m "$markers" >"$collection_file"; then
  cat "$collection_file"
  echo "pytest collection failed for ${collection_roots[*]}" >&2
  exit 1
fi

ignored_by() {
  local shard="$1" test_file="$2" ignored
  while IFS= read -r ignored; do
    if [[ -n "$ignored" ]] && under "$test_file" "$ignored"; then
      return 0
    fi
  done <<<"${shard_ignores[$shard]}"
  return 1
}

declare -A counts=()
declare -A invalid_files=()
test_count=0
while IFS= read -r node_id; do
  if [[ "$node_id" != *::* ]] || ! under_collection_root "${node_id%%::*}"; then
    continue
  fi

  ((test_count += 1))
  test_file="${node_id%%::*}"
  owner_count=0
  owner=""
  ownership=""
  for index in "${!root_names[@]}"; do
    root="${root_names[$index]}"
    shard="${root_shards[$index]}"
    if under "$test_file" "$root" && ! ignored_by "$shard" "$test_file"; then
      ((owner_count += 1))
      owner="$shard"
      ownership+="${ownership:+, }$owner:$root"
    fi
  done

  if (( owner_count != 1 )); then
    invalid_files[$test_file]="${ownership:-none}"
  else
    counts[$owner]=$(( ${counts[$owner]:-0} + 1 ))
  fi
done <"$collection_file"

if (( test_count == 0 )); then
  echo "pytest collection returned no tests under ${collection_roots[*]}" >&2
  exit 1
fi

if (( ${#invalid_files[@]} > 0 )); then
  echo "Test shard coverage failed for ${collection_roots[*]}:" >&2
  for test_file in "${!invalid_files[@]}"; do
    echo "  - $test_file owners: ${invalid_files[$test_file]}" >&2
  done
  exit 1
fi

echo "Test shard coverage passed for ${collection_roots[*]} ($test_count tests):"
for shard in "${shard_names[@]}"; do
  echo "  $shard: ${counts[$shard]:-0}"
done
