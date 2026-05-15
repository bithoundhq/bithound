# Bitcoin / LND / Elements — Incident Catalog (v0 draft)

**Purpose.** This is the seed knowledge base for the observability tool. Each
incident is described in the same shape so it can be mechanically translated
into a detector + diagnosis + suggested action.

**Schema.**
- **Symptom.** What an operator notices first (often a metric anomaly or a
  silent pathology with no obvious surface signal).
- **Signals.** The combination of observable values that distinguishes this
  incident from look-alikes. This is what the detector consumes.
- **Diagnosis.** Plain-language description of what's actually wrong and why.
- **Action.** What the operator should do, including "wait and see" when that's
  correct.
- **False positives / look-alikes.** Things that produce similar signals but
  require different responses.
- **Source(s).** Where this incident is documented in the wild.

---

## Category A: Bitcoin Core (bitcoind)

### A1. Tip lag — node believes it is in IBD when it shouldn't be

**Symptom.** `getblockchaininfo.initialblockdownload == true` even though tip is
recent. Node refuses to serve blocks; LND/Elements peg-ins stall. Wallet
operations may be blocked.

**Signals.**
- `getblockchaininfo.initialblockdownload == true`
- `getblockchaininfo.headers - getblockchaininfo.blocks` is small (e.g. < 1000)
- `getblockchaininfo.verificationprogress > 0.999`
- Time since last accepted block is bounded but nonzero (e.g. minutes to hours)
- Peer count is healthy (`getconnectioncount` ≥ 8)

**Diagnosis.** Node fell slightly behind during downtime, but the IBD heuristic
(based on `minimumchainwork` and `maxtipage`) decided we are in IBD. This is a
known foot-gun: in IBD mode the node won't serve blocks and may stall. Reported
on Bitcoin Core v29.0 specifically when the node is offline for more than ~12h.

**Action.** Restart with `-maxtipage=<large value>` (e.g. one year in seconds) or
issue `reconsiderblock` on the current tip. Long-term: track upstream issue.

**False positives / look-alikes.**
- Genuine IBD on a fresh install (peer count low, headers far ahead of blocks).
- Node truly partitioned from network (peer count low, no block progress at all).

**Source.** `bitcoin/bitcoin#32955` (v29.0), `bitcoin/bitcoin#25048`,
`bitcoin/bitcoin#25800`.

---

### A2. IBD stall — block download window starvation

**Symptom.** Sync progresses, then halts at a specific height for minutes.
Resumes only after a peer is disconnected and replaced. Repeats throughout IBD.

**Signals.**
- `getblockchaininfo.verificationprogress` flat over multi-minute window
- Headers ahead of blocks (`headers - blocks > 1024`)
- `getpeerinfo` shows peers with `synced_blocks` lower than node's current need
- Net debug log mentions `BLOCK_STALLING_TIMEOUT`
- Bandwidth usage drops near zero during the stall

**Diagnosis.** Bitcoin Core's 1024-block lookahead window has all blocks
in-flight, but the peer responsible for the critical "next" block is too slow.
The 2-second `BLOCK_STALLING_TIMEOUT` triggers, peer is disconnected, blocks are
reassigned — but if the operator's own connection is slow, the next peer hits
the same timeout. Cascades.

**Action.** Increase peer count (`-maxconnections`), prefer outbound to
geographically diverse peers, or accept the stalls if bandwidth is the
bottleneck. Mostly self-resolves.

**False positives / look-alikes.**
- Disk-bound write stalls during flush (correlate with iostat).
- Validation-bound stalls on assumevalid threshold (CPU pinned).

**Source.** `blog.lopp.net/revisiting-bitcoin-network-bandwidth-issues`.

---

### A3. Outbound peer starvation

**Symptom.** Peer count is low (e.g. 1-2 outbound) for a sustained period. Node
is at risk of being eclipsed — accepting a malicious view of the chain.

