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
- FRL training primitives: writing `Config_0` (FRL rate, FFE levels), reading
  `Status_Flags` (`FLT_Ready`, lane lock, `LTP_Req`), reading and clearing `Update_0`,
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

- `display-types` — for `HdmiForumFrl`, the FRL rate enum used in `Config_0`.
- `hdmi-hal` — for the `ScdcTransport` trait.

Culvert does not depend on `piaf` or `concordance`. It is consumed by the link training
crate, which sequences culvert's operations according to the FRL training algorithm.
---

## The SCDC Register Map

SCDC is defined in HDMI 2.1 spec section 10.4. Registers are one byte wide, addressed
by a one-byte offset over DDC/I²C to the sink's SCDC address (0x54).

The register map divides into four functional groups:

**Version** (0x01–0x02)
- `Sink_Version` (0x01, R) — SCDC protocol version supported by the sink.
- `Source_Version` (0x02, W) — SCDC protocol version the source intends to use.

**Update flags** (0x10–0x11)
- `Update_0` (0x10, R/W) — change notification flags: `FRL_Update`, `CED_Update`,
  `Status_Update`. The source reads and then clears these to detect sink-side state
  changes without polling every status register on every pass.
- `Update_1` (0x11, R/W) — `DSC_Update` (bit 0): DSC status has changed.

**TMDS and scrambling** (0x20–0x21)
- `TMDS_Config` (0x20, W) — `Scrambling_Enable` and `TMDS_Bit_Clock_Ratio`.
- `Scrambler_Status` (0x21, R) — sink acknowledgement that scrambling is active.

**FRL configuration and status** (0x30–0x41)
- `Config_0` (0x30, W) — `FRL_Rate` (4 bits, maps to `HdmiForumFrl`), `DSC_FRL_Max`,
  `FFE_Levels`. Written by the source to request a training rate.
- `Status_Flags_0` (0x40, R) — `Clock_Detected`, `Cable_Connected`, per-lane lock bits
  (`Ch0_Locked`–`Ch3_Locked`), `FLT_Ready` (sink ready to begin link training).
- `Status_Flags_1` (0x41, R) — `FRL_Start`, `LTP_Req` (link training pattern request
  from sink).

**Character Error Detection** (0x50–0x57)
- `ERR_DET_0_L/H` through `ERR_DET_3_L/H` — per-lane 15-bit error counters with a
  validity bit in the high byte. Lane 3 is only populated in FRL mode (4-lane).
  Counters are read as a pair (low + high byte) to form a single `u16` value.

