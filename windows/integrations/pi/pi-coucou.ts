/**
 * pi-coucou: Extension for Pi Coding Agent to report lifecycle events to Coucou
 * 
 * Interoperates cleanly across Windows, WSL2, and native Linux.
 * Relays session starts, user prompts, tool executions, subagents, and completions
 * to Coucou's top island via coucou-hook / coucou-hook.exe.
 */

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";

function findRelayPath(): string | null {
  // 1. Check native Windows path when Pi runs on Windows (cmd, powershell, Windows Terminal)
  if (process.platform === "win32") {
    const localAppData = process.env.LOCALAPPDATA || join(homedir(), "AppData", "Local");
    const winExe = join(localAppData, "Coucou", "bin", "coucou-hook.exe");
    if (existsSync(winExe)) return winExe;
    const directExe = join(localAppData, "Coucou", "coucou-hook.exe");
    if (existsSync(directExe)) return directExe;
  }

  // 2. Check WSL Windows installation path when Pi runs in WSL
  const winUser = process.env.USERNAME || "willy";
  const wslWinPath = `/mnt/c/Users/${winUser}/AppData/Local/Coucou/bin/coucou-hook.exe`;
  if (existsSync(wslWinPath)) {
    return wslWinPath;
  }

  // 3. Check local coucou repository build
  const localRepoBuild = join(homedir(), "documents", "projects", "coucou", "windows", "target", "release", "coucou-hook");
  if (existsSync(localRepoBuild)) {
    return localRepoBuild;
  }

  // 4. Check Linux standard path
  const linuxPath = join(homedir(), ".local", "share", "coucou", "bin", "coucou-hook");
  if (existsSync(linuxPath)) {
    return linuxPath;
  }

  return null;
}

interface CoucouPayload {
  hook_event_name: string;
  session_id: string;
  coucou_agent?: string;
  prompt?: string;
  tool_name?: string;
  tool_input?: unknown;
  cwd?: string;
}

function sendToCoucou(payload: CoucouPayload): void {
  const relay = findRelayPath();
  if (!relay) {
    return;
  }

  try {
    const isWsl = process.env.WSL_DISTRO_NAME != null;
    const args = ["--agent", payload.coucou_agent || "pi"];
    if (isWsl && process.env.WSL_DISTRO_NAME) {
      args.push("--wsl", process.env.WSL_DISTRO_NAME);
    }
    args.push(payload.hook_event_name);

    const child = spawn(relay, args, {
      stdio: ["pipe", "ignore", "ignore"],
      windowsHide: true,
    });

    child.on("error", () => {
      // Fail silently if Coucou is closed or pipe unreachable
    });

    child.stdin.write(JSON.stringify(payload) + "\n");
    child.stdin.end();
  } catch {
    // Fail silently
  }
}

export default function (pi: ExtensionAPI) {
  let sessionId = `pi-${Date.now().toString(36)}`;

  function getAgentName(ctx?: ExtensionContext): string {
    // Inside ORCA: its terminals set ORCA_PANE_KEY (ORCA's own Pi integration
    // reads the same variable). ORCA_SESSION and ADE are kept for older setups.
    if (process.env.ORCA_PANE_KEY || process.env.ORCA_SESSION || process.env.ADE === "orca") {
      return "orca";
    }
    return "pi";
  }

  pi.on("session_start", async (_event, ctx) => {
    sessionId = ctx?.sessionManager?.getSessionId?.() || `pi-${Date.now().toString(36)}`;
    sendToCoucou({
      hook_event_name: "SessionStart",
      session_id: sessionId,
      coucou_agent: getAgentName(ctx),
      cwd: process.cwd(),
    });
  });

  pi.on("before_agent_start", async (event, ctx) => {
    const prompt = event.messageType === "user" ? event.prompt : "Agent turn";
    sendToCoucou({
      hook_event_name: "UserPromptSubmit",
      session_id: sessionId,
      coucou_agent: getAgentName(ctx),
      prompt,
      cwd: process.cwd(),
    });
  });

  pi.on("tool_call", async (event, ctx) => {
    sendToCoucou({
      hook_event_name: "PreToolUse",
      session_id: sessionId,
      coucou_agent: getAgentName(ctx),
      tool_name: event?.name || "tool",
      tool_input: event?.input,
      cwd: process.cwd(),
    });
  });

  pi.on("tool_result", async (event, ctx) => {
    sendToCoucou({
      hook_event_name: "PostToolUse",
      session_id: sessionId,
      coucou_agent: getAgentName(ctx),
      tool_name: event?.name || "tool",
      tool_input: event?.input,
      cwd: process.cwd(),
    });
  });

  pi.on("agent_settled", async (_event, ctx) => {
    sendToCoucou({
      hook_event_name: "Stop",
      session_id: sessionId,
      coucou_agent: getAgentName(ctx),
      cwd: process.cwd(),
    });
  });

  pi.on("session_shutdown", async (_event, ctx) => {
    sendToCoucou({
      hook_event_name: "SessionEnd",
      session_id: sessionId,
      coucou_agent: getAgentName(ctx),
      cwd: process.cwd(),
    });
  });
}
