# Claude Code in WSL

Settings → Claude Code installs Coucou's hooks in the Windows
`%USERPROFILE%\.claude\settings.json`. Claude Code running inside WSL reads
`~/.claude/settings.json` in the distro instead, so its sessions never reach
Coucou until the same hooks are added there.

Each hook runs the relay Coucou installed on Windows, through WSL interop,
with `--wsl <distro>` so the session's folder and files reach Windows as
`\\wsl$\<distro>\…` paths (the code view reads them from there):

```json
{ "hooks": [{ "type": "command",
  "command": "'/mnt/c/Users/<you>/AppData/Local/Coucou/bin/coucou-hook.exe' --wsl Ubuntu PreToolUse",
  "timeout": 10 }] }
```

One such group is appended to each of these events, next to whatever hooks
are already there: SessionStart, UserPromptSubmit, Stop, StopFailure,
SubagentStart, SubagentStop, PreToolUse, PostToolUse, PostToolUseFailure,
PermissionRequest (timeout 120: the island's Allow / Deny card waits for you),
SessionEnd and Notification. The timeouts are the ones Coucou writes on Windows.

As for any change to `settings.json`: take a dated backup, merge rather than
replace, look at the diff, and only then write. Restart Claude Code afterwards.
