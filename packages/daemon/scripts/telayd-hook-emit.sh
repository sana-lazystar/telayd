#!/usr/bin/env bash
# telayd-hook-emit — Claude Code PreToolUse hook script.
#
# Reads hook payload from stdin and forwards it to the telayd daemon
# via Unix socket (~/.config/telayd/daemon.sock).
#
# Security (§5.2 checklist):
#   - bash strict mode: set -euo pipefail
#   - socket connect timeout 1s (NFR-3, B3 mitigation)
#   - daemon-down → exit 0 (Claude session abort forbidden)
#   - stdin parse failure → exit 0 (Claude session protection)
#   - socket path hardcoded from $HOME (no env injection)
#   - tmpfile uses mktemp + trap cleanup
#   - extra fd closed at start
#
# Works on macOS bash 3.2 (POSIX-compatible, no bashisms beyond 3.2).
# shellcheck disable=SC2064
# shellcheck disable=SC2329  # cleanup() is called via `trap ... EXIT` (indirect call)

set -euo pipefail

# ── Close extra file descriptors ─────────────────────────────────────────────
# Leave 0 (stdin), 1 (stdout), 2 (stderr) open; close anything else.
# On bash 3.2 /dev/fd may not enumerate — use explicit range guard.
for _fd in 3 4 5 6 7 8 9; do
  eval "exec ${_fd}>&-" 2>/dev/null || true
done

# ── Socket path (from $HOME, no user-controlled env) ─────────────────────────
SOCK="${HOME}/.config/telayd/daemon.sock"

# ── Daemon-absent guard ───────────────────────────────────────────────────────
# If the socket file doesn't exist, or is a symlink (symlink-redirect attack),
# the daemon is down or the path is tampered with.  Exit 0 immediately.
# (IG6 fix: diagnosis.md §Group6 symlink reject)
# NOTE: [ -L ] tests for symlink before [ ! -S ] so we reject symlinks even
# if they point to a valid socket (macOS bash 3.2 compat: -L is POSIX 1003.2).
if [ -L "${SOCK}" ] || [ ! -S "${SOCK}" ]; then
  exit 0
fi

# ── Read stdin into tempfile ──────────────────────────────────────────────────
TMPFILE=""
cleanup() {
  if [ -n "${TMPFILE}" ] && [ -f "${TMPFILE}" ]; then
    rm -f "${TMPFILE}"
  fi
}
trap 'cleanup' EXIT INT TERM

TMPFILE="$(mktemp /tmp/telayd-hook-XXXXXX)"

# IG9 fix (diagnosis.md §Group9 P2): replace `read -r -t 2 PAYLOAD_LINE` with
# `timeout 2 cat -` so that multi-line (pretty-printed) JSON is read in full.
# `head -c $((256*1024))` caps at 256 KiB before any further processing.
# `tr -d '\n'` collapses all newlines into a single-line payload that the
# daemon IPC parser expects (newline-terminated single-line wire format).
#
# Bash 3.2 (macOS default) compatible — no bashisms, no jq dependency.
# Failure modes:
#   - timeout exits 124 if stdin blocks > 2s → PAYLOAD is empty → exit 0.
#   - Any other failure propagates to the validation check below → exit 0.
PAYLOAD="$(timeout 2 cat - | head -c $((256*1024)) | tr -d '\n')" || true

# Guard: empty read (daemon timeout, closed stdin, or bash 3.2 timeout) → exit 0.
if [ -z "${PAYLOAD}" ]; then
  exit 0
fi

# Validate: payload must start with '{' (minimal JSON guard).
case "${PAYLOAD}" in
  '{'*)
    : # OK
    ;;
  *)
    # Not JSON → exit 0, protect Claude session.
    exit 0
    ;;
esac

# Write compacted single-line payload + newline terminator to tempfile (wire format).
printf '%s\n' "${PAYLOAD}" > "${TMPFILE}"

# ── Send to daemon via Unix socket (1s timeout) ───────────────────────────────
# Use nc (netcat) with -U (Unix socket) and -w 1 (1-second timeout).
# On macOS, nc -U <path> connects to a Unix domain socket.
# Failure (daemon busy / timeout) → exit 0, protect Claude session.
if ! nc -U -w 1 "${SOCK}" < "${TMPFILE}" > /dev/null 2>&1; then
  exit 0
fi

exit 0
