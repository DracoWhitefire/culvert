# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2026-04-05

### Changed

- **Minimum `hdmi-hal` version raised to 0.3.0**: culvert now requires
  `hdmi-hal >= 0.3.0`. Users who also depend on `plumbob` or `hdmi-hal-async`
  (both of which require 0.3.0) will no longer see a duplicate `hdmi-hal` copy
  in their dependency graph, and `ScdcTransport` is once again a single coherent
  type across the stack.

## [0.1.1] - 2026-04-04

### Added

- `ScramblerStatus::new(scrambling_active: bool)` — constructor required by `culvert-async`,
  which decodes the register and constructs this type outside the crate.
- `StatusFlags::new(...)` — constructor required by `culvert-async` for the same reason.
- `CedCounters::new(lane0, lane1, lane2, lane3)` — constructor required by `culvert-async`.
- `CedCount::new` is now `pub` (was `pub(crate)`); `culvert-async` needs to construct
  `CedCount` values when decoding `ERR_DET` registers.

## [0.1.0] - 2026-04-03

### Added

**Core SCDC client**

- `Scdc<T>` — stateless typed client that wraps an [`hdmi_hal::scdc::ScdcTransport`] and exposes
  one method per register group. Holds no protocol state; all sequencing and retry logic belongs
  in the caller.
- `Scdc::new(transport)` and `Scdc::into_transport()` for construction and unwrapping.

**Version registers**

- `read_sink_version()` — reads `Sink_Version` (0x01).
- `write_source_version(u8)` — writes `Source_Version` (0x02).

**TMDS scrambling**

- `write_tmds_config(TmdsConfig)` — writes `TMDS_Config` (0x20): `Scrambling_Enable` (bit 0) and
  `TMDS_Bit_Clock_Ratio` (bit 1).
- `read_scrambler_status()` — reads `Scrambler_Status` (0x21); returns `ScramblerStatus` with the
  sink's `scrambling_active` confirmation flag.

**FRL link training**

- `write_frl_config(FrlConfig)` — writes `Config_0` (0x30): `FRL_Rate` (bits 3:0), `DSC_FRL_Max`
  (bit 4), and `FFE_Levels` (bits 7:5).
- `read_status_flags()` — reads `Status_Flags_0` (0x40) and `Status_Flags_1` (0x41); returns
  `StatusFlags` covering clock detection, cable presence, per-lane symbol lock (lanes 0–3),
  `FLT_Ready`, `FRL_Start`, and the current `LtpReq`.

**Update flags**

- `read_update_flags()` — reads `Update_0` (0x10) and `Update_1` (0x11); returns `UpdateFlags`
  with `status_update`, `ced_update`, `frl_update`, and `dsc_update`.
- `clear_update_flags(UpdateFlags)` — write-1-to-clear: each flag set to `true` is cleared in the
  corresponding register; flags set to `false` are left unchanged.

**Character Error Detection**

- `read_ced()` — reads `ERR_DET` registers (0x50–0x57); returns `CedCounters` with per-lane
  `Option<CedCount>` values. A lane's counter is `None` when the high-byte validity bit is not
  set. Lane 3 is always `None` in TMDS or 3-lane FRL mode.

**Register types**

- `TmdsConfig` — `scrambling_enable: bool`, `high_tmds_clock_ratio: bool`.
- `ScramblerStatus` — `scrambling_active: bool`.
- `FrlConfig` — `frl_rate: FrlRate`, `dsc_frl_max: bool`, `ffe_levels: FfeLevels`.
- `FrlRate` — re-export of [`display_types::HdmiForumFrl`]; use `HdmiForumFrl::NotSupported` to
  clear FRL mode.
- `FfeLevels` — exhaustive enum `Ffe0`–`Ffe7` covering all 3-bit FFE level values.
- `LtpReq` — `None`, `Lfsr0`–`Lfsr3`; marked `#[non_exhaustive]` for forward compatibility.
- `StatusFlags` — decoded view of `Status_Flags_0/1`; marked `#[non_exhaustive]`.
- `UpdateFlags` — decoded view of `Update_0/1`; constructed via `UpdateFlags::new`; marked
  `#[non_exhaustive]`.
- `CedCount` — 15-bit newtype (`value() -> u16`); validity bit is consumed during decode and
  never exposed.
- `CedCounters` — `lane0`–`lane3: Option<CedCount>`; marked `#[non_exhaustive]`.

**Error types**

- `ScdcError<E>` — two-variant enum distinguishing `Transport(E)` (I²C/DDC bus error) from
  `Protocol(ProtocolError)` (spec-violating register content); marked `#[non_exhaustive]`.
- `ProtocolError` — `UnknownFrlRate(u8)` and `UnknownLtpReq(u8)`; the inner `u8` is the raw
  register field for diagnostics only — any out-of-spec value is unconditionally a violation.
  Marked `#[non_exhaustive]`.

**`plumbob` feature**

- Implements [`plumbob::ScdcClient`] for `Scdc<T>`, bridging `write_frl_config`,
  `read_training_status`, and `read_ced` to the `plumbob` trait vocabulary. Enabled by
  `features = ["plumbob"]`.

**Safety and portability**

- `#![no_std]` — the crate is fully `no_std` compatible with no `alloc` requirement.
- `#![forbid(unsafe_code)]` — no unsafe code anywhere in the crate.
