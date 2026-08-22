#!/usr/bin/env bash
# verify_all.sh -- one-command verification orchestrator for the HP2 tree.
#
# Usage: Build/verify_all.sh [preset] [--with-sanitizers] [--with-full-smoke]
#
# Stage sequence:
#   1. configure <preset>                       (cmake --preset)
#   2. build                                    (hp2_verification_binaries)
#   3. ctest <preset>
#   4. --with-sanitizers: macos-arm64-asan-ubsan then macos-arm64-tsan,
#      each as configure -> build -> ctest. The presets use disjoint binary
#      dirs (out/<preset>), so sanitizer runs never mix state.
#   5. --with-full-smoke: macos-arm64-full-smoke preset, i.e.
#      configure -> build -> ctest -L smoke-full (renderer_smoke_full sweep).
#
# Every stage's combined output replaces out/<preset>/verify-<stage>.log.
# A failing stage aborts the run immediately; the final summary lists every
# recorded stage result and where reports/artifacts land. Exit status is
# nonzero if any selected stage failed.

set -Eeuo pipefail

_usage() {
    cat <<'EOF'
Usage: Build/verify_all.sh [preset] [--with-sanitizers] [--with-full-smoke]

  preset             CMake preset to verify (default: macos-arm64)
  --with-sanitizers  also verify macos-arm64-asan-ubsan, then macos-arm64-tsan
  --with-full-smoke  also verify the macos-arm64-full-smoke preset
                     (configure + build + ctest -L smoke-full)
  -h, --help         show this help

Logs: out/<preset>/verify-<stage>.log   Reports: out/<tree>/Testing/
EOF
}

_die() {
    printf 'verify_all: error: %s\n' "$1" >&2
    exit 2
}

# --- argument parsing -------------------------------------------------------
preset=''
with_sanitizers=0
with_full_smoke=0

while (($# > 0)); do
    case "$1" in
        --with-sanitizers) with_sanitizers=1 ;;
        --with-full-smoke) with_full_smoke=1 ;;
        -h | --help)
            _usage
            exit 0
            ;;
        -*)
            printf 'verify_all: unknown option: %s\n' "$1" >&2
            _usage >&2
            exit 2
            ;;
        *)
            [[ -z "$preset" ]] || _die "unexpected extra argument: $1"
            preset="$1"
            ;;
    esac
    shift
done

[[ -n "$preset" ]] || preset='macos-arm64'

# --- environment guards -----------------------------------------------------
_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly _script_dir
_repo_root="$(cd "${_script_dir}/.." && pwd -P)"
readonly _repo_root

if [[ "$(pwd -P)" != "${_repo_root}" ]]; then
    _die "run from the repository root: cd '${_repo_root}' && Build/verify_all.sh ..."
fi

readonly log_dir="out/${preset}"
mkdir -p "${log_dir}"

# --- stage bookkeeping ------------------------------------------------------
stage_names=()
stage_results=()
stage_exit_codes=()

_record() { # name exit_code
    stage_names+=("$1")
    if (($2 == 0)); then
        stage_results+=('pass')
    else
        stage_results+=('FAIL')
    fi
    stage_exit_codes+=("$2")
}

_run_stage() { # name command [args...]
    local name="$1"
    shift
    local log="${log_dir}/verify-${name}.log"
    local started=$SECONDS
    local rc=0

    printf '\n===== stage [%s]: %s =====\n' "${name}" "$*"
    set +e
    "$@" 2>&1 | tee "${log}"
    rc=${PIPESTATUS[0]}
    set -e

    _record "${name}" "${rc}"
    printf -- '----- stage [%s] done: exit=%d elapsed=%ds -----\n' \
        "${name}" "${rc}" "$((SECONDS - started))"
    return "${rc}"
}

_abort() { # failed_stage_name
    printf '\nverify_all: ABORTED at stage "%s"; remaining stages skipped.\n' "$1"
    _summary
    exit 1
}

_summary() {
    printf '\n========================= verify_all summary =========================\n'
    if ((${#stage_names[@]} == 0)); then
        printf 'no stages completed\n'
    fi
    local i
    for i in "${!stage_names[@]}"; do
        printf '  %-34s %-4s (exit %s)\n' \
            "${stage_names[$i]}" "${stage_results[$i]}" "${stage_exit_codes[$i]}"
    done
    printf '\nStage logs     : %s/verify-<stage>.log\n' "${log_dir}"
    printf 'CTest/CDash    : out/<preset-binary-dir>/Testing/\n'
    printf 'Smoke reports  : out/<preset-binary-dir>/Testing/HP2/renderer_smoke_*/smoke-maps.json\n'
}

_on_interrupt() {
    printf '\nverify_all: interrupted by user; no partial state left behind.\n' >&2
    _summary >&2
    exit 130
}
trap _on_interrupt INT

# --- stage sequences --------------------------------------------------------
_verify_tree() { # prefix cmake_preset  (runs configure/build/ctest for one preset)
    local prefix="$1"
    local tree_preset="$2"
    local b="${prefix:+${prefix}-}"

    _run_stage "${b}configure" cmake --preset "${tree_preset}" ||
        _abort "${b}configure"
    _run_stage "${b}build" cmake --build --preset "${tree_preset}" \
        --target hp2_verification_binaries ||
        _abort "${b}build"
    _run_stage "${b}test" ctest --preset "${tree_preset}" ||
        _abort "${b}test"
}

printf 'verify_all: preset=%s sanitizers=%s full_smoke=%s\n' \
    "${preset}" "${with_sanitizers}" "${with_full_smoke}"

_verify_tree '' "${preset}"

if ((with_sanitizers)); then
    # Sequential on purpose; each preset owns a disjoint binary dir.
    _verify_tree 'san-asan-ubsan' 'macos-arm64-asan-ubsan'
    _verify_tree 'san-tsan' 'macos-arm64-tsan'
fi

if ((with_full_smoke)); then
    _run_stage 'full-smoke-configure' cmake --preset 'macos-arm64-full-smoke' ||
        _abort 'full-smoke-configure'
    _run_stage 'full-smoke-build' cmake --build --preset 'macos-arm64-full-smoke' \
        --target hp2_verification_binaries ||
        _abort 'full-smoke-build'
    _run_stage 'full-smoke-test' ctest --preset 'macos-arm64-full-smoke' -L 'smoke-full' ||
        _abort 'full-smoke-test'
fi

_summary

for i in "${!stage_results[@]}"; do
    if [[ "${stage_results[$i]}" != 'pass' ]]; then
        exit 1
    fi
done
exit 0
