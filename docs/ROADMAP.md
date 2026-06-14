# SpaceGraph — ROADMAP

**Status:** v0.5 (active-recon direction locked), 2026-06-14
**Owner:** 161sam (Sam)
**Verbindlichkeit:** Hoch. Phasenreihenfolge ist begründet (Abhängigkeiten,
Risiko-Reduktion, Reality-Check-Disziplin). Reihenfolge-Änderungen erfordern
dokumentierte Begründung in `docs/adr/`.

**Changelog v0.4 → v0.5 — direction change:**
- SpaceGraph **extends from a Monitor into a Monitor + Recon + Red-Team
  workspace** — a professional pentest tool intended for **commercial
  monetization as an ESN product**. It gains its own **active, aggressive scanner**
  (Shodan-class: discovery, port scan, fingerprint, TLS/cert, OS detect, CVE
  correlation, searchable index).
- **ADR-0013 supersedes ADR-0004 §O-7.** The dividing line moves from *passive vs
  active* to **intelligence/recon (SpaceGraph) vs system-action (Smolit)**. The
  two-plane discipline **moves inside SpaceGraph**: a passive observation plane
  (`spacegraph-agent`, read-only — **guarantee preserved**) + an active
  reconnaissance plane (new **`spacegraph-scanner`** crate, egress, scope-gated).
- New **Track E — Active Reconnaissance** (multi-phase; NOT auto-mode).
- Scope/authorization model locked: **both** own/authorized (default) and
  internet-wide (explicit, operator-owned, audited) modes; the scanner refuses to
  run without an explicit scope (O-11). The scanner **gathers** — it does **not**
  exploit (exploitation deferred to a separate later decision).
- Licensing (dual-license, Cockpit `ADR-053` precedent) + an authorized-use EULA
  become **release-blocking** for the commercial product (reserved ADR).

**Changelog v0.3 → v0.4:**
- A **visualization catalog** (§0.3) is adopted: the full set of what SpaceGraph
  renders and how, each item tagged with its data source and roadmap home.
- **D4 expanded** from "AI-fabric viz" into the **node-model extension + boundary
  phase** — the node-model extension (over `PROTOCOL_VERSION` 4) opens: the
  closed-core + open-extension node model, the **boundary/containment render
  primitive** (which serves the internet membrane *and* VM/Container *and* trust
  zones — one primitive), AI-fabric (Agent/Model), virtualization (VM/Container),
  and **telemetric state & vitals** (per-process CPU/RSS + system load/mem/disk).
  The 3→4 bump was spent by v0.5.2 FS-search (ratified ADR-0016); after that,
  new node classes are *data* and further wire changes are governed (O-8).
- New near-term phase **D0 — Perimeter & exposure visual pass** (AUTO, no wire):
  port-state-as-aperture, exposure-as-depth, anomaly-as-scene-distortion — all
  derived from data already collected. Does **not** wait behind v0.6.0.
- New passive `EventSource`s scoped under Track D2: **firewall** (nftables via
  read-only netlink — *not* an `nft`/`iptables` exec) and **traffic flow**
  (conntrack/nftables byte counters; eBPF stays deferred).
- **Telemetric state** answered: the live numbers top/htop/ss/df show come from
  procfs/sysfs the agent already reads — collected as node/host *attributes* (not
  via subprocess spawn, which the no-exec rule forbids), encoded as node vitality
  *and* read as numbers in the inspector + a system-vitals HUD.
- Operator-Decision **O-10** (node-model extensibility: closed-core + open-
  extension, one bump behind v0.6.0, derived-visual within design envelopes)
  locked.

**Changelog v0.2 → v0.3:**
- The **two-plane split** is made the organizing principle (§0.1): SpaceGraph is
  the read-only observability + detection *cortex*; the Smolit-Stack is the
  action / reasoning / egress / retention *muscle*. The professional admin +
  pentest tool is the **fabric**; SpaceGraph is its safe plane within it.
- A **SOC capability-layer map** (§0.2) grounds the "full professional tool"
  scope and assigns every layer to a plane.
- **Track D** is promoted from a parallel feature-bag into explicit phases
  **D1–D6** — the security-platform core.
- **MITRE ATT&CK** is added as a first-class, cross-cutting dimension
  (detection tags · tactic-phased viz · coverage/posture).
- **SOAR playbooks** added to Track C as `v0.7.x` (gated, post single-action).
- Operator-Decisions **O-7** (egress stays Smolit-side; SpaceGraph never gains an
  egress path), **O-8** (no `spacegraph-core` wire bump until the v0.6.0 MCP
  surface stands), **O-9** (scan/probe triggering stays passive until the v0.7.0
  AdminBot approval layer) locked. O-1 / O-2 / O-4 remain open (non-blocking).
- §1 "stands today" refreshed to post-`v0.5.0` reality (recon F2).

> This roadmap follows the ESN Way-of-Working established by the ESN-Cockpit
> roadmap: a **consumer hard-pins existing cross-repo contracts and does not
> invent its own**; every phase that touches an external ESN repo opens with a
> **Reality-Check-Gate**; each decision gets an **ADR**, each work package an
> **MP**, each new cross-repo relationship a **`CONSUMERS.md` §3** entry.

---

## 0. Vision & Scope

SpaceGraph is a native Rust/Bevy system-visualization tool that renders a live,
spatial-temporal graph of a host (processes, files, users, sockets, remote
hosts, alerts). It is growing into a **Monitor + Recon + Red-Team workspace** with
a distinctive "Ghost in the Shell" interface — a **professional pentest tool**
intended for **commercial monetization as an ESN product**. It visualizes *both*
the defended estate (inside-out: hosts, processes, perimeter, incoming threats)
*and* the offensive surface (outside-in: the scanned internet, target services lit
by their open apertures). Blue team sees its own exposure; red team sees the
target surface; purple team sees both, in one scene.

### 0.1 Two planes — now within SpaceGraph (organizing principle — binding)

The dividing line is **intelligence/recon (SpaceGraph) vs system-action (Smolit)**
— see **ADR-0013** (supersedes ADR-0004 §O-7). SpaceGraph now spans two planes
*internally*:

- **Passive observation plane** — `spacegraph-agent`, read-only host collection.
  **Its read-only / no-egress / no-exec guarantee is preserved** — SpaceGraph can
  still run as a safe monitor on a production or client host.
- **Active reconnaissance plane** — `spacegraph-scanner` (new crate): discovery +
  scanning + fingerprinting, with egress, **scope-gated** (Track E).

Both gather *intelligence*. The scanner **does not exploit or modify** targets —
exploitation is deferred to a separate later decision; system-action (remediation,
AdminBot) stays the Smolit plane. The scanner is a **separate crate** for
deployment, blast-radius, and dependency reasons (ADR-0013 §2). The original
two-plane discipline does not die — it **moves inside SpaceGraph**.

The legacy admin-platform layering below (§0.2) still holds for the *defensive*
side; the offensive/recon side is Track E.

### 0.1.1 The old split (defensive layers — still valid)

A modern admin/pentest platform is collection → normalization → detection →
enrichment → triage → response → forensics → reporting → offensive. For the
*defensive* layers, the capability is split across the SpaceGraph cortex and the
Smolit muscle, connected by the MCP fabric and the AdminBot approval spine:

