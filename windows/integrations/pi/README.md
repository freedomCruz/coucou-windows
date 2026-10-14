# Pi extension for Coucou

`pi-coucou.ts` reports a [Pi](https://github.com/earendil-works/pi-coding-agent)
session's lifecycle to Coucou: session start and end, prompts, tool calls and
the end of each turn. It runs `coucou-hook` (the relay in `windows/hook/`) with
the event as JSON, so Coucou needs nothing installed for Pi (`agent_pi`
uses `connect: none` in `src/core/pills.ts`).

This file is the source of truth. The copies Pi loads are installed from it.

## Install

Pi loads extensions from `~/.pi/agent/extensions/`. Pi under WSL and Pi on
Windows have separate homes, so copy it to both:

```sh
# Pi in WSL (WezTerm, herdr…)
cp windows/integrations/pi/pi-coucou.ts ~/.pi/agent/extensions/
# Pi on Windows (Windows Terminal, ORCA…)
cp windows/integrations/pi/pi-coucou.ts /mnt/c/Users/<you>/.pi/agent/extensions/
```

Start a new Pi session afterwards: running ones keep the old copy.

## How it finds the relay

1. On Windows: `%LOCALAPPDATA%\Coucou\bin\coucou-hook.exe`, then `%LOCALAPPDATA%\Coucou\coucou-hook.exe`.
2. In WSL: `/mnt/c/Users/$USERNAME/AppData/Local/Coucou/bin/coucou-hook.exe`, run
   through WSL interop with `--wsl $WSL_DISTRO_NAME` so paths become `\\wsl$\…`.
   `USERNAME` is usually unset in WSL; the fallback is the author's Windows user
   name, so change it when moving this elsewhere.
3. A local build in `~/documents/projects/coucou/windows/target/release/`.
4. On Linux: `~/.local/share/coucou/bin/coucou-hook`.

No relay found, or Coucou not running: the event is dropped and Pi goes on.

## Which pill

- In ORCA's terminals the session is tagged `orca` and goes on the ORCA pill.
  ORCA sets `ORCA_PANE_KEY` in every terminal it opens (its own Pi integration
  reads the same variable); `ORCA_SESSION` and `ADE=orca` are still accepted.
- Anywhere else it is tagged `pi` and goes on the Pi pill.

## What it sends

Pi's tool calls go as `PreToolUse` and `PostToolUse` (`PostToolUseFailure`
when the tool failed) with Pi's own names and fields: `edit` with `path` and
`edits[].oldText / newText`, `write`, `read`, `bash`. The relay renames them
to Claude Code's (`MultiEdit`, `file_path`, `old_string`…; see
`hook/src/normalize.rs`), so the live diff and the code view work as they do
for Claude Code. A tool's output goes as `tool_response`, its last 4000
characters; the relay keeps only a command's last lines of it.

A pill for an agent that is not declared in Settings → Active pills comes with
the session and goes 5 s after each turn ends. A second session working at the
same time gets a pill of its own.
