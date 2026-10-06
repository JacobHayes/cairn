# shellcheck shell=bash
# Sourced by every rung of the validation ladder (PRACTICES, The validation ladder).
#
# A rung reports how many things each of its suites checked or ran, through
# `ladder_suite`, and fails when any suite reports none, so one suite vanishing cannot hide
# behind another's count. A rung that reports no suite at all fails too (`ladder_rung_end`).
# A missing tool fails the rung rather than skipping it (`ladder_require_tool`).
#
# The caller sets LADDER_RUNG (its number) before sourcing this file. Every line meant for
# the ladder's summary starts with "ladder: rung N:", which scripts/ladder collects.

: "${LADDER_RUNG:?set LADDER_RUNG before sourcing scripts/ladder-lib.sh}"
ladder_suite_count=0

ladder_say() {
  printf 'ladder: rung %s: %s\n' "$LADDER_RUNG" "$*"
}

ladder_fail() {
  ladder_say "FAILED: $*" >&2
  exit 1
}

# ladder_require_tool NAME COMMAND...: fail the rung unless COMMAND (a version query) runs.
ladder_require_tool() {
  local name=$1
  shift
  "$@" >/dev/null 2>&1 || ladder_fail "tool missing: $name ('$*' did not run)"
}

# ladder_suite NAME COUNT UNIT: record that suite NAME checked or ran COUNT UNITs.
ladder_suite() {
  local name=$1 count=$2 unit=$3
  ladder_suite_count=$((ladder_suite_count + 1))
  [ "$count" -gt 0 ] || ladder_fail "suite $name checked or ran 0 $unit"
  ladder_say "suite $name: $count $unit"
}

# ladder_rung_end: call last; a rung that reported no suite checked nothing.
ladder_rung_end() {
  [ "$ladder_suite_count" -gt 0 ] || ladder_fail "reported no suites"
  ladder_say "passed"
}
