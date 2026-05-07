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
# OS-specific path matches the daemon's `directories` crate resolution:
#   - Darwin: `~/Library/Application Support/telayd/daemon.sock`
#   - Linux/other: `~/.config/telayd/daemon.sock` (XDG)
# Hardcoding the Linux-style path on macOS caused hook-to-daemon silent
# disconnect (host-env drift, dogfooding-discovered).
case "$(uname -s)" in
  Darwin)
    SOCK="${HOME}/Library/Application Support/telayd/daemon.sock"
    ;;
  *)
    SOCK="${HOME}/.config/telayd/daemon.sock"
    ;;
esac

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

# IG-r2-2 fix: replace `timeout 2 cat -` with a bash 3.2-compatible
# `read -r -t` loop.  `timeout(1)` is a GNU coreutils binary absent on stock
# macOS; under `set -euo pipefail` the missing command causes an immediate
# non-zero exit before IPC emit → hook silently dies (SH-1 / B-3 regression).
#
# Strategy: read stdin line-by-line with `read -r -t 2`.
#   - `-t 2` applies a per-read 2-second timeout (POSIX read timeout,
#     supported on macOS bash 3.2 since bash 2.x).
#   - First iteration: 2s budget for the initial byte (stdin from Claude Code
#     arrives almost instantly; 2s is generous).
#   - Subsequent iterations: 0.1s timeout so that the loop exits promptly once
#     stdin EOF is reached (no blocking on the last line).
#   - Accumulates into PAYLOAD_RAW (raw, possibly multi-line).
#   - 256 KiB guard: stop accumulating once character count exceeds 262144.
#   - `tr -d '\n'` (applied after the loop) collapses newlines → single-line
#     wire format expected by the daemon IPC parser.
#
# Failure modes (all safe):
#   - Stdin delivers 0 bytes within 2s → PAYLOAD_RAW empty → exit 0 below.
#   - Claude Code sends compact single-line JSON → loop reads 1 line, exits.
#   - Claude Code sends pretty-printed JSON → loop reads N lines, exits on EOF.
#   - No GNU coreutils needed; no jq dependency.
PAYLOAD_RAW=""
# Read the first line with a 2s timeout (guards against hung stdin).
# `-t 2` is supported by bash 3.2 (macOS default) — POSIX read timeout.
_line=""
if IFS= read -r -t 2 _line; then
  PAYLOAD_RAW="${_line}
"
  # Read remaining lines without a timeout — stdin is a pipe from Claude Code
  # so EOF arrives as soon as the JSON payload ends (no blocking).
  # The 256 KiB guard prevents unbounded accumulation.
  while IFS= read -r _line; do
    PAYLOAD_RAW="${PAYLOAD_RAW}${_line}
"
    if [ "${#PAYLOAD_RAW}" -ge 262144 ]; then
      break
    fi
  done
fi

# Collapse newlines → single-line wire format (daemon IPC expects newline-terminated line).
PAYLOAD="$(printf '%s' "${PAYLOAD_RAW}" | tr -d '\n')" || true

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

# ── Extract current tmux session name and inject into payload ────────────────
# Architecture intent (ipc.rs §16 comment): wire format includes `tmux_session`
# so the daemon dispatches keystrokes to the originating session, not a hardcoded
# fallback. Without this, daemon's `tmux send-keys` targets a non-existent session
# and `inject failed` warnings flood the log (dogfooding-discovered).
#
# `tmux display-message -p '#{session_name}'` runs against the inherited TMUX
# socket (no -L/-S needed: the env var TMUX selects the socket). When the hook
# fires outside tmux (e.g. `claude` in a plain terminal), `$TMUX` is unset and
# tmux returns non-zero → empty TMUX_SESSION_NAME → daemon falls back to its
# startup default. Either way safe for the Claude session.
TMUX_SESSION_NAME=""
if [ -n "${TMUX:-}" ]; then
  TMUX_SESSION_NAME="$(tmux display-message -p '#{session_name}' 2>/dev/null || true)"
fi

# Inject the field into the JSON object. We work on the already-validated
# single-line payload that starts with '{'. Insert `"tmux_session":"<name>",`
# right after the opening brace — JSON allows trailing commas only in
# permissive parsers, but inserting at the *front* of the object body produces
# valid JSON for any conforming parser (serde_json on the daemon side).
#
# Escape: tmux session names may legally contain `:` and `.` but never `"` or
# `\` per tmux defaults; still, defensive escape `"` and `\` so a future
# weird name can't break the JSON.
if [ -n "${TMUX_SESSION_NAME}" ]; then
  ESCAPED="$(printf '%s' "${TMUX_SESSION_NAME}" | sed 's/\\/\\\\/g; s/"/\\"/g')"
  # PAYLOAD starts with '{'. Replace the first '{' with '{"tmux_session":"<name>",'.
  # `sed` 1-occurrence replace via parameter expansion (bash 3.2-safe).
  PAYLOAD="{\"tmux_session\":\"${ESCAPED}\",${PAYLOAD#\{}"
fi

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
