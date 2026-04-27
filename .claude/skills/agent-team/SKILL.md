---
name: agent-team
description: Orchestrate a multi-agent code-review team for the Superpanels project (Implementer + gated Reviewers — Code, Test, Architecture, Security, Performance). Use this whenever the user invokes /agent-team, asks to "run the agent team on…", "review with the team", or describes a Superpanels Phase task they want executed with reviewer gates. Also use proactively when the user asks Claude to start a phase from `docs/plan/` (e.g., "Start phase 1.1", "Implement layout.rs bezel math", "Begin the KDE detection work") — these are exactly the workflows this skill is for. Do NOT use for one-line fixes, doc edits, or pure research questions where the review ceremony adds no value.
---

# agent-team

Orchestrates an Implementer + gated Reviewer team for one Superpanels task. Backed by the `TeamCreate` / `SendMessage` agent-teams API.

## When to use

Invoke for tasks that:
- Involve writing or substantially editing Rust / TypeScript / Svelte code in this project
- Map to one or more tickboxes in a `docs/plan/phase-*.md` file
- Are large enough that reviewer gates earn their keep (rule of thumb: more than ~30 lines of code or touching >1 file)

Skip for:
- Doc-only edits, typo fixes, config tweaks
- Pure research / "explain X to me" questions
- One-line bug fixes you can verify by inspection

## Required reading (before any action)

Always, in order:

1. `/mnt/storage/Projects/superpanels/CLAUDE.md` — project conventions
2. `/mnt/storage/Projects/superpanels/docs/plan/README.md` — index, then ONLY the specific `docs/plan/phase-*.md` whose tickbox the task maps to
3. `/mnt/storage/Projects/superpanels/docs/spec/README.md` — index, then ONLY the specific `docs/spec/NN-*.md` files the task touches (be specific; do not read the whole spec)

The legacy `PLAN.md` and `SPEC.md` at repo root are stub redirects — do not read them.

The reviewer role files in `references/` are read by the orchestrator when spawning each agent — pass the relevant file path in the agent prompt rather than copy-pasting role contents.

## Argument shapes

The skill accepts free-form task descriptions. Common shapes:

| Input | Interpretation |
|---|---|
| `Start phase 1.1` | Look up §1.1 in `docs/plan/phase-1-core-cli.md` and begin its first uncompleted tickbox |
| `Phase 1.2 KDE detection parser` | Begin the KDE detection work in `docs/plan/phase-1-core-cli.md` §1.2 |
| `Implement layout.rs bezel math` | Map to `docs/plan/phase-1-core-cli.md` §1.3 |
| `Wire up the KDE backend trait` | Map to `docs/plan/phase-1-core-cli.md` §1.5 |
| Just `Start phase XX` with no further detail | Begin the first uncompleted tickbox in that phase's file |

If the args don't map cleanly to a plan tickbox, ask the user to clarify before spawning anything. Don't guess.

## Workflow

### Step 1 — Scope and brief

Read CLAUDE.md, the relevant `docs/plan/phase-*.md`, and the relevant `docs/spec/NN-*.md` sections. Produce a one-paragraph scope brief: what task, which tickboxes, which files in scope, which spec sections are load-bearing. Show this brief to the user before spawning the team and wait for a thumbs-up. This is the only synchronous human gate; everything after runs autonomously until completion or a true blocker.

### Step 2 — Decide which reviewers will run

Based on what the Implementer is likely to touch:

| Reviewer | Always or gated? | Trigger rule |
|---|---|---|
| Code Reviewer | Always | (every task) |
| Test Reviewer | Always | (every task) |
| Architecture Reviewer | Gated | Task adds new modules, traits, or workspace dependencies, OR moves files between crates |
| Security Reviewer | Gated | Task touches `crates/superpanels-core/src/backends/`, IPC commands, custom-command handling, FS path crossing trust boundary, Tauri config, or subprocess spawning |
| Performance Reviewer | Gated | Task touches bezel math, image pipeline, library scan, canvas IPC; OR runs at phase exit |

Decide before spawning. If you're unsure for a borderline case, include the gated reviewer — false positives cost a few minutes; false negatives can let regressions land.

### Step 3 — Create the team

