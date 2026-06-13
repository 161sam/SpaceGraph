# SpaceGraph — ROADMAP

**Status:** v0.2 (decisions locked), 2026-06-13
**Owner:** 161sam (Sam)
**Verbindlichkeit:** Hoch. Phasenreihenfolge ist begründet (Abhängigkeiten,
Risiko-Reduktion, Reality-Check-Disziplin). Reihenfolge-Änderungen erfordern
dokumentierte Begründung in `docs/adr/`.

**Changelog v0.1 → v0.2:** Operator-Decisions **O-3** (AdminBot = direct IPC
peer), **O-5** (formal ESN admission as 7th Hexa-Repo member, Tier 3), **O-6**
(chronological version ladder, no Kickstarter coupling) locked. §4 sequencing
linearized; v0.6.0 gains the formal-admission deliverable; v0.7.0 architecture
fixes the direct-peer reach. O-1 / O-2 / O-4 remain open (non-blocking).

> This roadmap follows the ESN Way-of-Working established by the ESN-Cockpit
> roadmap: a **consumer hard-pins existing cross-repo contracts and does not
> invent its own**; every phase that touches an external ESN repo opens with a
> **Reality-Check-Gate**; each decision gets an **ADR**, each work package an
> **MP**, each new cross-repo relationship a **`CONSUMERS.md` §3** entry.

---

## 0. Vision & Scope

SpaceGraph is a native Rust/Bevy system-visualization tool that renders a live,
spatial-temporal graph of a host (processes, files, users, sockets, remote
hosts, alerts). It is moving from a **read-only observability tool** toward a
**professional admin + cyber-security workspace** with a distinctive
"Ghost in the Shell" interface.

The strategic thesis of this roadmap: **SpaceGraph does not build a control
plane, a reasoning engine, or a data lake — those already exist as hardened ESN
contracts. SpaceGraph joins the ESN fabric as both a provider and a consumer.**

- As a **provider**, SpaceGraph exposes its graph as an MCP surface
  (`mcp__spacegraph__*`), making live topology, queries and alerts consumable by
  the orchestrator hub and through it by ABrain, Smolit-Assistant and Cockpit.
- As a **consumer**, SpaceGraph drives system actions through **Smolit_AdminBot**
  (`adminbot_action_request`), reasoning through **ABrain** (MCP / HTTP
  `/text/generate`), and history/forensics through **OceanData** — each over its
  documented contract, hard-pinned, never reimplemented.

Everything actionable follows the universal ESN action shape:

```
Decision  →  Review  →  Approval  →  Execution  →  Audit
```

"Approve and execute" is never a single click; the Approval is its own object;
approver ≠ requester; capability-whitelist not generic command runner; audit is
mandatory and `correlation_id`-threaded. Mutating actions are onboarded **one at
a time**, each with its own threat-model and test set (the OceanData PR13.x
discipline).

---

## 1. Where SpaceGraph stands today (grounded audit)

**Viewer (`crates/spacegraph-viewer`, Bevy + egui).** HDR + bloom camera, neon
per-type *colour* (`render/theme.rs`, `GLOW_LEVELS=6`), recency glow, batched
`LineList` edges, billboard labels, node inspector (`I`), legend (`L`), Ctrl+P
search, free-fly (`V`), scan-pulse (`G`), incident-hunt (`M`), fog (`O`),
minimap. Performance baseline solid (index interning, grid repulsion, persistent
node entities — no per-frame churn). Multi-agent endpoint management exists
(`ui/settings_agents.rs`): add/remove UDS endpoints, auto-connect, per-agent
`AgentMode` override, a "Command…" launch-string helper (`util/agent_command.rs`).

**Agent (`crates/spacegraph-agent`).** Strictly **read-only** collectors behind
the `EventSource` trait: `watch_fs`, `watch_proc`, `net` (procfs sockets),
`suricata_eve` (alerts). `AgentMode::{User, Privileged}` governs *which paths are
read*, not write/execute. UDS transport, `PROTOCOL_VERSION = 3`.

**Core (`crates/spacegraph-core`).** `Node::{Process, File, User, Socket,
RemoteHost, Alert}`, `EdgeKind::{Opens, Execs, RunsAs, OwnsSocket, ConnectsTo,
ListensOn, AlertsOn}`, `Msg`/`Delta` wire protocol.

