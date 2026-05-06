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
# If the socket file doesn't exist, the daemon is down.  Exit 0 immediately.
if [ ! -S "${SOCK}" ]; then
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

# Read stdin with a 2-second guard to prevent runaway read.
if ! IFS= read -r -t 2 PAYLOAD_LINE; then
  # Stdin parse/read failure → protect Claude session.
  exit 0
fi

# Validate: payload must start with '{' (minimal JSON guard).
case "${PAYLOAD_LINE}" in
  '{'*)
    : # OK
    ;;
  *)
    # Not JSON → exit 0, protect Claude session.
    exit 0
    ;;
esac

# Write to tempfile (for nc stdin).
printf '%s\n' "${PAYLOAD_LINE}" > "${TMPFILE}"

# ── Send to daemon via Unix socket (1s timeout) ───────────────────────────────
# Use nc (netcat) with -U (Unix socket) and -w 1 (1-second timeout).
# On macOS, nc -U <path> connects to a Unix domain socket.
# Failure (daemon busy / timeout) → exit 0, protect Claude session.
if ! nc -U -w 1 "${SOCK}" < "${TMPFILE}" > /dev/null 2>&1; then
  exit 0
fi

exit 0
