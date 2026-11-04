import { spawn } from "node:child_process"
import { readFileSync } from "node:fs"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

const NOTIFY_COMMAND: string[] = __NOTIFY_COMMAND__
const AGENT_ID = __AGENT_ID__
const MCP_SERVER = __MCP_SERVER__
const SKILL_IDS: string[] = __SKILL_IDS__
const HOOK_TIMEOUT_MS = 10000
const STDOUT_MAX = 65536
const TEXT_MAX = 300
const PROMPT_MAX = 2000

const here = (): string => (import.meta as any).dir ?? dirname(fileURLToPath(import.meta.url))

/** Active only in a terminal managed by cli-companion, with a private server (--standalone). */
const active = (): boolean => Boolean(process.env.CLI_COMPANION_SESSION) && !process.argv.includes("--service")

const text = (value: unknown, max = TEXT_MAX): string | undefined =>
  typeof value === "string" && value.trim() !== "" ? value.trim().slice(0, max) : undefined

const notifyArgs = (event: string): string[] => [...NOTIFY_COMMAND.slice(1), "--notify", event, AGENT_ID]

function notify(event: string, payload: Record<string, unknown>): Promise<string> {
  return new Promise((resolve) => {
    let out = ""
    let settled = false
    let timer: ReturnType<typeof setTimeout> | undefined
    const done = () => {
      if (settled) return
      settled = true
      if (timer) clearTimeout(timer)
      resolve(out)
    }
    try {
      const child = spawn(NOTIFY_COMMAND[0], notifyArgs(event), { stdio: ["pipe", "pipe", "ignore"], windowsHide: true })
      timer = setTimeout(() => {
        child.kill()
        done()
      }, HOOK_TIMEOUT_MS)
      child.stdout?.on("data", (chunk) => {
        if (out.length < STDOUT_MAX) out += String(chunk)
      })
      child.on("error", done)
      child.on("close", done)
      child.stdin?.on("error", () => {})
      child.stdin?.end(JSON.stringify(payload))
    } catch {
      done()
    }
  })
}

/** Text of a block decision written by --notify on stdout, if any. */
function continuation(stdout: string): string | undefined {
  const line = stdout.trim().split(/\r?\n/).pop() ?? ""
  try {
    return text(JSON.parse(line)?.continue, PROMPT_MAX)
  } catch {
    return undefined
  }
}

function frontmatterDescription(content: string): string | undefined {
  const head = /^---\r?\n([\s\S]*?)\r?\n---/.exec(content)?.[1] ?? ""
  return text(/^description:\s*(.*)$/m.exec(head)?.[1]?.replace(/^["']|["']$/g, ""), 1024)
}

export default {
  id: __PLUGIN__,
  async setup(ctx: any) {
    if (!active()) return
    await ctx.mcp.transform((servers: any) => {
      servers.set(MCP_SERVER, { type: "local", command: [...NOTIFY_COMMAND, "--mcp"] })
    })
    await ctx.skill.transform((skills: any) => {
      for (const id of SKILL_IDS) {
        const path = join(here(), "skills", id, "SKILL.md")
        try {
          const content = readFileSync(path, "utf8")
          skills.add({ id, name: id, description: frontmatterDescription(content), path, content })
        } catch {}
      }
    })

    const sessions = new Set<string>()
    const subagents = new Set<string>()
    const continued = new Set<string>()
    let created = 0

    const start = (sessionID: string, source: string) => {
      if (sessions.has(sessionID) || subagents.has(sessionID)) return Promise.resolve()
      sessions.add(sessionID)
      return notify("session-start", { session_id: sessionID, source })
    }

    async function handle(event: any): Promise<void> {
      const data = event?.data ?? {}
      const sessionID: string | undefined = text(data.sessionID ?? data.form?.sessionID, 200)
      if (!sessionID) return
      if (event.type === "session.created") {
        if (data.parentID ?? data.parentId) {
          subagents.add(sessionID)
          return
        }
        created += 1
        await start(sessionID, created === 1 && sessions.size === 0 ? "startup" : "clear")
        return
      }
      if (subagents.has(sessionID)) return
      switch (event.type) {
        case "session.inbox.enqueued":
          await start(sessionID, "resume")
          return
        case "session.execution.succeeded": {
          await start(sessionID, "resume")
          const continuing = continued.delete(sessionID)
          const prompt = continuation(await notify("finished", { session_id: sessionID, continuing }))
          if (prompt) {
            continued.add(sessionID)
            await ctx.session.prompt({ sessionID, text: prompt })
          }
          return
        }
        case "session.execution.failed":
          await notify("needs-input", { session_id: sessionID, wait: "other", message: text(data.error?.message) })
          return
        case "permission.asked": {
          const tool = text(data.action, 128)
          const summary = text(Array.isArray(data.resources) ? data.resources.join(" ") : undefined)
          await notify("permission-request", { session_id: sessionID, tool, summary })
          await notify("needs-input", { session_id: sessionID, wait: "permission", tool, summary })
          return
        }
        case "form.created": {
          const form = data.form ?? {}
          const wait = form.metadata?.kind === "question" ? "question" : "other"
          await notify("needs-input", { session_id: sessionID, wait, message: text(form.fields?.[0]?.title ?? form.title) })
          return
        }
        case "session.deleted":
          if (sessions.delete(sessionID)) await notify("session-end", { session_id: sessionID, reason: "deleted" })
          return
      }
    }

    const abort = new AbortController()
    let queue = Promise.resolve()
    void (async () => {
      try {
        for await (const event of ctx.event.subscribe({ signal: abort.signal })) {
          queue = queue.then(() => handle(event)).catch(() => {})
        }
      } catch {}
    })()

    // The cleanup also runs when the plugin reloads: it is not the end of a session.
    return () => abort.abort()
  },
}