| Plane | Role | Posture | Owns |
|---|---|---|---|
| **SpaceGraph** | Observability & detection **cortex** — the "eyes" | **read-only, no egress, no execution** | passive collection, graph-native normalization, detection/correlation, ATT&CK mapping, the spatial/temporal live workspace, coverage/posture, read-only MCP provider surface |
| **Smolit-Stack** | Action, reasoning, egress, retention **muscle** — the "hands & brain" | gated / privileged | AdminBot (system actions + SOAR), ABrain (reasoning), Smolit-Assistant (operator + pentest driver + egress), OceanData (forensic store + audit), orchestrator (MCP hub + internet-facing execution) |

**Invariant (binding, see §5).** SpaceGraph never acquires an outbound-network
egress path and never executes a system action itself. Everything active —
threat-intel/CVE/reputation enrichment, scans, exploitation, internet research,
remediation — lives Smolit-side behind the `Decision → Review → Approval →
Execution → Audit` spine. This is not a limitation; it is the property that makes
SpaceGraph a *trustworthy* admin tool rather than a generic script runner.
(Locked by O-7.)

### 0.2 SOC capability-layer map

The full professional admin/pentest tool is the fabric below. Each layer is owned
by exactly one plane; the "Status" column tracks SpaceGraph-relevant work.

| # | Layer | Plane | Concretely | Status |
|---|---|---|---|---|
| 1 | **Collection** | SpaceGraph | `EventSource` families: `proc`/`fs`/`net` (live) + `suricata`/`clamav-log`/`nebula-log`/`auditd`/`eBPF`/`Zeek`/`Falco`/`journald`/`auth.log`/web-logs | live core; rest = Track D2+ (each its own MP) |
| 2 | **Normalization** | SpaceGraph | **graph-native model** instead of tables — *the differentiator* | done |
| 3 | **Detection & Correlation** | SpaceGraph + ABrain | `graph/rules.rs` compiled matchers (D1) → multi-stage campaign correlation (D3); deep reasoning via ABrain (`v0.8.0`) | D1 = next MP |
| 4 | **ATT&CK mapping** | SpaceGraph | technique/tactic tags on detections; tactic-phased viz; coverage view | D1 (tags) + D5 (coverage) |
| 5 | **Enrichment** (threat-intel / CVE / reputation / DAP) | **Smolit** (egress) → fed back via MCP | reputation/CVE/hash verdicts computed Smolit-side, attached to SpaceGraph nodes as read-only annotations over the fabric | O-7; not a SpaceGraph build item |
| 6 | **Triage & Investigation** | SpaceGraph + ABrain | alert inbox (live) → incident/case object (D6); ABrain-assisted triage | inbox done; case = D6 |
| 7 | **Response & SOAR** | **AdminBot** (gated) | single onboarded action (`v0.7.0`) → playbooks (`v0.7.x`) | NOT-AUTO |
| 8 | **Forensics & Retention** | **OceanData** | delta sink + timeline scrub past the ringbuffer | `v0.9.0` |
| 9 | **Reporting & Posture** | SpaceGraph + OceanData | ATT&CK coverage heatmap + posture/exposure score + pentest-engagement report view | D5 |
| 10 | **Offensive / Pentest** | **Smolit / Nebula** (active + egress) | recon/scan/exploit/internet-research run Smolit-side; SpaceGraph **observes & visualizes** the engagement, never drives it | O-7 + O-9 |

The strategic thesis is unchanged: **SpaceGraph joins the ESN fabric as both a
provider and a consumer; it does not build a control plane, a reasoning engine, a
data lake, or an egress channel — those already exist as hardened ESN
contracts.** It adds the one thing the fabric lacks: a graph-native, spatial-
temporal detection + visualization cortex.

### 0.3 Visualization catalog

What SpaceGraph renders and how, vs. classical tools (which show flat, tabular,
siloed, *static* state — you read logs of change). SpaceGraph's claim is **one
living, spatial-temporal, semantically-rich scene** you navigate like a place,
where topology + behaviour + threat + AI + boundaries + state + time coexist.
Each item below carries its data source and roadmap home; "AUTO/no-wire" items
are derivable over already-collected data and do **not** wait behind v0.6.0.

| Visualization | Effect | Data | Home |
|---|---|---|---|
| **Type-from-shape** | per-kind silhouette (octahedron/prism/cone/torus/…) | core kinds (live) | done |
| **Recency glow + edge pulse** | the graph *breathes*; activity flashes, decays | live | done |
| **Threat-motion vocabulary** | per-ATT&CK-tactic motion (worm-spread, C2 beacon, lateral sweep, exfil flow, brute-force flash) | tactic tag (D1) | D2 |
| **Port-state-as-aperture** | LISTEN = open aperture · ESTABLISHED = flow beam · gated = shuttered + barrier · closing = dimming | `Socket.state` (present) | **D0 — AUTO/no-wire** |
| **Exposure-as-depth** | internet-facing on outer shell · loopback buried in core → attack surface as silhouette | `Socket.local_addr` (present) | **D0 — AUTO/no-wire** |
| **Anomaly-as-scene-distortion** | a detection *distorts* locally (ripple/desaturate/focus-pull) → eye drawn to *where* | post-fx (present) | **D0 — AUTO/no-wire** |
| **Internet membrane + gateway portal** | local space enclosed; "the net" beyond a perimeter; egress through a visible chokepoint | derived + `/proc/net/route` | D4 boundary primitive |
| **VM / Container / trust-zone regions** | nested boundaries; privilege escalation = boundary crossing; segmentation legible | `virt` source + uid | D4 boundary primitive |
| **Telemetric vitality** | CPU/MEM/IO → pulse-rate/size/instability (thrashing vibrates, leak swells, flap flickers) | procfs/sysfs vitals | D4 (vitals) |
| **State readout + system-vitals HUD** | the htop numbers, *in* the scene (inspector + header) → drill without leaving | procfs/sysfs vitals | D4 (vitals) |
| **Traffic-as-flow** | beam thickness/rate ∝ bytes/s; exfil = fat outbound beam, beacon = thin rhythm | flow counters | D2 (flow source) |
| **Firewall-as-gate** | allowed paths glow through; blocked attempts flare red on the barrier | nftables (netlink, read-only) | D2 (firewall source) |
| **AI-fabric** | watch agents/LLMs/ML reason + act (agent→model→target) | orchestrator MCP tap | D4 (Agent/Model) |
| **Provenance trails** | "how did this get here" — ancestry / writer / initiator, pulled on | ppid + deeper | explain.rs depth |
| **Semantic zoom** | zoom out → containers collapse into VMs into hosts into zones | D4 containment | D4-enabled |
| **In-world time-scrubbing** | scrub time → scene *replays spatially*; attack propagates | timeline → forensic | `v0.9.0`/OceanData |
| **In-world approval/audit** | the gated-action spine + audit trail rendered in-scene | AdminBot | `v0.7.0` |

**Binding principle (flexible *and* unique).** Every visualization encodes
*real* semantics (deterministic, within a design envelope), degrades to Minimal
cleanly, and lives in the *one* scene. Flexibility without semantic discipline →
gray-sphere soup; uniqueness without flexibility → a pretty demo that can't model
the real world. Both together are the moat. (See O-10.)

