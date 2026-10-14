// Coucou plugin for OpenCode running in WSL, with Coucou on the Windows host.
// Settings -> Agents writes OpenCode's plugin on the Windows side only; this one
// is the same plugin, plus finding the Windows relay from WSL and telling it
// the distro (`--wsl`), so paths reach Windows as \\wsl$\... paths.
// Fire-and-forget: OpenCode never waits on it, and nothing happens when Coucou
// is closed or not found.
import { spawn } from 'node:child_process';
import { existsSync, readdirSync } from 'node:fs';

/** The relay Coucou installs on Windows, seen from WSL: the first user that has it. */
function findHook() {
  const users = process.env.COUCOU_WIN_USER
    ? [process.env.COUCOU_WIN_USER]
    : (() => { try { return readdirSync('/mnt/c/Users'); } catch { return []; } })();
  for (const user of users) {
    const hook = `/mnt/c/Users/${user}/AppData/Local/Coucou/bin/coucou-hook.exe`;
    if (existsSync(hook)) return hook;
  }
  return null;
}

const HOOK = findHook();
const DISTRO = process.env.WSL_DISTRO_NAME || '';
const EVENT_MAP = {
  'session.created': 'SessionStart',
  'session.idle': 'Stop',
  'session.error': 'StopFailure',
  'session.deleted': 'SessionEnd',
};

function forward(hook_event_name, payload) {
  if (!HOOK) return;
  try {
    const args = ['--agent', 'opencode', ...(DISTRO ? ['--wsl', DISTRO] : [])];
    const p = spawn(HOOK, args, { stdio: ['pipe', 'ignore', 'ignore'], detached: true });
    p.on('error', () => {});
    p.stdin.on('error', () => {});
    p.stdin.end(JSON.stringify({ hook_event_name, ...payload }) + '\n');
    p.unref();
  } catch {}
}

export const CoucouPlugin = async ({ directory } = {}) => ({
  event: async ({ event }) => {
    const hook_event_name = EVENT_MAP[event?.type];
    if (!hook_event_name) return;
    const props = event.properties || {};
    forward(hook_event_name, {
      session_id: props.sessionID || props.info?.id || '',
      cwd: directory || '',
    });
  },
  'tool.execute.before': async (input, output) => {
    forward('PreToolUse', {
      session_id: input?.sessionID || '',
      cwd: directory || '',
      tool_name: typeof input?.tool === 'string' ? input.tool : '',
      tool_input: output?.args ?? null,
    });
  },
  // The relay gives OpenCode's tools Claude Code's names, keeps a command's
  // last lines of its output and nothing of any other tool's.
  'tool.execute.after': async (input, output) => {
    const text = typeof output?.output === 'string' ? output.output : '';
    forward('PostToolUse', {
      session_id: input?.sessionID || '',
      cwd: directory || '',
      tool_name: typeof input?.tool === 'string' ? input.tool : '',
      tool_input: input?.args ?? null,
      tool_response: text.length > 4000 ? text.slice(-4000) : text,
    });
  },
});
