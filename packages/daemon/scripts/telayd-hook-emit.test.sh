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

# The hook script uses $HOME/.config/telayd/daemon.sock.
# To override it with our fake socket, we set HOME to a test home dir and
# create the expected path inside it.
FAKE_HOME="${TMPDIR_TEST}/home"
mkdir -p "${FAKE_HOME}/.config/telayd"
FAKE_SOCK="${FAKE_HOME}/.config/telayd/daemon.sock"

VALID_PAYLOAD='{"session_id":"abc123","hook_event_name":"PreToolUse","tool_name":"AskUserQuestion","tool_use_id":"toolu_TEST123","tool_input":{"questions":[{"question":"Q?","options":[{"label":"A"}],"multi_select":false}]}}'

# ── Test 1: daemon-down → exit 0 ─────────────────────────────────────────────
echo "Test 1: daemon-down case"
# Socket file does not exist → daemon-absent guard fires → exit 0.
exit_code=0
HOME="${FAKE_HOME}" bash "${HOOK_SCRIPT}" <<< "${VALID_PAYLOAD}"; exit_code=$?
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
# Background nc listener — writes received data to file, then exits.
# The hook uses nc -w 1 (1s connect timeout) to send and disconnect.
nc -lU "${FAKE_SOCK}" > "${RECEIVED_FILE}" 2>/dev/null &
NC_PID=$!
sleep 0.3  # Let nc bind before hook connects.

exit_code=0
HOME="${FAKE_HOME}" bash "${HOOK_SCRIPT}" <<< "${VALID_PAYLOAD}"; exit_code=$?
# Give nc 2s to finish writing after the connection closes.
sleep 0.5
kill "${NC_PID}" 2>/dev/null || true
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
HOME="${FAKE_HOME}" bash "${HOOK_SCRIPT}" <<< "not-json-at-all"; exit_code=$?
if [ "${exit_code}" -eq 0 ]; then
  run_test "stdin-non-json exits 0" "pass"
else
  run_test "stdin-non-json exits 0" "fail"
fi

# ── Test 4: empty stdin → exit 0 ─────────────────────────────────────────────
echo "Test 4: empty stdin"
exit_code=0
HOME="${FAKE_HOME}" bash "${HOOK_SCRIPT}" < /dev/null; exit_code=$?
if [ "${exit_code}" -eq 0 ]; then
  run_test "empty-stdin exits 0" "pass"
else
  run_test "empty-stdin exits 0" "fail"
fi

# ── Test 5 (IG-r2-2): hook script must NOT invoke `timeout` as a command ──────
# Regression guard: grep for non-comment lines that start with or call `timeout `.
# `timeout(1)` is a GNU coreutils binary absent on stock macOS — any invocation
# under `set -euo pipefail` would cause a silent abort before IPC emit.
# We exclude comment lines (lines whose first non-whitespace char is '#').
echo "Test 5 (IG-r2-2): hook script contains no executable 'timeout' command"
# Extract non-comment, non-empty lines that contain the word 'timeout' as a command.
# A command invocation looks like: `timeout <args>` at the start of a statement,
# possibly after `|` or `$(`. We grep for lines where `timeout` appears NOT inside
# a comment — i.e., the line does not start with optional whitespace + '#'.
non_comment_timeout="$(grep -n 'timeout' "${HOOK_SCRIPT}" | grep -v '^[0-9]*:[[:space:]]*#' || true)"
if [ -n "${non_comment_timeout}" ]; then
  # Further filter: look for lines where 'timeout' appears as a standalone command
  # (not as a word inside a string like "read timeout" or variable name).
  # Pattern: word boundary — `timeout ` with space after, or `\btimeout\b` call context.
  cmd_hits="$(echo "${non_comment_timeout}" | grep -E '(^[0-9]+:[[:space:]]*(|[^#]*[|;$`([:space:]])timeout[[:space:]])' || true)"
  if [ -n "${cmd_hits}" ]; then
    echo "  [FAIL] Found 'timeout' command invocation in non-comment lines:"
    echo "${cmd_hits}"
    run_test "no-timeout-command-in-hook-script" "fail"
  else
    run_test "no-timeout-command-in-hook-script" "pass"
  fi
else
  run_test "no-timeout-command-in-hook-script" "pass"
fi

# ── Test 6 (IG-r2-2): multi-line (pretty-printed) JSON → single-line payload ──
# Verifies the read-loop correctly collapses newlines into the single-line
# wire format the daemon IPC parser expects.
echo "Test 6 (IG-r2-2): multi-line JSON stdin → single-line payload to daemon"

# Remove stale socket from Test 2 so nc can rebind.
rm -f "${FAKE_SOCK}"
sleep 0.1

# Start fake socket listener on a fresh socket.
RECEIVED_ML="${TMPDIR_TEST}/received_ml.json"
nc -lU "${FAKE_SOCK}" > "${RECEIVED_ML}" 2>/dev/null &
NC_PID2=$!
sleep 0.3  # Let nc bind before hook connects.

# Feed pretty-printed JSON (split across lines).
printf '{\n  "session_id": "xyz",\n  "hook_event_name": "PreToolUse",\n  "tool_name": "AskUserQuestion",\n  "tool_use_id": "toolu_ML",\n  "tool_input": {"questions": [{"question": "Which?", "options": [{"label": "A"}, {"label": "B"}], "multi_select": false}]}\n}\n' \
  | HOME="${FAKE_HOME}" bash "${HOOK_SCRIPT}"
sleep 0.5
kill "${NC_PID2}" 2>/dev/null || true
wait "${NC_PID2}" 2>/dev/null || true

if [ -s "${RECEIVED_ML}" ]; then
  # The received line must not contain a raw newline (wire format = single line).
  line_count="$(wc -l < "${RECEIVED_ML}" | tr -d ' ')"
  if [ "${line_count}" -le 1 ]; then
    run_test "multi-line JSON collapsed to single-line wire format" "pass"
  else
    echo "  [FAIL] Received ${line_count} lines (expected 1)"
    run_test "multi-line JSON collapsed to single-line wire format" "fail"
  fi
else
  run_test "multi-line JSON payload received by daemon (non-empty)" "fail"
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "Results: ${PASS} passed, ${FAIL} failed"
if [ "${FAIL}" -gt 0 ]; then
  exit 1
fi
exit 0
