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

# ladder_cargo_test ARGS...: `cargo test` over the whole workspace with every feature, ARGS
# being the target selection and then `--` and the test filters. Prints the run's output and
# sets `ladder_tests` to one "package<TAB>test<TAB>result" line per test it ran. Every rung
# runs its Rust tests in this one shape. Cargo resolves features per invocation, so a run
# scoped to one crate (`-p`) compiles its own copy of every dependency whose features differ
# and links its own test binaries; run this way, the workspace's tests are built once and
# each rung runs the ones it picks by name. Returns cargo's status.
ladder_cargo_test() {
  local output status=0
  output=$(cargo test --locked --workspace --all-features --message-format=json-render-diagnostics "$@" 2>&1) \
    || status=$?
  grep -v '^{"reason":' <<<"$output" || true
  # The package of each test binary comes from cargo's artifact messages; a run's output
  # names the binary before its tests ("Running ... (<binary>)", "Doc-tests <crate>").
  # shellcheck disable=SC2016 # the backticks are a JavaScript template literal
  ladder_tests=$(node -e '
    const path = require("path");
    let text = "";
    process.stdin.on("data", (chunk) => (text += chunk)).on("end", () => {
      // A package id is "path+file:///<dir>#<name>@<version>", or "...<dir>#<version>" when
      // the name is the directory'"'"'s.
      const nameOf = (id) => {
        const [location, fragment] = id.split("#");
        return fragment.includes("@") ? fragment.split("@")[0] : path.basename(location);
      };
      const binaries = new Map();
      const crates = new Map();
      let current = null;
      for (const line of text.split("\n")) {
        if (line.startsWith("{\"reason\":")) {
          const message = JSON.parse(line);
          if (message.reason !== "compiler-artifact") continue;
          const name = nameOf(message.package_id);
          crates.set(name.replaceAll("-", "_"), name);
          if (message.executable) binaries.set(path.basename(message.executable), name);
          continue;
        }
        const running = /^\s+Running .* \((.+)\)$/.exec(line);
        if (running) { current = binaries.get(path.basename(running[1])) ?? `unknown binary ${running[1]}`; continue; }
        const doc = /^\s+Doc-tests (\S+)$/.exec(line);
        if (doc) { current = crates.get(doc[1]) ?? `unknown crate ${doc[1]}`; continue; }
        const test = /^test (.+) \.\.\. (\S+)/.exec(line);
        if (test && current !== null) console.log(`${current}\t${test[1]}\t${test[2]}`);
      }
    });' <<<"$output")
  return "$status"
}

# ladder_tests_matching PACKAGE REGEX: how many of the last ladder_cargo_test run's tests
# belong to PACKAGE and have a name matching REGEX (an awk extended regular expression).
ladder_tests_matching() {
  awk -F'\t' -v package="$1" -v pattern="$2" '$1 == package && $2 ~ pattern { count++ } END { print count + 0 }' <<<"$ladder_tests"
}