---

## 1. Where SpaceGraph stands today (grounded audit, post-`v0.5.0`)

**Viewer (`crates/spacegraph-viewer`, Bevy + egui).** HDR + bloom camera; per-type
emissive **geometry** (octahedron/prism/cone/torus/sphere cores + holographic
wireframe shells, `render/node_mesh`); orbital rings on hubs/threats; recency
glow ramp (`GLOW_LEVELS`); class-coloured edges with activity pulse; cyberspace
post-fx (scanlines/vignette/CA/grain, `render/postfx`); in-world lock-on reticle +
readout; grab-to-pin, edge-picking, radial context menu, marks; node inspector,
legend, search, free-fly, scan-pulse, incident-hunt, fog, minimap; micro-tags.
UX-shell + ESN house-standard tokens land with `v0.5.0` (in flight). Performance
baseline solid (index interning, grid repulsion, persistent node entities — no
per-frame churn). Multi-agent endpoint management (`ui/settings_agents.rs`).

**Agent (`crates/spacegraph-agent`).** Strictly **read-only** collectors behind
the `EventSource` trait (`sources/mod.rs`): `fs`, `proc`, `net` (procfs sockets),
`suricata_eve` (alerts). `AgentMode::{User, Privileged}` governs *which paths are
read*, never write/execute. UDS transport, `PROTOCOL_VERSION = 4`
(`MIN_COMPATIBLE_PROTOCOL = 3`; the 3→4 bump was spent by v0.5.2 FS-search). Hard
rule: no `child_process`/exec anywhere in the tree (audited).

**Core (`crates/spacegraph-core`).** `Node::{Process, File, User, Socket,
RemoteHost, Alert}`; `EdgeKind::{Opens, Execs, RunsAs, OwnsSocket, ConnectsTo,
ListensOn, AlertsOn}`; `Msg`/`Delta` wire protocol, `PROTOCOL_VERSION = 4`
(ratified, ADR-0016).

**Gaps that define this roadmap:**
1. The viewer has **no detection of its own** — alerts arrive only from external
   sources (Suricata). There is no graph-native rule/correlation engine
   (`graph/rules.rs` does not exist; `explain.rs` does single-pair BFS only).
2. **No ATT&CK dimension** — detections carry no technique/tactic, no coverage
   view, no tactic-phased semantics.
3. **AI activity is invisible** — there is no `Agent`/`Model` node concept and no
   tap on the orchestrator's MCP tool-call traffic.
4. The agent only observes; **no path to act** (by design — actions are Smolit-
   side, gated, `v0.7.0`).
5. SpaceGraph is **not yet in the ESN integration fabric** — no `CONSUMERS.md`,
   no MCP surface, no contract with AdminBot/ABrain/OceanData (Tracks B/C).
6. **No persistence/history** beyond the in-memory timeline ringbuffer (`v0.9.0`).

---

## 2. ESN integration contract map

What SpaceGraph will consume and provide. **All contracts below are existing and
hard-pinned; SpaceGraph adapts to them.** Transport baseline matches the ESN L1
posture: loopback HTTP / UDS, JWT bearer where used, minters omit `aud`/`iss`
(`auth.md`); remote/RS256-JWKS is L3 (MP-AUTH-7/8), out of scope here.

| Peer | Role | SpaceGraph relationship | Surface SpaceGraph uses | Direction |
|---|---|---|---|---|
| **orchestrator** | MCP-Hub (`mcp_proxy/`, `mcp__<server>__<tool>`) | registers as an MCP server → proxied as `mcp__spacegraph__*`; **taps tool-call traffic for AI-fabric viz (D4)** | stdio-MCP server (SpaceGraph-side) | provide + observe |
| **ABrain** | Reasoning brain (provider-only) | consumer | MCP (`abrain.run_plan`, `abrain.explain`, …) or HTTP `POST /text/generate` | consume |
| **Smolit_AdminBot** | Privileged host-OS admin daemon | consumer | `adminbot_action_request`, AdminBot-wire (UDS u32-prefix + JSON, SO_PEERCRED, 64 KiB cap), capability-whitelist, approval-default | consume |
| **OceanData** | Privacy-first data layer | consumer (history sink + context) | HTTP (`/assets`, `/audit/events`) and/or Context-Provider SPI | consume |
| **Smolit-Assistant** | Desktop assistant + **pentest/egress driver** | optional bridge; **emits the engagement activity SpaceGraph observes** | via the hub; surfaces SpaceGraph alerts | provide (indirect) + observe |
| **Cockpit** | Operator workspace | embedded tab | a `tabs/spacegraph/` consuming SpaceGraph's MCP/REST | provide |

**Decision pre-committed by the ESN fabric:** SpaceGraph does **not** add a new
provider/execution axis the way AdminBot/tool-daemon did — it *consumes* AdminBot
and *observes* the orchestrator. Enrichment that needs egress (threat-intel, CVE,
reputation) is produced Smolit-side and flows back as read-only node annotations
(O-7); SpaceGraph never makes the outbound call itself.

---

## 3. Roadmap — tracks & phases

Four tracks. **Track A** (viewer maturity) and **Track D** (security analytics)
have no hard ESN dependency and are auto-mode-safe — until Track D wants to
*publish* findings (then it rides Track B's MCP surface and Track C's approval
layer). **Tracks B/C** touch external repos and carry Reality-Check-Gates; the
action parts of Track C are security-critical and are **not** auto-mode.

Every phase block: **Goal · Reality-Check (if external) · Deliverables · Gate ·
Notes.** Gates are machine-checkable wherever the headless build allows; GPU/FPS
and live cross-repo smoke are documented local-capture steps, never a stop.

### Track A — Viewer maturity (no ESN dependency)

#### v0.3.x — Close-out *(done)*
Stabilized net layer + alerts + multi-agent; `ACCEPTANCE.md` reconciled; tag
`v0.3.0`.

#### v0.4.0 — Node detail + in-world interaction *(done)*
Per-type geometry, orbital rings, reticle/readout, grab-to-pin, edge-picking,
radial menu, post-fx. Tag `v0.4.0`.

#### v0.5.0 — UX/UI shell + ESN house-standard alignment *(in flight — current MP)*
**Goal:** dockable operator shell, command palette (navigation/view/settings
only — **no host actions**), query-DSL over the in-memory graph, native alert
inbox/triage, saved views, design tokens/typography; in-app theme selector
(recon F1).
**Gate:** round-trip/persist tests for shell+views+triage; query-DSL parse→apply
tests; Minimal-equivalence preserved; headless gates green; GPU capture
documented. No command-execution surface lands here. Tag `v0.5.0`.
**Notes:** O-1 (token parity vs deeper Smolitux alignment) — recommend parity.
AUTO.

### Track B — SpaceGraph as an ESN *provider*

