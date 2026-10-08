# Architecture

## Role

Culvert implements the HDMI 2.1 SCDC (Status and Control Data Channel) protocol. It sits
on top of `hdmi-hal`'s `ScdcTransport` trait and provides typed, structured access to the
SCDC register map: named fields, bitfield structs, and typed operations for scrambling
control, FRL training primitives, and CED (Character Error Detection) reporting.

The relationship to `hdmi-hal` mirrors the relationship of piaf to its input bytes.
`ScdcTransport` moves raw bytes; culvert gives those bytes meaning. The transport is
injected — culvert implements the protocol logic, the caller provides the hardware.

Culvert is a protocol primitive library, not a policy layer. It provides the typed
operations that a link training state machine needs to call. The sequencing of those
operations — when to set the FRL rate, how long to wait for `FLT_Ready`, how to handle
timeout and retry — belongs in the link training crate above.

---

## Scope

Culvert covers:

- a typed SCDC register map: named constants, bitfield structs, and typed values for all
  SCDC-defined registers,
- the `Scdc<T>` client: wraps a `ScdcTransport` and exposes typed read/write
  methods for each register group,
- scrambling control: writing `TMDS_Config`, polling `Scrambler_Status`,
- FRL training primitives: writing `Config_1` (FRL rate, FFE levels) and `Config_0`,
  reading `Status_Flags_0` (`FLT_Ready`, lane lock), the per-lane `LTP` requests and
  `Source_Test_Configuration`, reading and clearing `Update_0` (`FLT_Update`, `FRL_Start`),
- CED reporting: reading per-lane error counters from `ERR_DET` registers,
- version negotiation: reading `Sink_Version`, writing `Source_Version`,
- structured errors: transport errors and protocol-level violations (e.g. an unrecognised
  FRL rate value returned by the sink) surfaced as distinct variants.

The following are out of scope:

- **Async API** — an async variant of the SCDC client will live in a separate
  `culvert-async` crate with its own feature flags. Culvert carries no async surface.
- **Link training state machine** — the sequencing of FRL training (rate selection loop,
  timeout handling, retry logic, fallback to TMDS) belongs in the link training crate.
  Culvert provides the register operations; the state machine decides when to call them.
- **InfoFrame encoding** — a separate crate in the signaling layer.
- **PHY configuration** — `HdmiPhy` operations are the link training crate's concern.
- **I²C / DDC transport** — platform backends implement `ScdcTransport` from `hdmi-hal`.
  Culvert never touches I²C directly.

---

## Dependencies

```
display-types  ─┐
hdmi-hal       ─┴─►  culvert  ──►  frl-training
```

- `display-types` — for `HdmiForumFrl`, the FRL rate enum used in `Config_1`.
- `hdmi-hal` — for the `ScdcTransport` trait.

Culvert does not depend on `piaf` or `concordance`. It is consumed by the link training
crate, which sequences culvert's operations according to the FRL training algorithm.
---

## The SCDC Register Map

SCDC is defined in HDMI 2.1 spec section 10.4. Registers are one byte wide, addressed
by a one-byte offset over DDC/I²C to the sink's SCDC address (0x54).

