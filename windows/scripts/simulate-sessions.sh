#!/usr/bin/env bash
# Plays fake agent sessions through the real relay, from WSL, so the island can
# be checked by eye without driving the agents by hand. Nothing is edited or
# run: the events only describe edits and commands on files of this repo.
#
#   windows/scripts/simulate-sessions.sh [two-claude|agents|all]
#
# Needs Coucou running on Windows. No permission requests are sent: those wait
# for an answer.
set -euo pipefail

SCENARIO="${1:-all}"
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
DISTRO="${WSL_DISTRO_NAME:-Ubuntu}"
# The Windows user that has Coucou's relay (COUCOU_WIN_USER picks one).
WIN_USER="${COUCOU_WIN_USER:-}"
if [ -z "$WIN_USER" ]; then
  for d in /mnt/c/Users/*/; do
    u="$(basename "$d")"
    if [ -x "/mnt/c/Users/$u/AppData/Local/Coucou/bin/coucou-hook.exe" ]; then WIN_USER="$u"; break; fi
  done
fi
HOOK="${COUCOU_HOOK:-/mnt/c/Users/$WIN_USER/AppData/Local/Coucou/bin/coucou-hook.exe}"
[ -x "$HOOK" ] || { echo "Relay not found: $HOOK" >&2; exit 1; }

# send <agent|""> <session> <event> [json fields without braces]
send() {
  local agent="$1" sid="$2" event="$3" extra="${4:-}"
  local args=(--wsl "$DISTRO")
  [ -n "$agent" ] && args+=(--agent "$agent")
  printf '{"hook_event_name":"%s","session_id":"%s","cwd":"%s"%s}\n' \
    "$event" "$sid" "$REPO" "${extra:+,$extra}" | "$HOOK" "${args[@]}" "$event" >/dev/null || true
}

say() { printf '\n\033[1;36m▶ %s\033[0m\n' "$*"; }
LOG="/mnt/c/Users/$WIN_USER/AppData/Local/Coucou/coucou.log"
# Waits until the island has drawn <view> for <pill> (its "view" log line).
wait_for_view() {
  local view="$1" pill="$2" limit="$3" from
  from=$(wc -l < "$LOG")
  printf '  waiting for the %s view of %s (up to %ss)…' "$view" "$pill" "$limit"
  for ((i = 0; i < limit; i++)); do
    if tail -n +"$((from + 1))" "$LOG" | grep -aq -- "-> $view focus=$pill "; then echo " seen"; return 0; fi
    sleep 1
  done
  echo " not seen, going on"
}
wait_for() { local s="$1"; shift; printf '  %s ' "$*"; for ((i = s; i > 0; i--)); do printf '%s ' "$i"; sleep 1; done; echo; }

# A turn: a prompt, a read, an edit (diff and code view) and a command.
# Claude Code's tool names; the relay renames the other agents' (normalize.rs).
turn() {
  local agent="$1" sid="$2" edit_tool="$3" bash_tool="$4" edit_input="$5"
  send "$agent" "$sid" UserPromptSubmit '"prompt":"simulated turn"'
  send "$agent" "$sid" PreToolUse "\"tool_name\":\"$edit_tool\",\"tool_input\":$edit_input"
  sleep 1
  send "$agent" "$sid" PostToolUse "\"tool_name\":\"$edit_tool\",\"tool_input\":$edit_input"
  send "$agent" "$sid" PreToolUse "\"tool_name\":\"$bash_tool\",\"tool_input\":{\"command\":\"npm test\"}"
  sleep 1
  send "$agent" "$sid" PostToolUse "\"tool_name\":\"$bash_tool\",\"tool_input\":{\"command\":\"npm test\"},\"tool_response\":\"ok 1 pills\\nok 2 hooks\\n# pass 431\\n# fail 0\""
}

CLAUDE_EDIT="{\"file_path\":\"$REPO/windows/package.json\",\"old_string\":\"\\\"version\\\": \\\"0.4.0\\\"\",\"new_string\":\"\\\"version\\\": \\\"0.4.0\\\"\"}"
PI_EDIT="{\"path\":\"$REPO/windows/package.json\",\"edits\":[{\"oldText\":\"\\\"name\\\": \\\"coucou-windows\\\"\",\"newText\":\"\\\"name\\\": \\\"coucou-windows\\\"\"}]}"
OPENCODE_EDIT="{\"filePath\":\"$REPO/windows/package.json\",\"oldString\":\"\\\"version\\\": \\\"0.4.0\\\"\",\"newString\":\"\\\"version\\\": \\\"0.4.0\\\"\"}"

two_claude() {
  say "Two Claude Code sessions at once (sim-alpha, sim-beta)"
  send "" sim-alpha SessionStart
  send "" sim-beta SessionStart
  turn "" sim-alpha Edit Bash "$CLAUDE_EDIT"
  turn "" sim-beta Edit Bash "$CLAUDE_EDIT"
  say "NOW: put sim-alpha's pill in front and click its card to open the code view"
  wait_for_view code "integration_claude~sim-alpha" 120
  wait_for 2 "sim-alpha stops in"
  send "" sim-alpha Stop '"last_assistant_message":"sim-alpha finished its turn."'
  say "Expected: the 'finished' card (Open terminal / OK) over the code view"
  wait_for 8 "sim-beta stops in"
  send "" sim-beta Stop '"last_assistant_message":"sim-beta finished its turn."'
  wait_for 6 "ending both sessions in"
  send "" sim-alpha SessionEnd
  send "" sim-beta SessionEnd
}

agents() {
  say "Pi, ORCA and OpenCode at once"
  send pi sim-pi SessionStart
  send orca sim-orca SessionStart
  send opencode sim-opencode SessionStart
  turn pi sim-pi edit bash "$PI_EDIT"
  turn orca sim-orca edit bash "$PI_EDIT"
  turn opencode sim-opencode edit bash "$OPENCODE_EDIT"
  say "NOW: open each one's code view; the label must read Pi, ORCA, OpenCode"
  wait_for 15 "they stop in"
  send pi sim-pi Stop '"message":"sim-pi done."'
  send orca sim-orca Stop '"message":"sim-orca done."'
  send opencode sim-opencode Stop '"message":"sim-opencode done."'
  wait_for 6 "ending the sessions in"
  for s in pi orca opencode; do send "$s" "sim-$s" SessionEnd; done
}

case "$SCENARIO" in
  two-claude) two_claude ;;
  agents) agents ;;
  all) two_claude; agents ;;
  *) echo "usage: $0 [two-claude|agents|all]" >&2; exit 2 ;;
esac
say "Done. The island's log: grep -a 'stop \\|view ' /mnt/c/Users/$WIN_USER/AppData/Local/Coucou/coucou.log | tail"