#### v0.6.0 — SpaceGraph MCP server (read-only) + formal ESN admission  ← keystone
**Goal:** expose the graph as `mcp__spacegraph__*` and join the fabric as the 7th
Hexa-Repo member (Tier 3). Every consumer phase (v0.6.x/v0.7.0/v0.8.0) and the
AI-fabric viz (D4) depend on this.
**Reality-Check-Gate:** verify the orchestrator hub registration shape
(`mcp_proxy/`); read `INTERFACE_INVENTORY.md` admission row format.
**Crux:** canonical-state access — how an out-of-process stdio MCP server reads
the in-process `GraphState`. Resolve before tool code.
**Deliverables:** `crates/spacegraph-mcp` (read-only tools: topology stats, node
query, alert feed, explain-path); contract test per tool (fixture graph → typed
result); `INTERFACE_INVENTORY.md` row + Tier 3; `CONSUMERS.md` provider entry;
**ADR-0001**.
**Gate:** `tools/list`+`tools/call` smoke green; per-tool contract test; live-
smoke documented against the hub; auth token path tested. Tag `v0.6.0`.
**Notes:** read-only by design — no action tools (those are Track C). AUTO.

#### v0.6.x — Cockpit tab (embed)
*(unchanged from v0.2; thin React panel over MCP + launch native window, O-2 open)*

### Track C — SpaceGraph as an ESN *consumer* (action + reasoning + retention)

#### v0.7.0 — AdminBot integration  ← the admin-tool leap (security-critical)
**Goal:** SpaceGraph becomes a system-*editing* tool by consuming AdminBot's
action surface — **without building its own action channel.**
**Reality-Check-Gate:** AdminBot `adminbot_status`+`adminbot_action_request`,
AdminBot-wire (SO_PEERCRED, u32-prefix+JSON, 64 KiB cap), capability-whitelist +
approval-default + audit (`Smolit-Assistant ADR-0005`); OceanData
`OPERATOR_APPROVAL_ARCHITECTURE.md` as the binding approval discipline.
**Architecture (pre-committed):** `crates/spacegraph-adminbot-client`
(AdminBot-wire verbatim); SpaceGraph **never** runs a command — it emits
`adminbot_action_request` through the approval layer; viewer gains an **Approval
object** UI (`approver ≠ requester`, closed vocabulary, in-scene audit trail);
in-world batch action (multi-select N → one approval → dispatch). Onboard exactly
one low-risk action first (`process.snapshot` or `journal.query`), then one
mutating action at a time, each its own PR + threat-model + tests. **ADR-0002** +
ADR per onboarded action; `CONSUMERS.md` AdminBot entry. **O-9: scan/probe
triggering arrives here, not before — it is a mutating action under the spine.**
**Gate:** read-class action round-trips the full approval object with audit;
"approve and execute is two steps" asserted; per-action threat-model tests green;
no native execution anywhere (audited). Tag `v0.7.0`.
**Notes:** AdminBot reach **resolved (O-3): direct AdminBot IPC peer.** **NOT
auto-mode** — own master-prompt with hard-stops, Sam-decisions, per-action review.

#### v0.7.x — SOAR playbooks (gated)  ← the response-orchestration layer
**Goal:** named, multi-action **response playbooks** for a detection class —
the SOAR layer above single-action `v0.7.0`.
**Reality-Check-Gate:** re-confirm the AdminBot capability-whitelist and the
OceanData approval discipline still bound a *sequence* of actions (one approval
object may now gate N ordered actions; `approver ≠ requester` still holds per
playbook run).
**Architecture:** a playbook is a *closed, declarative* sequence of already-
onboarded AdminBot actions (e.g. *contain-host* = drop conntrack → block IP →
snapshot process), triggered by a detection (D1/D3) or ABrain proposal (`v0.8.0`),
surfaced as a **single Approval object** with the full ordered plan visible
before approval; each step still audited with the shared `correlation_id`. No new
action primitives — playbooks compose only vocabulary already onboarded one-at-a-
time in `v0.7.0`. **ADR-0011** + ADR per playbook.
**Gate:** a fixture detection → proposed playbook → single approval gates the
whole ordered sequence; rejecting any step halts the run; partial-failure leaves
an audited, resumable state; "no un-onboarded action appears in any playbook"
asserted. Tag within `v0.7.x`.
**Notes:** **NOT auto-mode.** Each playbook is its own PR + threat-model. Playbooks
never auto-trigger from a detection — a detection *proposes*; an operator
*approves*.

#### v0.8.0 — ABrain reasoning
*(unchanged from v0.2; graph-slice → reasoning/`action_intents` → proposed, gated
through the v0.7.0/v0.7.x layer, never auto-executed; `hypothesis`/`note`
annotation kinds; **ADR-0003-abrain**)*

#### v0.9.0 — OceanData history sink + context
*(unchanged from v0.2; delta sink for timeline scrub past the ringbuffer +
read-only Context-Provider SPI, default-off/opt-in/capped; **ADR-0003-oceandata**.
This is also the persistence backend for the D6 incident/case object.)*

### Track D — Security-analytics & visualization depth (phased; viewer-side, read-only)

Promoted from a parallel feature-bag into explicit phases. **D0, D1, D2, D3, D5
are AUTO and need no `spacegraph-core` wire change** (detections reuse
`Node::Alert`; perimeter/exposure, purple-team origin and ATT&CK tags ride as
viewer-side fields / source strings; new collectors emit existing kinds). The
3→4 wire bump is already spent (v0.5.2 FS-search, ratified ADR-0016), so
`PROTOCOL_VERSION = 4` is the baseline. **D4 — the node-model extension phase** —
opens the closed-core + open-extension node model, the boundary primitive,
AI-fabric (Agent/Model), virtualization (VM/Container), and telemetric vitals; it
stays **deferred behind v0.6.0** because the AI-fabric tap needs the MCP surface,
and any further wire change its schema needs is **governed** (O-8). **D6 depends
on v0.9.0** for persistence.
Detections are advisory until published; any remediation is deferred to Track C
(`v0.7.x`) under the approval spine.

#### D0 — Perimeter & exposure visual pass (AUTO, no wire)  ← near-term, parallel to v0.5.0
**Goal:** make the most security-relevant invisible properties — port state,
exposure, anomaly locality — legible, with no new data and no wire change.
**Deliverables:** **port-state-as-aperture** (the Socket torus renders per
`state`: LISTEN open · ESTABLISHED flow beam · gated/filtered shuttered + barrier
ring · closing dimming); **exposure-as-depth** (radial position derived from
`local_addr`: internet-reachable on the outer shell, RFC1918 mid, loopback at the
core — attack surface readable as silhouette); **anomaly-as-scene-distortion**
(an alert/detection localizes the post-fx — ripple/desaturate/focus-pull — so the
eye is drawn to *where*, not just *that*); the gateway as a derived `RemoteHost`
from `/proc/net/route` (small `net`-source read, reuses an existing kind). All
keyed off `theme.rs` constants, all degrade to Minimal. **ADR-0012** (perimeter &
exposure visual model).
**Gate:** exposure-bucket derivation unit-tested from `local_addr` fixtures;
aperture/barrier render selected by `state` (pure-fn test on the style picker);
anomaly distortion bounded + Minimal-off; no wire change, no new data field;
headless gates green.
**Notes:** AUTO; high visual + security value at low risk. Does **not** wait for
v0.6.0.