The register map divides into six functional groups. Bit positions are given per field;
see [Sources](#sources) for how each was established.

**Version** (0x01–0x02)
- `Sink_Version` (0x01, R) — SCDC protocol version supported by the sink.
- `Source_Version` (0x02, W) — SCDC protocol version the source intends to use.

**Update flags** (0x10)
- `Update_0` (0x10, R/W, write-1-to-clear) — change notification flags set by the sink:
  `Status_Update` (bit 0), `CED_Update` (bit 1), `RR_Test` (bit 2),
  `Source_Test_Update` (bit 3), `FRL_Start` (bit 4), `FLT_Update` (bit 5),
  `RSED_Update` (bit 6). The source reads and then clears these to detect sink-side
  state changes without polling every status register on every pass. `RR_Test` is the
  one flag the source must not clear (Intel `xe` series), so `clear_update_flags` never
  writes bit 2.
- `Update_1` (0x11) — no fields are defined by the sources below; culvert does not
  access it.

**TMDS and scrambling** (0x20–0x21)
- `TMDS_Config` (0x20, W) — `Scrambling_Enable` (bit 0) and `TMDS_Bit_Clock_Ratio`
  (bit 1).
- `Scrambler_Status` (0x21, R) — `Scrambling_Status` (bit 0): sink acknowledgement that
  scrambling is active.

**FRL configuration** (0x30–0x35)
- `Config_0` (0x30, W) — `RR_Enable` (bit 0, read request enable) and
  `FLT_No_Retrain` (bit 1).
- `Config_1` (0x31, W) — `FRL_Rate` (bits 3:0, maps to `HdmiForumFrl`) and
  `FFE_Levels` (bits 7:4). Written by the source to request a training rate.
- `Source_Test_Configuration` (0x35, R) — written by the sink (compliance testing) to
  instruct the source: `TxFFE_Pre_Shoot_Only` (bit 1), `TxFFE_De_Emphasis_Only` (bit 2),
  `TxFFE_No_FFE` (bit 3), `FLT_No_Timeout` (bit 5), `DSC_FRL_Max` (bit 6) and `FRL_Max`
  (bit 7). Read by the source when `Source_Test_Update` is set.

**Status** (0x40–0x42)
- `Status_Flags_0` (0x40, R) — `Clock_Detected` (bit 0), `Ch0_Locked` (bit 1),
  `Ch1_Locked` (bit 2), `Ch2_Locked` (bit 3), `Ln3_Locked` (bit 4, FRL 4-lane only),
  `FLT_Ready` (bit 6, sink ready for link training), `DSC_Decode_Fail` (bit 7).
- `Status_Flags_1` (0x41, R) — link training pattern requested for lane 0 (bits 3:0)
  and lane 1 (bits 7:4).
- `Status_Flags_2` (0x42, R) — link training pattern requested for lane 2 (bits 3:0)
  and lane 3 (bits 7:4).

  LTP request values: 0x0 = no pattern (lane trained), 0x1 = all ones, 0x2 = all zeros,
  0x3 = Nyquist clock, 0x4 = Rx DDE compliance pattern, 0x5–0x8 = LFSR 0–3,
  0xE = request FFE change, 0xF = request FRL rate change. Other values are undefined.

**Character Error Detection** (0x50–0x5A)
- `ERR_DET_0_L/H` (0x50/0x51), `ERR_DET_1_L/H` (0x52/0x53), `ERR_DET_2_L/H`
  (0x54/0x55) — per-lane 15-bit error counters with a validity bit (bit 7) in the high
  byte. Counters are read as a pair (low + high byte) to form a single `u16` value.
- `ERR_DET_Checksum` (0x56) — checksum over the CED registers.
- `ERR_DET_3_L/H` (0x57/0x58) — lane 3 counter, same format; only populated in FRL
  4-lane mode.
- `RS_Correction_L/H` (0x59/0x5A) — Reed-Solomon correction count (FRL), same format
  as a CED counter: validity bit (bit 7) in the high byte and a 15-bit count.

### Sources

The register map was originally written from a summary of the spec and was wrong in
several places (FRL configuration in `Config_0`, shifted lock bits, `FRL_Start` and the
LTP requests in the wrong registers, lane 3 CED at 0x56/0x57). The map above was rebuilt
from three independent implementations, which agree wherever they overlap:

- the AMD/Xilinx HDMI 2.1 receiver driver (`embeddedsw`, `v_hdmirx1/src/xv_hdmirx1_frl.c`,
  SCDC field table with address, mask and shift for every field; LTP values from
  `xv_hdmirx1_frl.h`),
- the Linux DRM SCDC helper patch series for HDMI 2.1 fields (LKML, 2026-07, v8),
- the Intel HDMI FRL enablement patch series for the `xe` driver (2026-08), including
  its FRL link training sequence.

`FFE_Levels` is the highest TxFFE level index the source supports: 0–3 for rates up to
12 Gbps and 0–7 for faster rates, per the Intel `xe` series (`drm_scdc_config_frl`) and
AMD's display driver (`hdmi_frl_get_max_ffe_level`). A sink treats a prohibited value as 0.

---

## The `Scdc<T>` Client

The central type is a thin client struct that owns the transport and exposes typed
methods grouped by register function:

```rust
pub struct Scdc<T> {
    transport: T,
}

impl<T: ScdcTransport> Scdc<T> {
    pub fn new(transport: T) -> Self;
    pub fn into_transport(self) -> T;

    // Version
    pub fn read_sink_version(&mut self) -> Result<u8, ScdcError<T::Error>>;
    pub fn write_source_version(&mut self, version: u8) -> Result<(), ScdcError<T::Error>>;

    // Scrambling
    pub fn write_tmds_config(&mut self, config: TmdsConfig) -> Result<(), ScdcError<T::Error>>;
    pub fn read_scrambler_status(&mut self) -> Result<ScramblerStatus, ScdcError<T::Error>>;

    // FRL configuration
    pub fn write_config_0(&mut self, config: Config0) -> Result<(), ScdcError<T::Error>>;
    pub fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), ScdcError<T::Error>>;
    pub fn read_source_test_config(&mut self) -> Result<SourceTestConfig, ScdcError<T::Error>>;

    // Status and update flags
    pub fn read_status_flags(&mut self) -> Result<StatusFlags, ScdcError<T::Error>>;
    pub fn read_ltp_requests(&mut self) -> Result<LtpRequests, ScdcError<T::Error>>;
    pub fn read_update_flags(&mut self) -> Result<UpdateFlags, ScdcError<T::Error>>;
    pub fn clear_update_flags(&mut self, flags: UpdateFlags) -> Result<(), ScdcError<T::Error>>;

    // CED
    pub fn read_ced(&mut self) -> Result<CedCounters, ScdcError<T::Error>>;
    pub fn read_rs_correction(&mut self) -> Result<Option<RsCorrectionCount>, ScdcError<T::Error>>;
}
```

`Scdc<T>` holds no state beyond the transport. Register reads and writes are direct and
stateless from the client's perspective; any sequencing state lives in the caller.

Methods map to register groups, not necessarily individual registers. Two methods span
multiple registers in a single logical operation:

- `read_ltp_requests()` reads `Status_Flags_1` (0x41) and `Status_Flags_2` (0x42) and
  returns the pattern requested for each of the four lanes as one `LtpRequests` struct.
  The sink updates all lanes' requests together and signals the change once through
  `FLT_Update`.
- `read_ced()` reads the four ERR_DET low/high byte pairs (0x50–0x55, 0x57–0x58) in a
  single pass and returns one `CedCounters` struct.

In both cases the method performs a contiguous sequential read with no intervening writes
or protocol state changes. This is distinct from the multi-step sequences (write rate,
poll for ready, handle pattern request) that belong in the link training crate.

---


## Key Types

```rust
pub struct TmdsConfig {
    pub scrambling_enable: bool,
    pub high_tmds_clock_ratio: bool,  // false = /10, true = /40
}

pub struct ScramblerStatus {
    pub scrambling_active: bool,
}

/// `Config_0` (0x30).
pub struct Config0 {
    pub rr_enable: bool,        // bit 0: sink may raise read requests
    pub flt_no_retrain: bool,   // bit 1
}

/// FFE (Feed-Forward Equalization) levels written into `Config_1` bits[7:4]: the highest
/// TxFFE level index the source supports. `FfeLevels::new` rejects values above 7, and
/// `FfeLevels::max_for(rate)` gives the per-rate maximum (3 up to 12 Gbps, 7 above), which
/// `write_frl_config` enforces.
pub struct FfeLevels(u8);

/// `Config_1` (0x31).
pub struct FrlConfig {
    pub frl_rate: HdmiForumFrl,   // from display-types; bits[3:0]
    pub ffe_levels: FfeLevels,    // bits[7:4]
}

/// `Source_Test_Configuration` (0x35), written by the sink.
pub struct SourceTestConfig {
    pub txffe_pre_shoot_only: bool,     // bit 1
    pub txffe_de_emphasis_only: bool,   // bit 2
    pub txffe_no_ffe: bool,             // bit 3
    pub flt_no_timeout: bool,           // bit 5
    pub dsc_frl_max: bool,              // bit 6
    pub frl_max: bool,                  // bit 7
}

/// Link Training Pattern requested by the sink for one lane (a 4-bit field in
/// `Status_Flags_1`/`Status_Flags_2`). An undefined value surfaces as
/// `ProtocolError::UnknownLtpReq`.
pub enum LtpReq {
    None              = 0x0,   // lane trained, no pattern requested
    AllOnes           = 0x1,
    AllZeros          = 0x2,
    NyquistClock      = 0x3,
    RxDdeCompliance   = 0x4,
    Lfsr0             = 0x5,
    Lfsr1             = 0x6,
    Lfsr2             = 0x7,
    Lfsr3             = 0x8,
    FfeChange         = 0xE,   // sink requests an FFE level change
    RateChange        = 0xF,   // sink requests a lower FRL rate
}

/// Per-lane pattern requests from `Status_Flags_1` (0x41) and `Status_Flags_2` (0x42).
pub struct LtpRequests {
    pub lane0: LtpReq,
    pub lane1: LtpReq,
    pub lane2: LtpReq,
    pub lane3: LtpReq,   // LtpReq::None in 3-lane FRL
}

/// `Status_Flags_0` (0x40).
pub struct StatusFlags {
    pub clock_detected: bool,    // bit 0
    pub ch0_locked: bool,        // bit 1
    pub ch1_locked: bool,        // bit 2
    pub ch2_locked: bool,        // bit 3
    pub ln3_locked: bool,        // bit 4, FRL 4-lane only
    pub flt_ready: bool,         // bit 6: sink ready for link training
    pub dsc_decode_fail: bool,   // bit 7
}

/// `Update_0` (0x10), write-1-to-clear.
pub struct UpdateFlags {
    pub status_update: bool,        // bit 0
    pub ced_update: bool,           // bit 1
    pub rr_test: bool,              // bit 2
    pub source_test_update: bool,   // bit 3
    pub frl_start: bool,            // bit 4: training passed, source may start FRL
    pub flt_update: bool,           // bit 5: LTP requests changed
    pub rsed_update: bool,          // bit 6
}

/// A 15-bit character error count decoded from an ERR_DET register pair.
/// The high byte's bit 7 is the validity flag; the counter occupies bits[14:0].
/// `CedCount::value()` returns the raw count as a `u16` (always <= 0x7FFF).
pub struct CedCount(u16);

/// A 15-bit Reed-Solomon correction count from RS_Correction_L/H (0x59/0x5A), same
/// format as a CED counter.
pub struct RsCorrectionCount(u16);

pub struct CedCounters {
    pub lane0: Option<CedCount>,   // None if validity bit not set
    pub lane1: Option<CedCount>,
    pub lane2: Option<CedCount>,
    pub lane3: Option<CedCount>,   // None in TMDS / 3-lane FRL mode
}
```

All output structs are `#[non_exhaustive]` for forward compatibility.

---

## Error Handling

Culvert surfaces two distinct failure categories:

```rust
#[non_exhaustive]
pub enum ScdcError<E> {
    /// The underlying I²C/DDC transport returned an error.
    Transport(E),
    /// The register data violates the SCDC protocol (e.g. an undefined FRL rate value).
    Protocol(ProtocolError),
}

#[non_exhaustive]
pub enum ProtocolError {
    UnknownFrlRate(u8),
    UnknownLtpReq(u8),
}
```

This mirrors the pattern established in piaf: transport failures and protocol violations
are distinct. A caller that only cares about transport health can match on `Transport(_)`;
one that wants to diagnose unexpected sink behaviour inspects `Protocol(_)`.

Both enums are `#[non_exhaustive]` at the type level, consistent with the rest of the
stack. Variants are plain — callers can match `UnknownFrlRate(rate)` without `..`.

`ScdcError` is `#[non_exhaustive]` to allow future variants without a breaking change.

---

## The Culvert / Link Training Boundary

This boundary is worth stating explicitly because the SCDC spec interleaves protocol
mechanics and training algorithm steps.

**Culvert's responsibility:** typed register access. Given a desired FRL rate, write it
into `Config_1`. Given a status register, decode it into `StatusFlags`. Culvert does not
know what to do with a `StatusFlags`; it only knows how to read one.

**Link training's responsibility:** the state machine. Receive a ranked list of FRL tiers
from concordance. For each tier: wait for `FLT_Ready`, write `Config_1`, then on each
`FLT_Update` read the per-lane `LTP` requests and act on them (send the pattern, change
FFE, or drop the rate) until every lane reports no pattern, then wait for `FRL_Start`. That sequencing logic, timeout
handling, and retry policy live in the link training crate — not here.

The rule: if it touches time, state across multiple register accesses, or fallback logic,
it belongs in link training. If it reads or writes registers and returns typed results, it
belongs in culvert.

---

## The `plumbob` Feature

culvert implements `plumbob::ScdcClient` for `Scdc<T>` behind a `plumbob` cargo feature.
This follows the same convention as `serde` feature flags in the ecosystem: the producing
crate reaches toward the consuming crate's trait, rather than the consumer depending on
the producer.

The feature is **temporarily removed**. plumbob 0.1's `ScdcClient` models a single link
training pattern request and waits for `FRL_Start` before the pattern loop, which does not
match the corrected register map (per-lane requests, `FLT_Update`-driven training,
`FRL_Start` only after training passes). It returns once plumbob's training state machine
follows LTS:2 → LTS:3 → LTS:P with per-lane requests.

---

## `no_std` Compatibility

Culvert requires no allocator. All output types are stack-allocated structs. The
`ScdcError<E>` type requires no heap. `Scdc<T>` holds only the transport, which is
caller-owned.

The full API is available in bare `no_std` environments.

---

## Design Principles

- **Typed access, not raw bytes.** Every register read returns a named struct, not a raw
  `u8`. Every register write takes a typed config, not a bit pattern. Culvert is the
  translation layer between the wire format and the rest of the stack.
- **Interface owned by the consumer.** The `ScdcClient` trait that culvert implements is
  defined in `plumbob`, not here. culvert implements the trait; it does not define it.
  This means the link training layer can swap culvert for any other `ScdcClient`
  implementation without touching culvert. The `plumbob` cargo feature (temporarily
  removed, see above) gates the impl so culvert remains independently usable without the
  link training layer as a dependency.
- **Spec accuracy and completeness.** All SCDC-defined registers are implemented. No
  register is omitted because its consumer has not been built yet. What is needed for
  0.1.0 ships in 0.1.0; the rest is tracked on the roadmap.
- **Stateless client, stateful caller.** `Scdc<T>` holds no protocol state. Sequencing,
  retry logic, and training state live in the caller. This keeps culvert fully testable
  in isolation — any sequence of register reads and writes can be exercised without
  simulating a training run.
- **Deterministic and testable.** The simulated transport pattern from `hdmi-hal` applies
  here: pre-load a register array, run culvert operations against it, assert on results.
  No hardware required.
- **Transport errors and protocol errors are distinct.** A caller should be able to tell
  whether a failure came from the I²C bus or from unexpected register content.
- **Stack-ordered delivery.** The 0.1.0 scope is the register coverage needed by the
  link training crate. Everything else the spec defines is on the roadmap.
- **No unsafe code.** `#![forbid(unsafe_code)]`.
- **Attested releases.** Every release is published through a GitHub Actions workflow
  that signs the `.crate` package with [SLSA Build Level 2](https://slsa.dev) provenance.
  Verify with `gh attestation verify <file> --repo DracoWhitefire/culvert`.