**Signals.**
- `getnetworkinfo.connections_out < 8` (Bitcoin Core's default outbound target)
- `getnetworkinfo.networkactive == true` (we haven't disabled networking)
- `getpeerinfo` outbound count low; many `addnode_count` attempts visible in
  debug log
- Tip is stale relative to wall clock (no new blocks for 30+ min during normal
  operation)
- `getnetworkinfo.warnings` may mention reachability

**Diagnosis.** Node is running but not finding enough outbound peers. Causes:
firewall change, ISP blocking port 8333, exhausted addrman, DNS seed failure,
or the start of an eclipse attack. Self-heals via addrman gossip in ~10min on
average but not always.

**Action.** Check firewall and port 8333 reachability. Add manual peers via
`addnode` to known-good nodes. If persists > 1h, restart with `-debug=net`.
If counts drop while previously healthy, investigate ISP / network change.

**False positives / look-alikes.**
- Operator deliberately running with `connect=N` (private setup).
- Tor-only node (different default targets).
- Node was just started and is still warming up.

**Source.** `bitcoinops.org/en/topics/eclipse-attacks`,
`chainnodes.org/blog/troubleshooting-5-common-node-issues-on-bitcoin`.

---

### A4. Mempool full / minrelayfee climbing

**Symptom.** Outgoing transactions silently rejected. RBF bumps that should
work fail. LND force-closes start to fail to propagate.

**Signals.**
- `getmempoolinfo.bytes / getmempoolinfo.maxmempool > 0.95`
- `getmempoolinfo.mempoolminfee > getmempoolinfo.minrelaytxfee`
  (mempool eviction is now setting the floor, not the operator's config)
- Rate of change of `mempoolminfee` is positive over a 10-minute window
- `getmempoolinfo.unbroadcastcount` may be nonzero

**Diagnosis.** Mempool has hit its memory cap (default 300 MB). Bitcoin Core is
evicting lowest-fee transactions and raising the effective minimum relay fee
above the operator's configured floor. Any tx with fee below `mempoolminfee`
will not be accepted, even if it would normally relay.

**Action.** This is usually a network-wide condition (fee spike), not operator
error. Inform downstream consumers (LND, wallet) of the new effective floor.
If chronically full, increase `-maxmempool`. Don't blindly raise — RAM matters.

**False positives / look-alikes.**
- Operator deliberately set a tight `-maxmempool` for resource reasons.
- Single-tx pinning attacks (rare, but check `getrawmempool verbose=true` for
  long descendant chains).

**Source.** `bitcoinops.org/en/blog/waiting-for-confirmation`,
`bitcoincore.academy/mempool-lifecycle.html`.

---

### A5. Reorg detected (small)

**Symptom.** A block previously at the tip is no longer on the active chain.
Confirmed transactions in that block have re-entered the mempool.

**Signals.**
- A block hash that was reported by ZMQ `hashblock` or appeared in
  `getbestblockhash` is no longer reachable from the new tip via parent links
- `getchaintips` shows a `valid-fork` or `valid-headers` entry with branch
  length ≥ 1
- Rate of mempool size change shows a positive spike (transactions returning)
- `MaybeUpdateMempoolForReorg` log lines

**Diagnosis.** Normal 1-2 block reorg from near-simultaneous block discovery.
Routine on Bitcoin (multiple per month). Confirmed transactions may have
become unconfirmed.

**Action.** For most operators: log + alert downstream consumers (LND, custody
systems) that confirmation counts on recent transactions just decreased. No
operational action needed unless > 3 blocks deep.

**False positives / look-alikes.**
- Local restart with re-validation (not a reorg, but
  `verificationprogress` may briefly drop).

**Source.** `cube.exchange/what-is/chain-reorganization`.

---

### A6. Reorg detected (deep) — chain split or attack

**Symptom.** Same as A5 but reorg depth ≥ 3 blocks.

**Signals.**
- `getchaintips` shows a fork ≥ 3 blocks deep
- The reorg includes our previously-confirmed coinbase or high-value txs
- Other operators' nodes (if cross-checked via gossip or block explorers) report
  the same tip — or DIFFERENT tips, which is much worse

**Diagnosis.** Either (a) a genuine deep reorg from a temporary mining stall on
one branch, (b) a chain split following a soft/hard fork the node didn't apply,
or (c) an eclipse attack feeding the node a fake chain. (c) is rare but
catastrophic for any system that took action on the now-unwound blocks.

**Action.** STOP all dependent systems (custody settlements, peg-ins,
withdrawals). Cross-check tip against multiple independent sources (block
explorers, other operators). If tip diverges from consensus, the local node is
eclipsed — restart with fresh peers via `-seednode`. If tip matches consensus,
the network experienced a real deep reorg and downstream systems must reconcile.

**False positives / look-alikes.** Almost none. A 3+ block reorg is always
worth treating as an incident.

**Source.** `bitcoinops.org/en/topics/eclipse-attacks`,
`coinbase.com/blog ETC 51% attack postmortem` (analogous case).

---

### A7. Block database corruption

**Symptom.** Bitcoind crashes on startup or mid-run with messages like
`Corrupted block database detected` or `LevelDB CorruptionError: missing start
of fragmented record`.

**Signals.**
- Process exited non-zero
- Log contains `Corrupted block database detected` or LevelDB error strings
- `df` shows the data directory's filesystem may be near full or had recent
  unclean shutdown (correlate with `dmesg` for I/O errors)
- SMART data on the underlying disk may show pending sectors / reallocations

**Diagnosis.** Storage corruption, almost always from one of: unclean
shutdown (power loss, OOM kill), failing disk, filesystem bug, or running on a
network filesystem that doesn't respect fsync.

**Action.** Try `-reindex-chainstate` first (faster). If that fails,
`-reindex` (rebuilds from blocks/, takes hours). If blocks/ is also corrupt,
check disk health (smartctl) before anything else — restoring onto a failing
disk is wasted time. Recommend SSD on a journaled local filesystem. Never run
data directory on NFS.

**False positives / look-alikes.** None — if you see this message, it's real.

**Source.** `bitcoin/bitcoin#21013`, `#8523`, `#6502`, `#2305`.

---

### A8. Validation-bound — high CPU, slow tip advance

**Symptom.** Node accepts blocks but with multi-second delays. Tip advances
but lags wall clock by minutes.

**Signals.**
- CPU pinned (process metric)
- `getblockchaininfo.verificationprogress` advances slowly
- Block-acceptance latency (time between ZMQ `hashblock` and `getbestblockhash`
  reflecting it) > 2s sustained
- DBcache memory usage at configured max

**Diagnosis.** Either CPU is genuinely undersized for current chain workload,
or `dbcache` is too small forcing repeated UTXO disk reads, or another process
is contending for CPU/IO.

**Action.** Increase `-dbcache` if RAM allows. Check for noisy neighbors (other
processes). Validate that signature checks are using all available cores
(`-par=0` for auto).

**False positives / look-alikes.**
- IBD assumevalid threshold crossing (signature checks resume).
- Block with abnormally heavy script load (rare).

---

## Category B: Lightning (LND)

### B1. Channel inactive — peer offline

**Symptom.** A channel that was previously usable shows `active: false` in
`listchannels`. Routing through it fails.

**Signals.**
- `lnrpc.ListChannels` entry with `active == false`
- `lnrpc.GetNodeInfo` on the peer pubkey shows no recent updates
- `lncli getinfo` peer connection list does not include this pubkey

**Diagnosis.** Peer is offline OR the TCP connection between us was dropped
(NAT timeout, ISP issue, peer restart). LND will retry reconnect with backoff.

**Action.** Wait 5-15 minutes — most resolve automatically. If persists > 1h,
attempt manual `connect`. If peer is unreachable for > 24h with HTLCs in flight,
prepare for force-close (see B3).

**False positives / look-alikes.**
- Our own node restarted recently — give the channel manager time to reconnect.
- `chan_status_flags` includes `ChanStatusBorked` — this is much worse, not a
  simple peer offline.

**Source.** `lightningnetwork/lnd#7974`, `stacker.news/items/17752`.

---

### B2. HTLC stuck mid-flight

**Symptom.** A pending HTLC has been in `pending_htlcs` for longer than
expected. CLTV is approaching expiry.

**Signals.**
- An entry in `lnrpc.ListChannels.pending_htlcs` with `expiration_height`
  approaching `getblockchaininfo.blocks`
- Same payment_hash visible on incoming side but not outgoing side, OR vice
  versa (asymmetric — payment is wedged at a hop)
- HTLC age > N minutes (operator-configurable, default e.g. 10)
- Channel `active == false` while HTLC is pending — HIGH RISK

**Diagnosis.** Downstream peer disconnected mid-route, OR a routing peer is
behaving badly, OR a deliberate channel-jamming attack. If CLTV expires before
resolution, force-close is the protocol-mandated outcome.

**Action.** If channel is still active, watch and wait. If
`expiration_height - tip < 13` (LND's default `--final_cltv_delta` plus
margin), prepare for inevitable force-close — verify on-chain fees are
reasonable so the close tx will confirm. Consider `lncli wallet bumpfee` on
anchor outputs after the close.

**False positives / look-alikes.**
- Long-lived hold invoice (legitimately pending HTLCs, expected behavior).
- Submarine swap in flight.

**Source.** `lightningnetwork/lnd#3604`, `#2021`, `#6037` (jamming).

---

### B3. Force-close initiated (ours)

**Symptom.** A channel transitions from active to `pending_force_closing`. A
commitment transaction with our funds is broadcast on-chain.

**Signals.**
- `lnrpc.PendingChannels.pending_force_closing_channels` newly contains an
  entry
- `closing_txid` appears in our local mempool
- Our `walletbalance.confirmed` balance briefly decreases (anchor reserves)

**Diagnosis.** Either (a) we initiated due to peer unresponsive with HTLC in
flight, (b) peer initiated and we're observing the broadcast, or (c) LND's
contract court detected an irrecoverable state (revoked commitment).

**Action.** Verify the close tx is in our mempool and propagating. If fee is
too low for current conditions (tx not confirming after 6 blocks), bump via
CPFP on the anchor output: `lncli wallet bumpforceclosefee`. Track until
all outputs are swept (`limbo_balance == 0`). Funds in limbo for >2 weeks
indicates a stuck close tx (see B4).

**False positives / look-alikes.**
- Cooperative close — different RPC path, different fee dynamics, no anchor
  sweep needed.

**Source.** `lightningnetwork/lnd#7670`, `lightningnetwork/lnd#7779`.

---

### B4. Stuck force-close (cannot bump fees)

**Symptom.** Force-close transaction has been in pending state > 1 week.
Limbo balance non-zero. `bumpforceclosefee` returns errors or does nothing.

**Signals.**
- `pending_force_closing_channels[].closing_txid` not in our local mempool
  (`getrawtransaction` returns -5)
- BUT a different close tx for the same channel point IS confirmed or in
  another node's mempool (visible via block explorer API)
- Time since force-close initiation > 7 days
- `chan_status_flags` may include `ChanStatusLocalCloseInitiator`

**Diagnosis.** Both peers broadcast competing close transactions. They are
mutually-conflicting (same inputs). Each node sees only its own tx in its
local mempool. Neither can bump the other's. The "winning" tx is whichever
miners eventually confirm, which depends on which has higher feerate at the
time miners look.

**Action.** Use `chantools sweepremoteclosed` to bump via external mempool
data. Alternatively, query a public mempool API (mempool.space,
blockstream.info) to see which close tx is actually circulating, and CPFP
the right one. Operator-curated playbook: this scenario is documented well in
the Umbrel community guide.

**False positives / look-alikes.** None at this depth — this is a specific
identifiable failure mode.

**Source.** `lightningnetwork/lnd#7779`,
`community.umbrel.com/t/the-guide-where-your-lightning-close-transaction-cant-get-the-channel-closed/15096`.

---

### B5. Channel jamming attack in progress

**Symptom.** Channel reports max HTLCs (close to 483) all pending. Channel
appears active but cannot route.

**Signals.**
- `pending_htlcs.length` near max (default 483, configurable)
- HTLCs share patterns: same upstream peer, similar amounts, all very young or
  cycling (one resolves, another appears)
- Forwarding revenue dropped sharply against historical baseline
- `routing.failed` events log spike

**Diagnosis.** Adversary deliberately filling HTLC slots to deny service,
either to our channel specifically or to a routing path that traverses us.
Currently no robust protocol-level defense (work ongoing).

**Action.** Limited options. Reduce `--max-pending-htlcs` per channel (lowers
attack surface but also legitimate capacity). Consider closing the channel if
the upstream peer is consistently the source. Log the pattern for the broader
community — these attacks are research-relevant.

**False positives / look-alikes.**
- Genuine high-volume routing through us (check forwarding revenue is also
  high — an attack typically yields ~zero successful forwards).

**Source.** `lightningnetwork/lnd#6037`.

---

### B6. Watchtower / chain backend lag

**Symptom.** LND log warnings about `chain backend behind` or watchtower
sessions falling behind.

**Signals.**
- LND's view of best block != bitcoind's (cross-check via
  `lnrpc.GetInfo.block_height` vs `getblockchaininfo.blocks`)
- Difference persistent over multiple poll intervals
- LND log: "Unable to retrieve block" or "chain backend is X blocks behind"

**Diagnosis.** Bitcoin Core is up but slow to respond, OR LND's ZMQ/RPC
connection to bitcoind is degraded, OR bitcoind itself is in IBD mode (see A1).

**Action.** Verify bitcoind health first — most LND chain-lag issues are
actually bitcoind issues. Check ZMQ socket reachability and that LND's
`bitcoind.zmqpubrawblock` config matches bitcoind's bind. Restart LND only
after confirming bitcoind is healthy.

**False positives / look-alikes.**
- LND just started — graceful warm-up period.

---

## Category C: Elements (elementsd / Liquid)

### C1. Peg-in claim failed — insufficient confirmations

**Symptom.** `claimpegin` returns an error about confirmations.

**Signals.**
- The mainchain peg-in tx is confirmed but with `< 102` confirmations
- Operator (or downstream system) attempted `claimpegin` prematurely

**Diagnosis.** Liquid requires 102 mainchain confirmations before a peg-in can
be claimed (high number specifically to survive deep mainchain reorgs).

**Action.** Wait. This is a hard protocol requirement, not a tunable.

**False positives / look-alikes.** None.

**Source.** `docs.liquid.net/docs/technical-overview`,
`help.blockstream.com/.../900001387966`.

---

### C2. Peg-in validation disabled — silent risk

**Symptom.** No symptom. Silent. The node is configured with
`validatepegin=0`.

**Signals.**
- Config inspection shows `validatepegin=0` set explicitly
- This is a configuration-time check, not a runtime metric

**Diagnosis.** The Elements node is not validating that incoming peg-ins
correspond to real locked BTC on the mainchain. For a non-trivial wallet, this
is a serious self-imposed downgrade of security guarantees.

**Action.** Set `validatepegin=1` and ensure a working bitcoind RPC connection
is configured. The exception is well-isolated test environments.

**False positives / look-alikes.**
- Genuinely a test/regtest setup — validate that's actually true.

**Source.** `docs.liquid.net/docs/building-on-liquid`.

---

### C3. Liquid block production stalled

**Symptom.** No new Liquid blocks for several minutes (Liquid targets 1
block/min).

**Signals.**
- `getblockchaininfo.blocks` flat for > 5 min (Liquid is 1-min blocks normally)
- `getbestblockhash` unchanged
- `getnetworkinfo.connections` may be normal — this is a federation-side issue,
  not necessarily our local connectivity

**Diagnosis.** The federation has lost quorum (more than 1/3 of functionaries
offline or partitioned). Block signing has halted. Our node is healthy but the
network itself is paused. This has happened on Liquid before (publicly
documented multi-hour outages).

**Action.** Nothing operationally — this is the federation's problem to fix.
Pause downstream systems that depend on Liquid block timing (e.g.
LBTC-denominated settlement). Subscribe to federation status announcements.

**False positives / look-alikes.**
- Our node lost connectivity to bridge nodes (check peer count).
- Our system clock is wildly wrong.

**Source.** `docs.liquid.net/docs/technical-overview` (federation byzantine
fault tolerance section).

---

## Cross-cutting incidents

### X1. Disk space exhaustion imminent

**Symptom.** Node will crash without warning when disk fills.

**Signals.**
- Free space on data directory's filesystem < 10 GB AND falling
- Rate of growth × time-to-zero < 24h
- Pruning not configured OR prune target close to actual usage

**Diagnosis.** Standard disk-fill. Bitcoin Core does not pre-allocate; it dies
mid-write. Compounded if logs are also on the same volume.

**Action.** Enable pruning (`-prune=N`), move data to a larger volume, or
clean other data on the disk. Predict failure ≥ 24h in advance.

---

### X2. Clock skew

**Symptom.** Various subtle failures: peers banning us, headers rejected,
HTLCs expiring "early."

**Signals.**
- System clock differs from NTP reference by > 70 seconds
  (Bitcoin Core's network-adjusted-time tolerance is 70 minutes, but problems
  start much earlier)
- `getnetworkinfo.timeoffset` magnitude > 30s

**Diagnosis.** NTP failed, VM clock drift, or virtualization hypervisor bug.

**Action.** Restart NTP service / check `chronyc tracking`. On VMs, verify
host time sync is enabled.

---

## Notes for the rule engine

A few patterns emerge from this catalog that should inform the data model:

1. **Many incidents need cross-source correlation.** B6 needs both LND state
   and bitcoind state. C1 needs both elementsd state and bitcoind state. The
   `WorldSnapshot` type should hold all three sources' states together.

2. **Time windows matter.** "Peer count low for > 10 min" is fundamentally
   different from "peer count low right now." The detector framework needs to
   support stateful predicates over rolling windows, not just snapshot
   predicates. This argues for the engine maintaining per-detector state.

3. **Severity is multi-dimensional.** A1 (tip lag) is a yellow flag for a
   hobbyist node and a red flag for a custodial Lightning operator. The same
   detection should yield different urgency levels depending on operator
   profile (configurable).

4. **Look-alikes are the hardest part.** Most of the operational mistakes I've
   seen in the wild come from confusing one of these incidents for another.
   The "false positives / look-alikes" field is where the real value is —
   any rule engine that doesn't surface these alongside the primary diagnosis
   will produce noisy alerts that operators will eventually mute.

5. **Some incidents are config-time, not runtime** (C2). The tool should
   probably do a one-shot config audit at startup and surface those findings
   distinctly from runtime incidents.

## Gaps to fill in v0.1

- LND wallet/macaroon issues (auth misconfig, expired macaroons in clients).
- Tor-specific incidents (onion service down, control port misconfig).
- Watchtower client connectivity (separate from B6 — that's about chain backend).
- Backup failures (SCB not being written / not being readable).
- Indexers (txindex / coinstatsindex sync state divergent from main chain).
- Specific Elements asset-related incidents (asset issuance ledger consistency).