**Gaps that define this roadmap:**
1. Every node is one `Sphere::new(0.28)` mesh — type identity is colour-only; no
   in-world detail or in-world interaction beyond 2D egui panels.
2. The agent only observes; there is no path to *act* on the system.
3. SpaceGraph is **not in the ESN integration fabric** — not among the seven
   Smolit-Stack components (`INTERFACE_INVENTORY.md`), not in the Hexa-Repo map,
   no `CONSUMERS.md`, no MCP surface, no contract with AdminBot / ABrain /
   OceanData.
4. No persistence/history beyond the in-memory timeline ringbuffer.

---

## 2. ESN integration contract map

What SpaceGraph will consume and provide. **All contracts below are existing and
hard-pinned; SpaceGraph adapts to them.** Transport baseline matches the ESN L1
posture: loopback HTTP / UDS, JWT bearer where used, minters omit `aud`/`iss`
(`auth.md`); remote/RS256-JWKS is L3 (MP-AUTH-7/8), out of scope here.

| Peer | Role | SpaceGraph relationship | Surface SpaceGraph uses | Direction |
|---|---|---|---|---|
| **orchestrator** | MCP-Hub (`mcp_proxy/`, `mcp__<server>__<tool>`) | SpaceGraph registers as an MCP server → proxied as `mcp__spacegraph__*` | stdio-MCP server (SpaceGraph-side) | provide |
| **ABrain** | Reasoning brain (provider-only) | consumer | MCP (`abrain.run_plan`, `abrain.explain`, …) or HTTP `POST /text/generate` (127.0.0.1:8788 / UDS) | consume |
| **Smolit_AdminBot** | Privileged host-OS admin daemon (polkit-bridged systemd) | consumer | `adminbot_action_request` axis, AdminBot-wire (UDS u32-prefix + JSON, SO_PEERCRED, 64 KiB cap), capability-whitelist, approval-default | consume |
| **OceanData** | Privacy-first data layer | consumer (history sink + context) | HTTP (`/assets`, `/audit/events`) and/or Context-Provider SPI (`query_context`, `fetch_context_summary`, read-only, UDS/loopback) | consume |
| **Smolit-Assistant** | Desktop assistant (WS IPC 127.0.0.1:8787) | optional bridge | consumes SpaceGraph via the hub; may surface SpaceGraph alerts | provide (indirect) |
| **Cockpit** | Operator workspace (Tauri, action-bus + ConfirmationLayer) | embedded tab | a `tabs/spacegraph/` consuming SpaceGraph's MCP/REST | provide |

**Decision pre-committed by the ESN fabric:** SpaceGraph does **not** add a new
provider/execution axis the way AdminBot/tool-daemon did — it *consumes*
AdminBot. The "external-tool/system-action lives in a daemon peer" decision
(ADR-0005 / ADR-0008) already covers the action universe; SpaceGraph is a client
of that universe, not a new daemon.

---

## 3. Roadmap — tracks & phases

Four tracks. **Track A** (viewer maturity) has no ESN dependency and can run
fully in parallel — including in auto-mode (it is the safe track). **Tracks
B/C/D** touch external repos and carry Reality-Check-Gates; the action parts of
Track C are security-critical and are **not** auto-mode.

Every phase block: **Goal · Reality-Check (if external) · Deliverables · Gate ·
Notes.** Gates are machine-checkable wherever the headless build allows; GPU/FPS
and live cross-repo smoke are documented local-capture steps, never a stop.

### Track A — Viewer maturity (no ESN dependency)

#### v0.3.x — Close-out
**Goal:** stabilize what landed (net layer, alerts, multi-agent) and reconcile
docs before new feature surface.
**Deliverables:** `docs/ACCEPTANCE.md` reconciled to reality; `README` controls
list complete; `docs/perf/RUNLOG.md` final v0.3 section; tag `v0.3.0`.
**Gate:** workspace green (`fmt`/`clippy -D warnings`/`test`); every ACCEPTANCE
gate passes or is amended with a dated note; tag created.