All registers are implemented in full per the spec. Registers needed by the link
training layer are available in 0.1.0; the remainder are tracked on the roadmap.

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

    // FRL training primitives
    pub fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), ScdcError<T::Error>>;
    pub fn read_status_flags(&mut self) -> Result<StatusFlags, ScdcError<T::Error>>;
    pub fn read_update_flags(&mut self) -> Result<UpdateFlags, ScdcError<T::Error>>;
    pub fn clear_update_flags(&mut self, flags: UpdateFlags) -> Result<(), ScdcError<T::Error>>;

    // CED
    pub fn read_ced(&mut self) -> Result<CedCounters, ScdcError<T::Error>>;
}
```

`Scdc<T>` holds no state beyond the transport. Register reads and writes are direct and
stateless from the client's perspective; any sequencing state lives in the caller.

Methods map to register groups, not necessarily individual registers. Two methods span
multiple registers in a single logical operation:

- `read_status_flags()` reads both `Status_Flags_0` (0x40) and `Status_Flags_1` (0x41)
  and merges them into one `StatusFlags` struct. The two registers form one logical unit —
  splitting them across two calls would force the caller to reason about a combined value
  that the spec treats as atomic.
- `read_update_flags()` and `clear_update_flags()` both operate on `Update_0` (0x10) and
  `Update_1` (0x11), returning and writing the full `UpdateFlags` struct. A caller polling
  for any update should not need two calls to see all flags.
- `read_ced()` reads the four ERR_DET low/high byte pairs (0x50–0x57) in a single pass
  and returns one `CedCounters` struct.

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

/// FFE (Feed-Forward Equalization) level count written into Config_0 bits[5:3].
pub enum FfeLevels {
    Ffe0 = 0,
    Ffe1 = 1,
    Ffe2 = 2,
    Ffe3 = 3,
    Ffe4 = 4,
    Ffe5 = 5,
    Ffe6 = 6,
    Ffe7 = 7,
}

pub struct FrlConfig {
    pub frl_rate: HdmiForumFrl,   // from display-types
    pub dsc_frl_max: bool,
    pub ffe_levels: FfeLevels,
}

/// Link Training Pattern requested by the sink via Status_Flags_1 bits[7:4].
/// An undefined nibble value surfaces as `ProtocolError::UnknownLtpReq`.
pub enum LtpReq {
    None  = 0,   // no LTP requested
    Lfsr0 = 1,
    Lfsr1 = 2,
    Lfsr2 = 3,
    Lfsr3 = 4,
}

pub struct StatusFlags {
    pub clock_detected: bool,
    pub cable_connected: bool,
    pub ch0_locked: bool,
    pub ch1_locked: bool,
    pub ch2_locked: bool,
    pub ch3_locked: bool,   // FRL 4-lane only
    pub flt_ready: bool,
    pub frl_start: bool,    // sink signals FRL training may begin
    pub ltp_req: LtpReq,
}

pub struct UpdateFlags {
    pub frl_update: bool,
    pub ced_update: bool,
    pub status_update: bool,
    pub dsc_update: bool,   // Update_1 (0x11) bit 0
}

/// A 15-bit character error count decoded from an ERR_DET register pair.
/// The high byte's bit 7 is the validity flag; the counter occupies bits[14:0].
/// `CedCount::value()` returns the raw count as a `u16` (always <= 0x7FFF).
pub struct CedCount(u16);

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
into `Config_0`. Given a status register, decode it into `StatusFlags`. Culvert does not
know what to do with a `StatusFlags`; it only knows how to read one.

**Link training's responsibility:** the state machine. Receive a ranked list of FRL tiers
from concordance. For each tier: write `Config_0`, wait for `FLT_Ready`, handle
`LTP_Req`, declare success or fall back to the next tier. That sequencing logic, timeout
handling, and retry policy live in the link training crate — not here.

The rule: if it touches time, state across multiple register accesses, or fallback logic,
it belongs in link training. If it reads or writes registers and returns typed results, it
belongs in culvert.

---

## The `plumbob` Feature

culvert implements `plumbob::ScdcClient` for `Scdc<T>`, gated behind a `plumbob` cargo
feature. This follows the same convention as `serde` feature flags in the ecosystem: the
producing crate reaches toward the consuming crate's trait, rather than the consumer
depending on the producer.

```toml
# Cargo.toml of a crate using both
culvert  = { version = "0.1", features = ["plumbob"] }
plumbob  = "0.1"
```

The impl converts between culvert's internal types and plumbob's owned types:

- `culvert::StatusFlags` → `plumbob::TrainingStatus` (projecting `frl_start` and `ltp_req`)
- `culvert::FrlConfig` ← `plumbob::FrlConfig` (field-for-field, with `LtpReq` conversion)
- `culvert::CedCounters` → `plumbob::CedCounters` (same structure, different type paths)

culvert's richer `StatusFlags` (lane lock bits, cable detection, clock detection) is not
exposed through `ScdcClient` — plumbob defines only what the training state machine
needs. Callers that need the full register set use `Scdc<T>` directly.

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
  implementation without touching culvert. The `plumbob` cargo feature gates the impl
  so culvert remains independently usable without the link training layer as a dependency.
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

