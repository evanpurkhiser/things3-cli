#!/usr/bin/env bash
set -euo pipefail

export TZ=UTC

# Wrapper for trycmd cases.
#
# Default behavior: if ./journal.json exists and --load-journal is not
# explicitly provided, append --load-journal journal.json for things3.
# For all commands, enforce deterministic flags and if cloud commit request
# logs are present on stderr, pretty-print just their request payload JSON.

argv=("$@")

if [[ ${#argv[@]} -gt 0 && "${argv[0]}" == "things3" ]]; then
	if [[ -n "${TRYCMD_BIN_THINGS3:-}" ]]; then
		argv[0]="${TRYCMD_BIN_THINGS3}"
	fi

	has_no_color=0
	has_no_cloud=0
	has_load_journal=0
	has_log_level=0
	has_log_format=0
	has_log_filter=0
	for ((i = 1; i < ${#argv[@]}; i++)); do
		if [[ "${argv[i]}" == "--no-color" ]]; then
			has_no_color=1
		elif [[ "${argv[i]}" == "--no-cloud" ]]; then
			has_no_cloud=1
		fi
		if [[ "${argv[i]}" == "--load-journal" ]]; then
			has_load_journal=1
		elif [[ "${argv[i]}" == "--log-level" ]]; then
			has_log_level=1
		elif [[ "${argv[i]}" == "--log-format" ]]; then
			has_log_format=1
		elif [[ "${argv[i]}" == "--log-filter" ]]; then
			has_log_filter=1
		fi
	done

	globals=()
	if [[ $has_no_color -eq 0 ]]; then
		globals+=("--no-color")
	fi

	if [[ $has_no_cloud -eq 0 ]]; then
		globals+=("--no-cloud")
	fi
	if [[ $has_log_level -eq 0 ]]; then
		globals+=("--log-level" "debug")
	fi
	if [[ $has_log_format -eq 0 ]]; then
		globals+=("--log-format" "json")
	fi
	if [[ $has_log_filter -eq 0 ]]; then
		globals+=("--log-filter" "off,things_cli::cloud_commit::request=debug")
	fi

	if [[ $has_load_journal -eq 0 && -f "journal.json" ]]; then
		globals+=("--load-journal" "journal.json")
	fi

	if [[ ${#globals[@]} -gt 0 ]]; then
		argv=("${argv[0]}" "${globals[@]}" "${argv[@]:1}")
	fi
fi

stderr_file="$(mktemp)"
trap 'rm -f "$stderr_file"' EXIT

set +e
"${argv[@]}" 2>"$stderr_file"
status=$?
set -e

pretty_json="$({ jq -RrS 'fromjson? | select(.event == "cloud.commit.request") | .request_json | fromjson' <"$stderr_file"; } || true)"
if [[ -n "$pretty_json" ]]; then
	printf '%s\n' "$pretty_json" 1>&2
else
	cat "$stderr_file" 1>&2
fi

exit "$status"