#### D1 — Graph-native detection rule engine + ATT&CK tagging  ← next MP (AUTO)
**Goal:** the viewer synthesizes its own detections from graph topology, each
tagged with a MITRE ATT&CK technique/tactic.
**Architecture:** `graph/rules.rs` — **compiled matchers** (not a DSL; DSL
deferred, no speculative generality), run in a **budgeted Update system after
layout** over the canonical `GraphModel` (reusing its prebuilt adjacency +
`AggEdge`/`EdgeStats` indices + `EdgeKindClass`); detections emit as first-class
`Node::Alert` with `source = "spacegraph-rule"` and a stable de-dup id (re-arm,
no per-frame full-graph rescan). First rules match on **existing** graph data
(no new collector): lateral-movement, suspicious new listener, beaconing
candidate. Each rule carries `technique`/`tactic` (ATT&CK). **ADR-0004**
(two-plane security architecture), **ADR-0005** (rule engine), **ADR-0006**
(ATT&CK dimension).
**Gate:** fixture-graph → expected-detections unit tests (+ negative fixtures);
de-dup/re-arm asserted; cap/eviction interaction with `max_visible_alerts`
asserted; rule engine runs under a documented budget on the layout-bench scales
(500/1000/2000/5000); no `child_process`/exec in the tree (audited); headless
gates green. **No wire change, no new EventSource, no publish, no egress.**
**Notes:** AUTO; runs in parallel with `v0.5.0`.

#### D2 — Threat-motion vocabulary + Nebula source + purple-team origin (AUTO)
**Goal:** distinct visual + motion semantics per attack class, the first external
security-tool source, and disambiguation of authorized pentest activity from real
threats.
**Deliverables:** ATT&CK-tactic-driven motion in `render/` (worm-spread along
edges, C2 periodic beacon pulse, lateral-movement traversal sweep, exfil outbound-
weighted flow, brute-force rapid edge flashes) — new `theme.rs` constants, no
ad-hoc colours; **`nebula` `EventSource`** tailing `~/.local/share/nebula/logs`
(the `suricata_eve` pattern, pure parse + fixture); a viewer-side **origin tag**
(`observed` vs `red_team`) derived from the source stream, rendered as a distinct
edge/node treatment so a purple-team view shows your engagement and real attacker
activity in one scene. **ADR-0009** (purple-team origin).
**Gate:** per-class motion degrades to Minimal cleanly; `nebula` parse fixture +
count/severity assertion; origin tagging unit-tested from a fixture stream;
headless gates green. New collector → its own MP; **still no wire change** (origin
is viewer-side; Nebula emits existing node/edge kinds).
**Notes:** AUTO. Nebula is *offensive* — SpaceGraph **observes** it; *launching* a
Nebula engagement is a Smolit/AdminBot action (O-9), not a SpaceGraph feature.
**Sibling D2-class sources (each its own MP, all passive/read-only, no exec):**
a **firewall** source (nftables via **read-only netlink — never an `nft`/`iptables`
shell-out**) feeding the *firewall-as-gate* visual (allowed paths glow, blocked
flare red); a **traffic-flow** source (conntrack / nftables byte counters; eBPF
stays deferred per the roadmap) feeding *traffic-as-flow* (beam weight ∝ bytes/s,
exfil = fat outbound beam through the membrane). Both emit existing kinds /
viewer-side fields — **no wire change.**

#### D3 — Multi-stage correlation / campaign aggregation (AUTO)
**Goal:** an attack is a *sequence* of detections over time, not isolated alerts.
**Deliverables:** a viewer-internal **campaign** aggregation that links related
detections (shared subject node / temporal window / ATT&CK tactic progression)
into one tracked chain, rendered as a highlighted path through the graph + a lane
on the timeline; no new wire type (aggregation is over already-ingested alerts).
First-class `Campaign` *node* (and thus a wire bump) is deferred — revisit only if
a published campaign object is needed (then it follows O-8).
**Gate:** fixture detection sequence → one campaign (not N); negative fixture must
not chain; de-dup/re-arm across ticks; headless gates green.
**Notes:** AUTO, viewer-side. **ADR-0007** (correlation model).

#### D4 — Node-model extension + boundary primitive + AI-fabric + vitals  ← rides protocol 4, behind v0.6.0
**Goal:** open the node model once so it can represent anything (VM, Container,
Agent, Model, and classes not yet imagined), render boundaries/containment as
space, make AI activity recognizable, and carry live telemetric state — over the
established `PROTOCOL_VERSION` 4 (the 3→4 bump was spent by v0.5.2 FS-search,
ratified ADR-0016), after which new classes are data, not wire changes (O-10).
**Reality-Check-Gate:** confirm the orchestrator hub re-exports tool-call traffic
in a tappable, payload-opaque shape (`mcp__<server>__*`). The protocol-4 migration
(v0.5.2 FS-search) already proves the `Hello`-mismatch reject works (a v3 peer
decodes to 0 and is rejected).
**Architecture (one coherent node-model extension over protocol 4, designed
together to avoid piecemeal churn — O-8):**
- **Closed-core + open-extension node model.** Core kinds
  (`Process/File/User/Socket/RemoteHost/Alert`) stay first-class and
  hand-designed; a generic **`Node::Entity { class, attrs }`** + a class-
  registration mechanism carries the long tail as *data*. New `EdgeKind`s
  `Contains`/`RunsIn` (containment) + `Invokes`/`Reasons`/`Proposes`/`ToolCall`
  (AI). `Inference`/`ToolCall` are **edges/events**, not nodes (bound growth).
- **Derived-visual function.** An extension class declares a `VisualHint` (a
  `family` archetype + a few axis scalars + a stable class-id hash); the renderer
  *derives* a cached, deterministic mesh/material per class (within the family's
  envelope, distinguishable-but-related), with **hand-authored overrides** for the
  classes that matter (VM, Container). Degrades to the flat sphere under Minimal.
  Binding rule: derived appearance must encode *real* semantics, never noise.
- **Boundary / containment render primitive.** `Contains`/`RunsIn` defines
  parent→child; the renderer draws children inside a translucent boundary hull —
  **one primitive serving the internet membrane (outermost boundary, gateway =
  its portal), VM/Container nesting, and trust zones.** Enables semantic zoom
  (collapse containers→VMs→hosts→zones).
- **Virtualization source.** A `virt` `EventSource` (libvirt / Docker / LXD /
  Qubes qrexec — read-only) emits `Entity` classes (VM, Container, Pod, Namespace)
  + `Contains` edges.
- **AI-fabric.** `Agent`/`Model` as registered classes (or core kinds), sourced
  primarily from the orchestrator MCP tap (ESN agents already flow through it with
  `correlation_id`), secondarily from `nebula`/local-inference processes.
- **Telemetric state & vitals.** Per-process CPU/RSS/threads/state from
  `/proc/<pid>/stat|statm` and a host-vitals message (load/mem/swap/disk/
  throughput from `/proc/stat|meminfo|loadavg` + statvfs + `/proc/net/dev`) — read
  via procfs/sysfs the agent already reads, **never via subprocess spawn** (no-exec
  rule). Encoded as **node vitality** (pulse/size/instability) *and* read as
  **numbers** in the inspector + a **system-vitals HUD** (the htop header, in
  scene). The goal is to subsume every tool's *data* into the one scene, not to
  clone their UIs.
