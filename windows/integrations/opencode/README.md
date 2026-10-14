# OpenCode in WSL

Settings → Agents → OpenCode writes Coucou's plugin for OpenCode on the
Windows side (`%USERPROFILE%\.config\opencode\plugins\coucou.js`). OpenCode
running inside WSL reads `~/.config/opencode/plugins/` in the distro instead,
so it never sees that plugin and its sessions never reach Coucou.

`coucou.js` here is the same plugin for OpenCode in WSL. It finds the relay
Coucou installed on Windows (`/mnt/c/Users/<user>/AppData/Local/Coucou/bin/coucou-hook.exe`,
the first user that has it; set `COUCOU_WIN_USER` to choose) and passes
`--wsl $WSL_DISTRO_NAME`, so the session's folder and files reach Windows as
`\\wsl$\<distro>\…` paths the code view can read.

## Install

```sh
cp windows/integrations/opencode/coucou.js ~/.config/opencode/plugins/coucou.js
```

Restart OpenCode afterwards. Keep it in step with `OPENCODE_PLUGIN` in
`src-tauri/src/agents.rs` when that changes.