#### v0.4.0 — Node Detail & In-World Interaction  *(MP already written)*
**Goal:** per-type geometry (solid core + wireframe shell), lock-on reticle +
in-world readout, orbital rings, grab-to-pin + edge-picking + radial context
menu, cyberspace post-FX. The GitS thread + half the UX thread.
**Deliverables:** see
`CC_MASTERPROMPT_spacegraph_v0.4.0_node-detail-interaction.md` (6 phases,
auto-mode, `naga`-gated WGSL). Touches `render/node_mesh.rs`, `ui/reticle.rs`,
`render/postfx.rs`, `assets/shaders/cyberspace_post.wgsl`, `graph/` pin state.
**Gate:** per the MP — structural ECS + `naga` + config round-trip gates green;
local visual capture documented; tag `v0.4.0`.
**Notes:** runnable now, independent of everything below.

#### v0.5.0 — UX/UI shell + ESN house-standard alignment
**Goal:** turn floating egui windows into a coherent operator shell and align
with the ESN house UX standard (the Portfolio-MVP / Smolitux-UI tokens).
**Deliverables:**
- Dockable IDE-shell layout (left rail + bottom timeline + right inspector;
  pin/tile) replacing ad-hoc windows.
- True **command palette** (extend Ctrl+P beyond node search → actions,
  navigation, agents, settings; fuzzy) — mirror Cockpit's Cmd+K.
- **Query-DSL** replacing the substring filter (`type:process host:web-01
  sev:high recent:5m`) with chips.
- **Alert inbox** with triage (port the `to_integrate/notification_system`
  idea, native).
- Saved views / bookmarks; first-run tour; colourblind-safe palettes alongside
  Minimal; status/health bar.
- **Design-token alignment:** adopt the house typography (Inter / Space Grotesk
  / JetBrains Mono) and the three-brand token semantics where SpaceGraph's egui
  theme can express them (so SpaceGraph reads as part of the ESN family).
**Gate:** shell layout persists round-trip; palette + DSL covered by unit tests
(parse → action / query → predicate); Minimal-equivalence preserved.
**Notes:** SpaceGraph is Bevy/egui-native, so it **cannot** consume Smolitux-UI
(React) directly — alignment is token/typography/interaction-convention parity,
not component reuse. (Open decision O-1.)

### Track B — SpaceGraph as an ESN *provider*

