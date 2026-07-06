# Pause & resume with work persistence (design)

Status: **design.** This is the design response to issue #4 (*feat: Pause and
resume support with work persistence*). The literal ask — pause the manager and
its agents mid-task, serialize the in-progress work to disk, and on resume have
each agent pick up from its last checkpoint — is **not** fully implementable by
Prompt alone today: the in-flight work lives inside the agent CLI's own context,
which Prompt does not own. This doc records what already survives a restart,
names the one hard blocker, and lays out a scoped build order that delivers the
realistic subsets smallest-first.

## Why

Long agent runs are expensive and interruptible. A user wants to close the app
(or the machine sleeps, or the daemon is killed) and later come back to the same
mesh, with each agent resuming the task it was on rather than starting cold. The
value is concentrated in one place: not losing an agent's accumulated reasoning
when the session is torn down.

## What exists today

Two persistence stories already exist, and they are very different in quality.

**Session restore** (`session-restore`) saves layout only.
`SessionState`/`TabState` (`crates/app/src/sessionstate.rs`) is
`{ layout, cwds, title }` — the split tree, each pane's working directory (in
pre-order leaf order), and the tab title. On restore
(`crates/app/src/root/persist.rs`) it spawns **fresh shells** at those cwds. It
captures no running process and no agent state, so a pane that was running an
agent restores as a bare shell. Saving is skipped entirely for a tab that holds
a webview.

**The relay bus** is the one thing that genuinely survives a kill/restart.
`relay.db` is SQLite in WAL mode (`crates/relay/src/db/mod.rs`) and persists
`agents(name, role, caps, cursor, online, last_seen)`, `subs`, and the last
`MESSAGE_RETENTION` (10,000) `messages`. The daemon is spawned detached
(`setsid`) and outlives an app restart — the app's quit path never stops it
(`crates/app/src/relay/mod.rs`).

**Same-name re-registration resumes a read cursor.** `upsert_agent`
(`crates/relay/src/db/mod.rs`) preserves `cursor` on conflict, so an agent that
re-registers under the same name picks up the messages it missed, bounded by the
10k-message retention window. A brand-new name starts its cursor at the current
message tip (`max_message_id`) and sees no history. This is already noted in
`docs/relay.md` ("re-register — the same name keeps its read cursor").

**The live worker registry is in-memory only.** `App.workers`
(`crates/relay/src/state/mod.rs`) is an `Arc<Mutex<HashMap<String, Worker>>>`,
populated by `spawn::launch` (`crates/relay/src/spawn/mod.rs`) and **not**
rehydrated from the DB on daemon startup. Restarting the daemon therefore loses
every live background worker; only their `agents` rows (name + cursor) remain in
the bus.

## The blocker

An agent's actual in-progress work — its conversation transcript, the tool calls
it has made, its partial plan — lives **inside the Claude Code / Codex CLI
process's own context**. Relay carries the message bus and nothing else; it has
no window into that transcript and no way to serialize it. The launch builders
(`crates/relay/src/cli/agent.rs`) always start a **fresh** agent, with no
`--resume`/`--continue`, so a relaunched agent begins from an empty context even
when its `agents` row and bus cursor survive.

So "pause mid-turn and resume with work intact" cannot be done by Prompt on its
own: that state is opaque and owned by the agent CLI. `docs/relay.md` already
states the shape of this — "a long-running agent holds one growing context for
its whole shift; restart it for a fresh one" — and `docs/parity.md` tracks the
general version as **Persistent, detachable sessions** ("a multi-week
subsystem"). Checkpointing agent in-context work is the agent CLI's job; the
most Prompt can do is ask the CLI to reload its own checkpoint (step 3).

## Scoped plan

Build order, smallest first. Each step stands on its own and is honest about
what it does *not* do.

### 1. Pause / Resume mesh control

A thin surface over what already works. **Pause** = stop the daemon (SIGTERM +
`stop_all` workers) while the WAL bus persists on disk; **resume** = start it
again. This is nearly implemented already: `RelayStart` / `RelayStop` /
`RelayRestart` menu actions exist (`crates/app/src/root/dispatch.rs`, dispatching
to `crates/app/src/relay/mod.rs`). The work is a labelled Pause/Resume affordance
plus copy.

*Effort:* small. *Honest caveat:* this stops the *daemon*, not any agent's
in-flight turn, and any workers that were running come back only if step 2 lands
— otherwise resume brings up the bus with fresh-context agents and no live
workers. Bus messages and read cursors survive; reasoning does not.

### 2. Persist + reload the worker registry

Make "resume the daemon" actually bring the workers back. Write each background
worker's spec to the DB in `spawn::launch` (`crates/relay/src/spawn/mod.rs`), add
a `workers` table alongside `agents`/`subs`/`messages`
(`crates/relay/src/db/mod.rs`), and rehydrate it on daemon startup so the server
respawns them. Self-contained and independently useful — it also fixes the
current crash-recovery gap where a daemon restart silently drops live workers.

*Effort:* medium. *Honest caveat:* respawned workers still start with fresh
context (they re-register under the same name and reclaim their bus cursor, but
not their transcript). This restores *presence*, not *work*.

### 3. Surface `claude --resume <session-id>`

The **only** route to real work-intact resume, and the centerpiece that delivers
the literal "resume from checkpoint" the issue asks for. Capture each agent's
session id from the agent CLI's stream output at launch, store it beside the
saved agent definition, and add a `--resume <id>` path to the launch builder
(`crates/relay/src/cli/agent.rs`). On relaunch the agent then does two things at
once: re-registers under the same name (reclaiming its bus cursor, per step's
prerequisite behaviour in `upsert_agent`) **and** reloads its own transcript via
the CLI.

*Effort:* medium. *Honest caveat:* correctness is bounded entirely by the agent
CLI — Prompt is only threading an id through. It works for providers that expose
a resumable session id (Claude Code's `--resume`, Codex's equivalent) and not
for those that don't; the fidelity of the resumed context is the CLI's to
guarantee, not ours.

### 4. Make session-restore agent-aware

Close the loop on the app side. Remember that a pane was an agent
(provider / name / role) in `TabState` (`crates/app/src/sessionstate.rs`) instead
of dropping it to a plain shell on restore (`crates/app/src/root/persist.rs`),
and on restore offer to relaunch that agent — optionally with `--resume` once
step 3 exists — rather than spawning a bare shell at its cwd.

*Effort:* medium. *Honest caveat:* depends on step 3 for the "with work intact"
half; without it, restore can relaunch the agent fresh but cannot recover its
transcript. Also needs a decision on the current "skip save if a pane holds a
webview" rule, which agent panes may trip.

## Recommendation

Defer the full "pause and resume with work persistence" feature to this design;
do not promise it wholesale against issue #4. The small piece extractable now is
**step 1** — a labelled Pause / Resume mesh control over the existing
Start/Stop/Restart actions, shipped with the honest caveat that it parks the bus,
not agents' in-flight reasoning. **Step 3** (`claude --resume`) is the
highest-value follow-up: it is the single change that turns "resume" from
"relaunch cold" into the checkpoint-resume the issue actually wants, and its cost
and correctness are both bounded by the agent CLI rather than by Prompt. Steps 2
and 4 are the connective tissue that make step 3 feel seamless — worth doing, but
only after 1 and 3 have established the surface and the mechanism.
