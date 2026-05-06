#!/usr/bin/env bash
# Smoke tests for telayd-hook-emit.sh
#
# Bats-style tests (manual — no bats dependency required).
# Run: bash telayd-hook-emit.test.sh
#
# Tests:
#   P0: daemon-up case (fake socket listener)
#   P0: daemon-down case (socket missing, graceful exit)
#   P1: stdin parse fail (non-JSON), exit 0

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOOK_SCRIPT="${SCRIPT_DIR}/telayd-hook-emit.sh"

PASS=0
FAIL=0

run_test() {
  local name="$1"
  local result="$2"  # "pass" or "fail"
  if [ "${result}" = "pass" ]; then
    echo "  [PASS] ${name}"
    PASS=$((PASS + 1))
  else
    echo "  [FAIL] ${name}"
    FAIL=$((FAIL + 1))
  fi
}

TMPDIR_TEST="$(mktemp -d /tmp/telayd-test-XXXXXX)"
trap 'rm -rf "${TMPDIR_TEST}"' EXIT

FAKE_SOCK="${TMPDIR_TEST}/daemon.sock"

VALID_PAYLOAD='{"session_id":"abc123","hook_event_name":"PreToolUse","tool_name":"AskUserQuestion","tool_use_id":"toolu_TEST123","tool_input":{"questions":[{"question":"Q?","options":[{"label":"A"}],"multi_select":false}]}}'

# ── Test 1: daemon-down → exit 0 ─────────────────────────────────────────────
echo "Test 1: daemon-down case"
# No socket at TMPDIR_TEST/daemon.sock
result="pass"
HOME="${TMPDIR_TEST}" bash "${HOOK_SCRIPT}" <<< "${VALID_PAYLOAD}" || result="fail"
# Must exit 0 even when daemon is absent.
exit_code=0
HOME="${TMPDIR_TEST}" bash "${HOOK_SCRIPT}" <<< "${VALID_PAYLOAD}"; exit_code=$?
if [ "${exit_code}" -eq 0 ]; then
  run_test "daemon-down exits 0" "pass"
else
  run_test "daemon-down exits 0" "fail"
fi

# ── Test 2: daemon-up case (fake nc listener) ─────────────────────────────────
echo "Test 2: daemon-up case"
# Start a minimal fake Unix socket listener using nc.
# nc -lU <path> on macOS listens on a Unix socket and accepts one connection.
RECEIVED_FILE="${TMPDIR_TEST}/received.json"
# Background nc listener — writes received data to file.
nc -lU "${FAKE_SOCK}" > "${RECEIVED_FILE}" 2>/dev/null &
NC_PID=$!
sleep 0.2  # Let nc bind.

exit_code=0
HOME="${TMPDIR_TEST}" bash "${HOOK_SCRIPT}" <<< "${VALID_PAYLOAD}"; exit_code=$?
wait "${NC_PID}" 2>/dev/null || true

if [ "${exit_code}" -eq 0 ]; then
  run_test "daemon-up exits 0" "pass"
else
  run_test "daemon-up exits 0" "fail"
fi

if [ -s "${RECEIVED_FILE}" ]; then
  run_test "daemon-up received payload" "pass"
else
  run_test "daemon-up received payload" "fail"
fi

# ── Test 3: stdin parse fail → exit 0 ────────────────────────────────────────
echo "Test 3: stdin non-JSON"
exit_code=0
HOME="${TMPDIR_TEST}" bash "${HOOK_SCRIPT}" <<< "not-json-at-all"; exit_code=$?
if [ "${exit_code}" -eq 0 ]; then
  run_test "stdin-non-json exits 0" "pass"
else
  run_test "stdin-non-json exits 0" "fail"
fi

# ── Test 4: empty stdin → exit 0 ─────────────────────────────────────────────
echo "Test 4: empty stdin"
exit_code=0
HOME="${TMPDIR_TEST}" bash "${HOOK_SCRIPT}" < /dev/null; exit_code=$?
if [ "${exit_code}" -eq 0 ]; then
  run_test "empty-stdin exits 0" "pass"
else
  run_test "empty-stdin exits 0" "fail"
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "Results: ${PASS} passed, ${FAIL} failed"
if [ "${FAIL}" -gt 0 ]; then
  exit 1
fi
exit 0