**Gate:** fixture MCP tool-call stream → rendered agent→model→target; fixture
`virt` topology → nested boundary regions; a derived class fixture → a
deterministic, Minimal-degrading visual; vitals fixture → vitality encoding +
correct readout numbers; `PROTOCOL_VERSION = 4` handshake-checked end to end; any
D4 schema migration documented; headless gates green.
**Notes:** **deferred behind v0.6.0** — the MCP tap presupposes the provider
surface (the wire is already at protocol 4; the 3→4 bump was spent by v0.5.2
FS-search, ratified ADR-0016). Any further wire change D4's schema needs is
governed (O-8). Large phase (L–XL); the boundary primitive alone is layout +
render work. **ADR-0008** (node-model extension + boundary + vitals).

#### D5 — ATT&CK coverage heatmap + posture score (AUTO)
**Goal:** "how well am I covered / how exposed am I."
**Deliverables:** an ATT&CK-Navigator-style **coverage view** (which techniques
the rule corpus can detect → gaps), a **posture/exposure score** derived from
coverage + observed attack-surface (open listeners, unusual outbound, alert
density), and a pentest-engagement **report view** (graph slice of a Nebula
engagement). Read-only computation over the in-memory graph + rule registry.
**Gate:** coverage completeness asserted (every rule maps to a technique; the
view lists detected vs undetected techniques); posture score deterministic over a
fixture graph; headless gates green.
**Notes:** AUTO, after a meaningful rule corpus exists (D1/D2/D3). Folds into the
Reporting layer; retention of historical posture is `v0.9.0`/OceanData.

#### D6 — Incident / case object (viz + OceanData persistence)
**Goal:** group related detections/campaigns into an **incident** with state
(`open`/`investigating`/`contained`/`closed`), assignee, notes — the triage/
investigation workflow object.
**Deliverables:** a viewer-side incident object aggregating campaigns + alerts +
the operator's notes and actions taken; persisted via the OceanData sink
(`v0.9.0`) for retention/reopening; ABrain-assisted triage summary (`v0.8.0`)
attached as a `note` annotation.
**Gate:** incident lifecycle transitions unit-tested; round-trip to the OceanData
sink and back; headless gates green.
**Notes:** **depends on v0.9.0** (persistence) and benefits from `v0.8.0`
(triage). **ADR-0010** (incident/case object).

### Track E — Active Reconnaissance (the scanner; NOT auto-mode)

The offensive/recon plane (ADR-0013). A **separate `spacegraph-scanner` crate**
with egress, `CAP_NET_RAW`, and a **hard scope gate** — it refuses to run without
an explicit `Scope`. **Both** scan modes (own/authorized default + internet-wide
opt-in, audited). The scanner **gathers** — it does **not** exploit
(exploitation = a separate later decision, out of Track E). **Every Track-E phase
is NOT auto-mode**, its own master-prompt with hard-stops and a scope-policy
review; **CI/dev tests scan only loopback / RFC5737 documentation ranges
(192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24), never real third-party targets.**
Discovered infrastructure is `Entity`-class (O-10) → full graph integration rides
D4; initial viz reuses `RemoteHost` + the mirrored D0 aperture vocabulary.

#### E1 — Scanner crate + scope model + discovery + port scan  ← first scanner MP
**Goal:** the active plane exists, scope-gated, doing host discovery + port
scanning, with the discovered surface rendered as outward apertures.
**Architecture:** new crate `crates/spacegraph-scanner`; a first-class `Scope`
(target CIDR sets + RoE metadata + mode flag + rate/aggressiveness) that the
scanner **must** be given or it refuses; native Rust discovery (ICMP/ARP/SYN
sweep) + SYN port scan (configurable rate) via raw sockets (`pnet`); an audit
record per scan; emit discovered hosts/open-ports to the viewer over the scanner's
own contract, rendered initially as `RemoteHost` + outward apertures (mirrored
D0). **ADR-0013** + **ADR-0014** (scanner architecture, authored at this phase).
**Gate:** scope gate enforced (no scope → refuses, asserted); discovery + SYN scan
unit-/integration-tested against **loopback / a local test listener / RFC5737
ranges only**; rate-limit honoured; audit record emitted per scan; **no
exploitation code anywhere** (audited). NOT auto-mode.

#### E2 — Service / banner fingerprinting + TLS/cert inspection
Native banner grabbing + protocol probes (zgrab-class), TLS handshake + cert chain
extraction; discovered services carry version/banner/cert as `Entity` attrs.
Optional wrap of `zgrab2` where reimplementing is wasteful (the scanner may exec;
the agent may not). Tests against local services / doc-ranges. NOT auto-mode.

#### E3 — OS detection + CVE correlation
TCP/IP-stack fingerprinting (OS guess); correlate discovered service versions to a
**vendored** CVE dataset (no live egress for the correlation itself — the *scan*
egresses, the lookup is local). CVE-tagged services glow on the surface. NOT
auto-mode.

#### E4 — Searchable recon index (the "Shodan search")
A persistent, queryable store of discovered hosts/services/certs/vulns with
faceted search ("all exposed X", "all hosts running version Y", historical
deltas). Retention via OceanData (`v0.9.0`) or a dedicated store (decide at the
phase). The search surface mirrors the v0.5.0 query-DSL. NOT auto-mode.

#### E5 — Red-team / engagement features
Scope/RoE management UI, multi-engagement, scan scheduling, **stealth/evasion
options** (slow/distributed/fragmented to evade IDS), and **pentest reporting**
(the engagement as a graph slice + an exportable report — a commercial
deliverable). NOT auto-mode.

#### E6 — Full Entity-class graph integration (rides D4)
Promote discovered infra to first-class `Entity` classes (ScannedHost / Service /
Cert / Vuln) under the D4 extension model; the unified estate + recon scene
(defended inside + scanned outside in one boundary-nested space). Depends on D4.
NOT auto-mode.

> **Exploitation is explicitly out of Track E.** Shodan-class = recon. Offensive
> *action* (weaponizing CVEs, gaining access) is a distinct, more sensitive
> capability — a separate later decision, likely via Nebula/Smolit, not built
> here.

---

## 4. Sequencing — chronological version ladder + Track D overlay (O-6)

The version ladder is worked chronologically; no Kickstarter coupling. Track D
runs opportunistically in parallel and publishes its findings through Track B's
MCP surface and Track C's approval layer.