```
TeamCreate(team_name = "<phase>-<slug>", description = "Superpanels <task summary>")
```

Slug derives from the task: `phase-1-1-scaffold`, `phase-1-2-kscreen-parser`, `phase-1-3-bezel-math`, etc. Keep the slug short and grep-able.

Create one task per role via `TaskCreate` so progress is visible:
- `Implement: <one-line task>` (owner: implementer)
- `Code review` (owner: code-reviewer; blockedBy: implement task)
- `Test review` (owner: test-reviewer; blockedBy: implement task)
- one task per gated reviewer if applicable, all blockedBy the implement task

### Step 4 — Spawn the Implementer

Use the Agent tool with `subagent_type: "general-purpose"` and the team coordination params. The prompt body must:

1. Open with the contents of `references/implementer.md`
2. Then a `## TASK` section with: the scope brief, the plan tickbox path (e.g. `docs/plan/phase-1-core-cli.md` §1.3), and the file paths in scope
3. Then a `## CONSTRAINTS` section with: pre-commit hooks must pass, conventional commits required, no `--no-verify`

Wait for the Implementer to report back via `SendMessage` (or via task completion). Do not start reviewers before the Implementer reports done.

### Step 5 — Spawn reviewers in parallel

Once the Implementer reports done, spawn every reviewer determined in Step 2 in a single message (parallel). Each reviewer prompt:

1. Opens with the contents of its `references/*.md` file
2. Includes a `## SCOPE` section with: the diff (or commits) to review, the plan tickbox path, the spec section files (`docs/spec/NN-*.md`) that govern it
3. Includes the team_name so they can post their verdict

Reviewers must NOT write code. Their output is a Status (Approve / Request changes) plus a list of file:line citations to the docs they enforced.

### Step 6 — Aggregate verdicts

Wait for all spawned reviewers to report. Then:

- **All Approve →** report success to the user with a short summary (commits, files changed, reviewer scoreboard) and stop.
- **Any Request-changes →** consolidate the change requests into one numbered list, send to the Implementer via `SendMessage`. The Implementer addresses them, reports done. Re-spawn only the reviewers that requested changes (not the Approves). Loop.

Hard limit: 3 review rounds. If round 4 would be needed, stop and escalate to the user with the diff and the unresolved requests — there's a disagreement the docs can't resolve.

### Step 7 — Shut down

When all reviewers Approve:

1. `SendMessage` each teammate with `{type: "shutdown_request"}`
2. Wait for shutdown responses
3. `TeamDelete`

Final report to the user includes:
- Task: what was done
- Commits: SHAs + short messages
- Reviewer scoreboard (rounds, who blocked, who approved)
- Followups: any non-blocking advisory items the reviewers flagged
- Where to look in the relevant `docs/plan/phase-*.md` for the next tickbox

## Escalation rules

Stop and ask the user (do not decide unilaterally) if:

- Reviewers disagree and the docs don't resolve it
- Implementer would need to add a non-trivial dependency
- A `docs/spec/` or `docs/plan/` file looks ambiguous, contradictory, or wrong (note it; never edit spec/plan files as part of the implementation task — that's a separate PR)
- Round 3 review still has blockers
- The task as-described would touch files outside its declared scope

## Output format to the user (final)

```
## ✓ <task title>

**Plan tickbox(es):** `docs/plan/phase-X-*.md` §X.Y "<title>"
**Commits:** <sha>... <sha>... (<n> total)
**Files:** <count> changed (+<lines> -<lines>)

**Reviewer scoreboard**
| Reviewer | Rounds | Final |
|---|---|---|
| Code | <n> | ✓ Approved |
| Test | <n> | ✓ Approved |
| <gated>... |

**Advisory followups** (not blocking)
- ...

**Next**: `docs/plan/phase-X-*.md` §X.Y "<next tickbox>"
```

## Reference files

Each role's prompt lives under `references/`. Reviewers and the Implementer should read their *own* file and the relevant doc(s) it points at — they should NOT read the others' files (keeps context lean and roles bounded).

- `references/implementer.md`
- `references/code-reviewer.md`
- `references/test-reviewer.md`
- `references/architecture-reviewer.md`
- `references/security-reviewer.md`
- `references/performance-reviewer.md`