#### v0.6.0 — SpaceGraph MCP server (read-only)   ← keystone
**Goal:** expose the live graph as an MCP surface so the orchestrator hub
proxies it as `mcp__spacegraph__*`, making SpaceGraph consumable by ABrain,
Smolit-Assistant and Cockpit. This is the single highest-leverage integration
move — it is the "API boundary" done the ESN-native way.
**Reality-Check-Gate:** read orchestrator `mcp_proxy/` contract + `mcp__<server>__<tool>`
naming (Sam-decision MP70.5); read ABrain `MCP_V2_INTERFACE.md` (capability-first
tool shape) as the design template; verify the auth posture (`auth.md`, bearer,
omit `aud`/`iss`).
**Deliverables:**
- New crate `crates/spacegraph-mcp` (stdio MCP server; `rmcp` server-side,
  mirroring the tool-daemon's client choice for ecosystem consistency).
- Capability-first tools (read-only): `spacegraph.query_graph` (typed filter →
  node/edge set), `spacegraph.get_node` (detail by id), `spacegraph.neighbors`,
  `spacegraph.explain_path` (the existing why-connected BFS), `spacegraph.list_alerts`
  (severity-filtered), `spacegraph.topology_summary` (counts / hubs / outer-ring).
- The MCP server reads the **same `GraphState` projection** the viewer renders —
  it is a thin interface over the canonical graph, no parallel logic (the ABrain
  MCP-V2 principle).
- ADR-0001 (SpaceGraph): "MCP surface as the canonical external read API";
  `CONSUMERS.md` created with an orchestrator-hub provider entry.
- **Formal ESN admission (O-5):** a cross-repo MP adds SpaceGraph's row to the
  shared `INTERFACE_INVENTORY.md`, extends the Hexa-Repo map to **Hepta** (7th
  member), and assigns **Tier 3 (Pre-Release)** — promotion to Tier 2 at v1.0,
  mirroring smolit-tool-daemon's posture. This is the point SpaceGraph becomes a
  real fabric member.
**Gate:** `tools/list` + `tools/call` smoke green; each tool has a contract test
(fixture graph → expected typed result); live-smoke documented against the
orchestrator hub (proxied as `mcp__spacegraph__*`); auth token path tested.
**Notes:** read-only by design — no action tools here (those live in Track C
behind AdminBot + the approval layer). Tag `v0.6.0`.

#### v0.6.x — Cockpit tab (embed)
**Goal:** surface SpaceGraph inside the Cockpit operator workspace.
**Reality-Check-Gate:** read Cockpit's tab-manifest + action-bus pattern
(ADR-066 orchestrator-tab as the template); confirm Cockpit consumes over a
process/network boundary (no shared code, no subprocess spawn of a foreign repo).
**Deliverables:** a `tabs/spacegraph/` in Cockpit consuming SpaceGraph's MCP/REST
for a 2D summary (alert feed, topology stats, jump-to-node deep links); the
native 3D viewer launches as its own window from the tab.
**Gate:** tab renders the alert feed + topology summary from the MCP surface;
deep-link opens the viewer focused on a node; Cockpit `CONSUMERS.md §3` gains a
SpaceGraph entry.
**Notes:** SpaceGraph is a **native Bevy window**, not iframe-embeddable like
OceanData's web UI. The embed is "thin React panel over MCP + launch native
window," not an iframe. (Open decision O-2.)

### Track C — SpaceGraph as an ESN *consumer*

#### v0.7.0 — AdminBot integration  ← the admin-tool leap (security-critical)
**Goal:** SpaceGraph becomes a system-*editing* tool by consuming AdminBot's
action surface — **without building its own action channel.** This is the
inflection from observability to administration.
**Reality-Check-Gate:** read AdminBot `adminbot_status` + `adminbot_action_request`
axes, AdminBot-wire (UDS u32-prefix + JSON, SO_PEERCRED, 64 KiB frame cap),
capability-whitelist + approval-default + audit (ADR-0005); read OceanData's
`OPERATOR_APPROVAL_ARCHITECTURE.md` as the binding approval discipline.
**Architecture (pre-committed to the ESN pattern):**
- New crate `crates/spacegraph-adminbot-client` — AdminBot-wire IPC client
  (the AdminBot-wire family verbatim; an `esn-daemon-ipc` shared crate is the
  documented future extraction, not built here).
- SpaceGraph **never** runs a command itself; it emits `adminbot_action_request`
  through the approval layer. The viewer gains an **Approval object** UI
  (`Decision → Review → Approval → Execution → Audit`), with `approver ≠
  requester`, a closed action vocabulary, and the audit trail rendered in-scene
  (reuse the v0.4.0 reticle/readout for the targeted node).
- **Onboard exactly one low-risk action first** (proposed: `process.snapshot` or
  `journal.query` — read-class AdminBot actions), then one mutating action at a
  time (kill/signal a process, restart a unit, drop a socket/conntrack, block an
  IP), each as its own PR with its own threat-model + test set.
- In-world batch action: multi-select N nodes → one approval → AdminBot dispatch
  (this is where the GitS interaction meets the admin spine).
- ADR-0002 (SpaceGraph): "Actions via AdminBot, not a native channel"; ADR per
  onboarded action; `CONSUMERS.md` AdminBot consumer entry.
**Gate:** read-class action round-trips through the full approval object with
audit; "approve and execute is two steps" asserted in tests; per-action threat-
model tests green; no native command execution anywhere in the tree (audited).
**Notes:** AdminBot reach is **resolved (O-3): SpaceGraph is a direct AdminBot
IPC peer.** Its host-local agent speaks AdminBot-wire directly (SO_PEERCRED,
u32-prefix + JSON), no Smolit-Assistant hop — matching AdminBot's own host-local
posture and minimizing hops. The `esn-daemon-ipc` shared crate stays a future
extraction, not built here.
**This phase is NOT auto-mode.** It gets its own master-prompt with hard-stops,
Sam-decisions, and per-action review — like the OceanData PR13.x sequence.

#### v0.8.0 — ABrain reasoning
**Goal:** graph-native reasoning — "explain this attack path", "propose
remediation for this alert cluster", "what is the blast radius of this host".
**Reality-Check-Gate:** read ABrain `MCP_V2_INTERFACE.md` + ADR-0003 (HTTP
`/text/generate` vs MCP `run_plan`); ABrain is **provider-only** and **does not
execute** — its `action_intents` are proposals, gated.
**Deliverables:**
- `crates/spacegraph-abrain-client` (MCP or HTTP, version-tagged adapter).
- SpaceGraph sends a graph slice as context → ABrain returns reasoning /
  `action_intents`; intents are surfaced as **proposed** AdminBot actions and run
  through the v0.7.0 approval layer (never auto-executed).
- New annotation node/edge kinds for ABrain hypotheses (`hypothesis`, `note`),
  rendered distinctly; `correlation_id` threaded into the audit trail.
**Gate:** a fixture alert cluster → ABrain call → rendered reasoning + proposed
(not executed) actions; intents that imply mutation always pause at approval;
adapter version-pin recorded.
**Notes:** keep the ABrain invariant intact — SpaceGraph consumes reasoning,
ABrain governs/proposes, AdminBot + the approval layer execute.

#### v0.9.0 — OceanData history sink + context
**Goal:** time-travel / forensics beyond the in-memory window, and cross-source
context.
**Reality-Check-Gate:** read OceanData HTTP surface (`/assets`, `/audit/events`)
and the Context-Provider SPI (`query_context` / `fetch_context_summary`,
read-only, UDS/loopback, redaction-default `local_only`).
**Deliverables:**
- Event sink: SpaceGraph deltas → OceanData (asset/audit model, or a dedicated
  time-series — Open decision O-4), enabling history scrub past the ringbuffer.
- Read consumption via the Context-Provider SPI for cross-source context
  (default-off, opt-in, `max_items`/`purpose` bounded — no data-lake dump).
- ADR-0003 (SpaceGraph) + `CONSUMERS.md` OceanData entry.
**Gate:** a recorded session round-trips to the sink and back into a scrubbable
timeline; SPI reads honour `local_only` + caps; auth path tested.

### Track D — Security-analytics depth (parallel, viewer-side)

Runs alongside the others; no hard external dependency until it wants to publish
findings (then it uses the Track B MCP surface and Track C approval layer).
- **Graph-native detections:** topology rules ("process spawns shell + new
  outbound socket + alert" = lateral-movement) as a small rule engine in
  `graph/`; detections become first-class alert nodes.
- **More `EventSource`s:** eBPF, auditd, Zeek, Falco — the already-documented
  extension point in the agent. Each is its own MP (eBPF is its own rabbit hole;
  scope deliberately).
- **Attack-surface enrichment:** nodes carry package versions / CVE tags / open-
  port-vs-baseline; nmap/CVE enrichment done natively/safely (not `child_process
  exec`).
- **Fleet:** host grouping, fleet overview, cross-host edge stitching (a
  `RemoteHost` that is another monitored host → join graphs).

---

## 4. Sequencing — chronological version ladder (Operator-Decision O-6)

The roadmap is worked **chronologically as a single version ladder**; no
Kickstarter coupling. Track D runs opportunistically in parallel and publishes
its findings through Track B's MCP surface and Track C's approval layer.

```
v0.3.x  →  v0.4.0  →  v0.5.0  →  v0.6.0   →  v0.6.x  →  v0.7.0   →  v0.8.0  →  v0.9.0
 close-     node      UX-shell   MCP svc     Cockpit    AdminBot    ABrain     OceanData
 out        detail    + house    + ESN       tab        (NOT        reasoning  history
            (MP done) tokens     admission   (embed)    auto)
└────── Track A: no ESN dep, auto-safe ──────┘ └──────────── Tracks B / C: ESN fabric ────────────┘

Track D (analytics): parallel throughout; publishes via v0.6.0 MCP + v0.7.0 approval layer.
```

The chronological order already places the provider surface (v0.6.0) before the
consumer/action work (v0.7.0) — the correct dependency anyway: SpaceGraph becomes
a fabric member (MCP + formal admission) before it drives system actions. Track A
is auto-mode-safe end to end; v0.6.0 is read-only/low-risk and auto-capable;
**v0.7.0 (AdminBot) is the one phase that is not auto-mode** and gets its own
hard-stop master-prompt with per-action review.

---

## 5. Way-of-Working discipline (binding)

- **Reality-Check-Gate** before every phase touching an external ESN repo: read
  that repo's README + relevant ADRs + verify the live API surface; record the
  check in the phase's RUNLOG entry.
- **Hard-pin cross-repo contracts.** SpaceGraph waits on contract updates from
  ABrain / AdminBot / OceanData / orchestrator rather than inventing adapters.
  Adapter-layer per client, version-tagged. Cross-repo issues for SpaceGraph's
  consumer contracts.
- **ADR per decision, MP per work package, `CONSUMERS.md §3` per relationship.**
  ADR numbering SpaceGraph-local (ADR-0001 = MCP surface, ADR-0002 = AdminBot,
  ADR-0003 = OceanData, …). Conventional commits, English, imperative.
- **Naming hygiene / existing-code-first / archive-not-delete / no AI authorship
  markers** (existing AGENTS.md rules carry over).
- **Quality gates** before every commit: `fmt --check`, `clippy --workspace
  --all-targets -D warnings`, `test --workspace`; no `unwrap`/`expect` in
  render/IPC paths.
- **Auth posture L1:** loopback / UDS, JWT bearer where used, minters omit
  `aud`/`iss`, one secret per node; RS256/JWKS (MP-AUTH-7/8 analog) is L3, out of
  scope until ESN-Auth Phase B lands.
- **Action discipline:** the `Decision → Review → Approval → Execution → Audit`
  shape is mandatory for every mutating action; onboard one action per PR with
  its own threat-model + tests; "approve and execute" is never one click.

---

## 6. Decisions

**Resolved — locked 2026-06-13:**

| ID | Decision | Resolution |
|---|---|---|
| **O-3** | AdminBot reach (v0.7.0) | **Direct AdminBot IPC peer** — SpaceGraph's host-local agent speaks AdminBot-wire directly, no Smolit-Assistant hop |
| **O-5** | Formal ESN repo-system admission | **Yes** — SpaceGraph joins as the **7th Hexa-Repo** member at v0.6.0; gets an `INTERFACE_INVENTORY.md` row + **Tier 3 (Pre-Release)**, promotion to Tier 2 at v1.0 |
| **O-6** | Kickstarter coupling | **None** — roadmap worked chronologically as a version ladder (§4) |

**Still open — non-blocking for the near-term phases:**

| ID | Decision | Blocks | Recommendation |
|---|---|---|---|
| **O-1** | UX house-standard: token/typography parity only, or deeper Smolitux-alignment? | v0.5.0 | Parity only (Bevy/egui can't consume React components) |
| **O-2** | Cockpit embed mechanism for a native Bevy window | v0.6.x | Thin React panel over MCP + launch native window (no iframe) |
| **O-4** | OceanData history: asset/audit model, or a dedicated time-series sink? | v0.9.0 | Start with asset/audit; revisit if query shape demands a TS store |

---

## Appendix A — Assumptions to verify

- **A.1** ABrain / OceanData / AdminBot / orchestrator APIs are in active
  development with their own roadmaps. Adapter per client, version-tagged;
  cross-repo issues for SpaceGraph's consumer contracts.
- **A.2** AdminBot's `adminbot_action_request` action vocabulary is the binding
  set; SpaceGraph onboards a subset, one at a time.
- **A.3** The orchestrator hub proxies stdio-MCP servers as `mcp__<server>__*`;
  SpaceGraph's MCP server must be hub-registrable (verify registration shape).
- **A.4** Cockpit consumes peers over a process/network boundary only; SpaceGraph
  exposes nothing that requires shared code or a foreign subprocess spawn.

## Appendix B — Contract sources (read at Reality-Check time)

- ESN component map: `docs/architecture/INTERFACE_INVENTORY.md`
- Action discipline: OceanData `docs/architecture/OPERATOR_APPROVAL_ARCHITECTURE.md`
- AdminBot pattern: Smolit-A `ADR-0005-adminbot-safety-boundary.md`,
  `ADR-0008-outbound-tool-surface.md`, `ADR-0009-tool-daemon-integration.md`
- ABrain surface: `MCP_V2_INTERFACE.md`, ABrain `ADR-0003-native-api-for-text-generation.md`
- OceanData surface: OceanData `INTERFACE_INVENTORY.md §2.5`, `ROADMAP.md` PR13.x,
  `ADR-0006-oceandata-context-provider-spi.md`, `context_query_contract.md`
- Auth posture: `docs/auth.md`, `ADR-068-smolit-stack-auth-posture.md`
- Consumer/embed template: Cockpit `ROADMAP.md`, `ADR-066-orchestrator-hub-integration.md`
- MCP-hub + naming: Cockpit `ROADMAP.md` "Phase-3-Bündelung" (Sam-decision MP70.5,
  `mcp__<server>__<tool>`)