```
v0.3.x → v0.4.0 → v0.5.0 → v0.6.0  → v0.6.x → v0.7.0  → v0.7.x → v0.8.0 → v0.9.0
 close    node     UX/      MCP svc   Cockpit  AdminBot  SOAR     ABrain   Ocean-
 out      detail   shell    + ESN     tab      (NOT      playbk   reason   Data
 (done)   (done)   (now)    admit              auto)     (NOT     (NOT     history
                                                          auto)    auto)    (NOT auto)
└────── Track A: no ESN dep, auto-safe ──────┘ └─────────── Tracks B / C: ESN fabric ───────────┘

Track D (security analytics & visualization), parallel overlay:
  D0 perimeter & exposure pass  ─── AUTO, no wire   → near-term, parallel to v0.5.0
  D1 rule engine + ATT&CK tags ──┐ AUTO, no wire    → next MP, runs alongside v0.5.0
  D2 threat-motion + Nebula      │ AUTO, no wire     → after/with D1
     + firewall + flow sources   ─┤ AUTO, no wire     → sibling MPs (passive, no exec)
  D3 multi-stage correlation     ─┘ AUTO, no wire    → after D1
  D4 extension model + boundary  ─── rides proto 4   → DEFERRED behind v0.6.0 (MCP tap)
     + AI-fabric + virt + vitals      3→4 spent by      further bumps governed (O-8)
                                      v0.5.2 FS-search
  D5 ATT&CK coverage + posture   ─── AUTO            → after a rule corpus exists
  D6 incident/case object        ─── needs persist   → after v0.9.0 (OceanData)

Track E (active reconnaissance — the scanner), NOT auto-mode:
  E1 scanner crate + scope + scan ─── NEW egress crate → first scanner MP (scope-gated)
  E2 fingerprint + TLS/cert       │
  E3 OS detect + CVE correlation  ┤ each NOT auto      → tests: loopback/RFC5737 only
  E4 searchable recon index       │
  E5 red-team / engagement / report
  E6 full Entity-class viz        ─── rides D4
```

Track A is auto-safe end to end; `v0.6.0` is read-only/low-risk and auto-capable;
**`v0.7.0`/`v0.7.x`/`v0.8.0`/`v0.9.0` and all of Track E are the security-sensitive
/ NOT-auto phases**, each with its own hard-stop master-prompt. D0/D1/D2/D3/D5 are
auto-safe; D4 and D6 inherit their dependency's constraints. **Track E is the
active/offensive plane — never auto-mode, scope-gated, scans only loopback/RFC5737
in CI/dev.**

---

## 5. Way-of-Working discipline (binding)

