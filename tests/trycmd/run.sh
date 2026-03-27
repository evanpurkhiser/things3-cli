#!/usr/bin/env bash
set -euo pipefail

# Wrapper for trycmd cases.
#
# Default behavior: if ./journal.json exists and --load-journal is not
# explicitly provided, append --load-journal journal.json for things3.
# Optional behavior: pretty-print cloud commit request payloads from stderr.
# Enable with TRYCMD_PRETTY_DEBUG_JSON=1.

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

	if [[ "${TRYCMD_MUTATION_LOG:-0}" == "1" ]]; then
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
	fi

	if [[ $has_load_journal -eq 0 && -f "journal.json" ]]; then
		globals+=("--load-journal" "journal.json")
	fi

	if [[ ${#globals[@]} -gt 0 ]]; then
		argv=("${argv[0]}" "${globals[@]}" "${argv[@]:1}")
	fi
fi

if [[ "${TRYCMD_PRETTY_DEBUG_JSON:-0}" != "1" ]]; then
	exec "${argv[@]}"
fi

stderr_file="$(mktemp)"
trap 'rm -f "$stderr_file"' EXIT

set +e
"${argv[@]}" 2>"$stderr_file"
status=$?
set -e

jq -rS '
  select(.event == "cloud.commit.request")
  | .request_json
  | fromjson
' <"$stderr_file" 1>&2 || true

exit "$status"