- **Plane invariant (O-7', supersedes O-7 — see ADR-0013).** The **agent**
  (`spacegraph-agent`) never acquires an outbound-network egress path and never
  executes a system action itself; its read-only / no-egress / **no-exec**
  guarantee is preserved and enforced by review. Egress and active probing live
  **only** in the dedicated **`spacegraph-scanner`** crate (the active recon
  plane), which is scope-gated and audited. Defensive enrichment requiring egress
  (threat-intel/CVE/reputation) is still produced Smolit-side and consumed as
  read-only annotations. The scanner may exec (e.g. wrap zgrab2); **the agent may
  not.** A PR adding a network client or `child_process`/exec to the *agent* is
  rejected.
- **Scanner scope gate (O-11).** The scanner refuses to run without an explicit
  `Scope`; both own/authorized (default) and internet-wide (explicit, audited)
  modes are supported; every scan is audited (what/when/scope/authorization). The
  scanner gathers — it does not exploit. CI/dev scans target only loopback /
  RFC5737 documentation ranges.
- **Wire-stability (O-8).** The single sanctioned 3→4 bump is **spent** — v0.5.2
  FS-search took it for the search/materialise messages (ratified ADR-0016), so
  `PROTOCOL_VERSION = 4` is the baseline (`MIN_COMPATIBLE_PROTOCOL = 3`; a v3 peer
  is still rejected by the `Hello` handshake). No further `spacegraph-core`
  schema/`PROTOCOL_VERSION` change without governance review. Track-D work
  (D0/D1/D2/D3/D5) reuses existing node/edge kinds (detections → `Node::Alert`;
  ATT&CK + origin = viewer-side fields) and **adds no wire change**; D4's own
  extension schema is evaluated when D4 is designed (behind v0.6.0).
- **Passive-until-gated (O-9).** No scan/probe/exploit *triggering* from
  SpaceGraph until the `v0.7.0` AdminBot approval layer; until then external
  tools run independently and SpaceGraph ingests their output (the `suricata_eve`
  / `nebula` pattern).
- **ATT&CK tagging.** Every detection rule declares a `technique`/`tactic`; a rule
  with no mapping fails review. The technique↔rule table is the coverage view's
  single source of truth (D5).
- **State via the source, not the tool (O-7 corollary).** Live telemetric state
  (CPU/mem/IO/load/disk/throughput) is read from procfs/sysfs the agent already
  reads, as node/host *attributes* — **never** by spawning `top`/`htop`/`ss`/`df`
  or any subprocess (that is `child_process`/exec, forbidden). SpaceGraph subsumes
  each tool's *data* into the one scene; it does not clone tool UIs.
- **Derived-visual discipline (O-10).** Extension-class visuals are *derived*
  deterministically from a `VisualHint` within a family envelope; derived
  appearance must encode *real* semantics (never random noise), be reproducible
  (same class → same look), and degrade to the flat sphere under Minimal. The
  hand-designed core primitives are never replaced — derivation augments the long
  tail only.
- **Reality-Check-Gate** before every phase touching an external ESN repo: read
  that repo's README + relevant ADRs + verify the live API surface; record the
  check in the phase's RUNLOG entry.
- **Hard-pin cross-repo contracts.** SpaceGraph waits on contract updates from
  ABrain/AdminBot/OceanData/orchestrator rather than inventing adapters. Adapter
  per client, version-tagged. Cross-repo issues for SpaceGraph's consumer
  contracts.
- **ADR per decision, MP per work package, `CONSUMERS.md §3` per relationship.**
  ADR numbering SpaceGraph-local — see the §7 ledger. Conventional commits,
  English, imperative.
- **Naming hygiene / existing-code-first / archive-not-delete / no AI authorship
  markers** (existing AGENTS.md rules carry over). No `enhanced`/`advanced`/`v2`/
  `pro` suffixes; extend existing files before adding new ones; replaced files go
  to `docs/archive/<date>-<reason>/`, never silently deleted.
- **Quality gates** before every commit: `fmt --check`, `clippy --workspace
  --all-targets -D warnings`, `test --workspace`; no `unwrap`/`expect` in
  render/IPC paths.
- **Auth posture L1:** loopback / UDS, JWT bearer where used, minters omit
  `aud`/`iss`, one secret per node; RS256/JWKS is L3, out of scope until ESN-Auth
  Phase B lands.
- **Action discipline:** `Decision → Review → Approval → Execution → Audit` is
  mandatory for every mutating action; onboard one action per PR with its own
  threat-model + tests; "approve and execute" is never one click; playbooks
  (`v0.7.x`) compose only already-onboarded actions.

---

## 6. Decisions

**Resolved:**

| ID | Decision | Resolution |
|---|---|---|
| **O-3** | AdminBot reach (`v0.7.0`) | **Direct AdminBot IPC peer** — host-local agent speaks AdminBot-wire directly, no Smolit-Assistant hop |
| **O-5** | Formal ESN repo-system admission | **Yes** — 7th Hexa-Repo member at `v0.6.0`; `INTERFACE_INVENTORY.md` row + **Tier 3**, promotion to Tier 2 at `v1.0` |
| **O-6** | Kickstarter coupling | **None** — chronological version ladder (§4) |
| **O-7** | ~~Egress / enrichment ownership~~ | **SUPERSEDED by ADR-0013 / O-7'.** (Was: Smolit-side only, SpaceGraph never gains egress.) |
| **O-7'** | Egress ownership (revised) | **The `spacegraph-agent` stays egress-free / no-exec; egress + active probing live only in the dedicated scope-gated `spacegraph-scanner` crate.** The two-plane line moves to intelligence/recon (SpaceGraph) vs system-action (Smolit). The agent's read-only guarantee is preserved. (ADR-0013) |
| **O-8** | `spacegraph-core` wire-bump governance | **The 3→4 bump is spent** (v0.5.2 FS-search; ratified ADR-0016) — `PROTOCOL_VERSION = 4` is the baseline, `MIN_COMPATIBLE_PROTOCOL = 3`. No further `spacegraph-core` schema/`PROTOCOL_VERSION` change without governance review. D4's node-model-extension schema (`Entity`, new `EdgeKind`s, vitals) is evaluated at D4 — additively over protocol 4 where the `MIN_COMPATIBLE` scheme allows, else a governed bump; D4 stays gated behind `v0.6.0` (MCP tap). The **scanner has its own contract** (not the agent wire); discovered infra is `Entity`-class. |
| **O-9** | Scan/probe trigger posture | **Re-scoped.** *AdminBot-driven* system actions stay passive until `v0.7.0` (unchanged). *Reconnaissance scanning* is now a first-class SpaceGraph capability via the scope-gated scanner (Track E, O-11) — distinct from system-action triggering. |
| **O-10** | Node-model extensibility | **Closed-core + open-extension.** Core kinds stay first-class/hand-designed; a generic `Entity{class,attrs}` + class registration carries the long tail as *data*; visuals for extension classes are *derived* within design envelopes (deterministic, semantic, Minimal-degrading). Carried over protocol 4 (the baseline since v0.5.2); D4 implements the extension model behind `v0.6.0`, thereafter new classes need no further bump. |
| **O-11** | Scanner scope / authorization | **Both modes, scope-gated (ADR-0013).** A first-class `Scope` (CIDR sets + RoE + mode + rate); the scanner **refuses to run without an explicit scope**; own/authorized is the default, internet-wide is an explicit operator-owned **audited** mode; every scan is audited. The scanner **gathers, does not exploit** (exploitation deferred). CI/dev scans only loopback/RFC5737. |

**Still open — non-blocking:**

| ID | Decision | Blocks | Recommendation |
|---|---|---|---|
| **O-1** | UX house-standard: token parity only, or deeper Smolitux alignment? | `v0.5.0` | Parity only (Bevy/egui can't consume React components) |
| **O-2** | Cockpit embed mechanism for a native Bevy window | `v0.6.x` | Thin React panel over MCP + launch native window (no iframe) |
| **O-4** | OceanData history: asset/audit model, or a dedicated time-series sink? | `v0.9.0` | Start with asset/audit; revisit if query shape demands a TS store |

---

## 7. ADR ledger (SpaceGraph-local)

SpaceGraph ADRs are `ADR-NNNN` (4-digit, zero-padded), **local to this repo**;
foreign-repo ADRs are always cited with their repo prefix (e.g.
`Smolit-Assistant ADR-0005`, `OceanData ADR-0006`). Reserved slots are authored
at their phase's master-prompt (with the Reality-Check), per §5.

| ADR | Subject | Status / phase |
|---|---|---|
| ADR-0001 | SpaceGraph MCP provider surface | reserved — author at `v0.6.0` |
| ADR-0002 | Actions via AdminBot, not a native channel | reserved — author at `v0.7.0` (+ ADR per onboarded action) |
| ADR-0003-abrain | ABrain reasoning adapter (MCP vs HTTP) | reserved — author at `v0.8.0` |
| ADR-0003-oceandata | OceanData history sink + context SPI | reserved — author at `v0.9.0` |
| **ADR-0004** | Security-analytics two-plane architecture | **PARTLY SUPERSEDED by ADR-0013** (§O-7); **§O-8 amended by ADR-0016** (protocol-4 ratification); the two-plane discipline + agent read-only guarantee are retained/re-scoped |
| **ADR-0005** | **Graph-native detection rule engine** | authored |
| **ADR-0006** | **MITRE ATT&CK detection & coverage dimension** | authored |
| ADR-0007 | Multi-stage correlation / campaign model | reserved — author at D3 |
| **ADR-0008** | **Node-model extension + boundary primitive + AI-fabric + vitals** | authored — implemented at D4 (behind `v0.6.0`) |
| **ADR-0009** | **Threat-motion + purple-team origin** | **authored** — implemented at D2-core |
| ADR-0010 | Incident / case object | reserved — author at D6 |
| ADR-0011 | SOAR playbooks (gated, compose-only) | reserved — author at `v0.7.x` |
| **ADR-0012** | **Perimeter & exposure visual model** (port-state/exposure/anomaly, AUTO) | authored — implemented at D0 |
| **ADR-0013** | **Active reconnaissance plane** (supersedes ADR-0004 §O-7; locks O-7'/O-11; `spacegraph-scanner`) | **authored (this cycle)** — implemented across Track E |
| ADR-0014 | `spacegraph-scanner` technical architecture (engine, scope object, native-vs-wrap, data contract) | reserved — author at E1 |
| ADR-0015 | Licensing (dual-license AGPL+Commercial) + authorized-use EULA | reserved — **release-blocking** for the commercial product (Sam/Johanna) |
| **ADR-0016** | **FS-search baseline reconciliation** (ratify `PROTOCOL_VERSION 4`; restore agent no-exec) | **authored (this cycle)** — amends ADR-0004 §O-8; reaffirms O-7' |

---

## Appendix A — Assumptions to verify

- **A.1** ABrain / OceanData / AdminBot / orchestrator APIs are in active
  development with their own roadmaps. Adapter per client, version-tagged;
  cross-repo issues for SpaceGraph's consumer contracts.
- **A.2** AdminBot's `adminbot_action_request` vocabulary is the binding set;
  SpaceGraph onboards a subset, one at a time; playbooks compose only that subset.
- **A.3** The orchestrator hub proxies stdio-MCP servers as `mcp__<server>__*`
  and re-exports tool-call traffic in a payload-opaque, tappable shape (verify for
  D4).
- **A.4** Cockpit consumes peers over a process/network boundary only; SpaceGraph
  exposes nothing requiring shared code or a foreign subprocess spawn.
- **A.5** Nebula writes engagement logs to `~/.local/share/nebula/logs` (BSD-2-
  Clause); verify the log schema before building the `nebula` `EventSource` (D2).
- **A.6** libvirt / Docker / LXD / Qubes qrexec expose read-only topology
  (running VMs/containers + their nesting) without requiring a privileged write or
  a subprocess shell-out; verify the read path per backend before the `virt`
  source (D4). nftables exposes the ruleset over read-only netlink (NFNL) without
  shelling to `nft` (D2 firewall source).
- **A.7** The procfs/sysfs fields for vitals (`/proc/<pid>/stat` utime/stime +
  `statm` RSS; `/proc/stat`, `/proc/meminfo`, `/proc/loadavg`, `/proc/net/dev`;
  statvfs per mount) are readable under the current `AgentMode::User` posture for
  the host's own processes; privileged cross-user detail follows the existing
  `AgentMode::Privileged` path (D4 vitals).
