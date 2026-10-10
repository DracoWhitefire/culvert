# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking changes

- **`ScdcTransport::read` now takes `&self` instead of `&mut self`** (inherited from
  `hdmi-hal`). Any type you pass to `Scdc<T>` that implements `ScdcTransport` must
  change its `read` method receiver from `&mut self` to `&self`. If the implementation
  mutates state during reads (e.g. an operation counter), wrap those fields in `Cell`
  or `Mutex`.
- **SCDC register map corrected** (see *Fixed*); the typed API follows the corrected map:
  - `write_frl_config` writes `Config_1` (0x31). `FrlConfig::dsc_frl_max` is removed
    (`DSC_FRL_Max` is a sink-written flag, now read with `read_source_test_config`).
    `FfeLevels` is the highest TxFFE level index (0–7, `FfeLevels::new`,
    `FfeLevels::value`) instead of the `Ffe0`–`Ffe7` enum, and `write_frl_config`
    returns `ProtocolError::FfeLevelsOutOfRange` when it exceeds the maximum for the
    rate (3 up to 12 Gbps, 7 above).
  - `StatusFlags` decodes `Status_Flags_0` only: `ch0_locked`–`ch2_locked` and
    `ln3_locked` (bits 1–4) and `dsc_decode_fail` (bit 7). `cable_connected`,
    `ch3_locked`, `frl_start` and `ltp_req` are removed; `StatusFlags::new` takes the
    seven flags in bit order.
  - Link training pattern requests are read per lane with `read_ltp_requests`.
    `LtpReq` follows the spec values: LFSR 0–3 are 0x5–0x8, and `AllOnes`, `AllZeros`,
    `NyquistClock`, `RxDdeCompliance`, `FfeChange` and `RateChange` are added. Every
    value decodes: the undefined ones (0x9–0xD) are `LtpReq::Reserved(value)`, so a
    stray value on one lane (such as the unused lane 3 at a 3-lane rate) no longer fails
    the whole read; whether it matters is for the link training layer to decide.
    `ProtocolError::UnknownLtpReq` is removed, and `codec::decode_ltp_requests` returns
    `LtpRequests` rather than a `Result`. `LtpReq::value()` returns the 4-bit value.
  - `UpdateFlags` decodes all `Update_0` flags (`rr_test`, `source_test_update`,
    `frl_start`, `flt_update`, `rsed_update`); `frl_update` and `dsc_update` are
    removed, `Update_1` is no longer accessed, and `UpdateFlags::new` takes the seven
    flags in bit order. `clear_update_flags` refuses to clear `rr_test`, returning the
    new `ProtocolError::RrTestNotClearable` and writing nothing, instead of silently
    leaving it out of the write: culvert treats Read Request Test as a flag the source
    must not clear. The rule comes from the Intel `xe` series alone, and Amlogic's driver
    clears the bit, so it is provisional; `doc/architecture.md` records the evidence, the
    reasoning and what would change it.
- **The `plumbob` feature implements plumbob's per-lane `ScdcClient`.** plumbob 0.1's
  interface (`read_training_status`, a single link training pattern request) could not
  work with the corrected register map. `Scdc<T>` now implements the one-method-per-register
  operation trait of plumbob's link training states (LTS:2 → LTS:3 → LTS:P):
  `read_flt_ready` (`Status_Flags_0` bit 6), `read_update_flags` and `clear_update_flags`
  (the `Update_0` training flags), `read_ltp_requests` (per lane),
  `read_source_test_config` (`FLT_no_timeout`), `write_config_0_defaults`,
  `write_frl_config` (`Config_1`) and `read_ced`. culvert's types convert to plumbob's
  through `From` impls in the feature-gated module; culvert's own types are unchanged.
  `Scdc` does not wait between polls: the poll interval plumbob's limits assume (2 ms by
  default) belongs in the transport.

### Changed

- **Minimum `hdmi-hal` version raised to 0.5**: `Scdc<T>` is bound by hdmi-hal 0.5's
  `ScdcTransport`, the version plumbob's training uses. It has the `read(&self)` receiver
  from hdmi-hal 0.4.0 (see above) and adds the `read_block` default method, so a
  transport written for hdmi-hal 0.4 only needs its dependency raised.
- **`display-types` updated to 0.4** — aligns with display-types 0.4 across the stack.
  `FrlRate` (the re-exported `HdmiForumFrl`) is now display-types 0.4's type, the one
  `hdmi-hal` and `plumbob` use.

### Fixed

- **Wrong SCDC register map** — the map culvert implemented did not match the HDMI 2.1
  register layout. FRL rate and FFE levels were written to `Config_0` (0x30), whose bit 0
  enables sink read requests; lane-lock bits were shifted by one; `FRL_Start` and the
  link training pattern requests were read from the wrong registers, for one lane only,
  with the wrong pattern values; and lane 3's CED counter was read from 0x56/0x57, where
  0x56 is the CED checksum. The map was rebuilt from open-source HDMI 2.1
  implementations (Xilinx, AMD, Amlogic, Realtek and two Linux DRM patch series), with
  every field confirmed by at least two of them; see `doc/architecture.md`.

### Added

- **`LtpReq::from_value`** — the request for a raw 4-bit value (`None` above 0xF); every
  value has exactly one request, `Reserved` only for 0x9–0xD. The decoder uses it.
- `ScdcTransport` re-exported at the crate root, from `hdmi-hal`: the bound on `Scdc<T>`.
- **`culvert::codec`** — the register map without I/O: every register address culvert
  accesses (and the CED checksum), `CED_REGISTERS`, and one encode or decode function per
  register (`encode_tmds_config`, `decode_scrambler_status`, `decode_update_flags`,
  `encode_clear_update_flags`, `encode_config_0`, `encode_config_1`,
  `decode_source_test_config`, `decode_status_flags`, `decode_ltp_requests`, `decode_ced`,
  `decode_rs_correction`). `Scdc<T>` now only performs the reads and writes and uses these;
  `culvert-async` uses the same functions, so the sync and async clients share one
  register map.
- `write_config_0` and `Config0` (`RR_Enable`, `FLT_No_Retrain`).
- `read_source_test_config` and `SourceTestConfig` (`TxFFE_Pre_Shoot_Only`,
  `TxFFE_De_Emphasis_Only`, `TxFFE_No_FFE`, `FLT_No_Timeout`, `DSC_FRL_Max`, `FRL_Max`).
- `read_ltp_requests`, `LtpRequests` and `LtpRequests::all_trained`.
- `read_rs_correction` and `RsCorrectionCount` (Reed-Solomon correction count, 0x59/0x5A).
- `FfeLevels::max_for` and `ProtocolError::FfeLevelsOutOfRange`.

- **SLSA Build Level 2 provenance** — release artifacts are attested via
  `actions/attest-build-provenance` and verified with
  `gh attestation verify <file> --repo DracoWhitefire/culvert`.
- **Fuzz targets** — `cargo-fuzz` harnesses for `read_status_flags`,
  `read_ltp_requests` and `read_ced`; 60 s smoke runs on every PR/push, 1 h deep runs on a weekly
  schedule, with automatic corpus minimisation and corpus-update PRs.

### Internal

- **CI builds for a `no_std` target** — new `Build (no_std)` steps build the crate for
  `thumbv7em-none-eabi`, with and without the `plumbob` feature. Nothing checked a
  target without `std` before, so a dependency that enables `std` (as hdmi-hal did
  through `display-types`) went unnoticed.
  The publish workflow runs the same build steps.
- **Automated publish can be triggered by `release-tag`** — `publish.yml` gains a
  `workflow_dispatch` trigger. Tags pushed with `GITHUB_TOKEN` do not start push-triggered
  workflows, so `release-tag`'s "Trigger publish workflow" step
  (`gh workflow run publish.yml`) could not start a publish run. Dispatches against a
  non-tag ref (e.g. `main`) are skipped, so they cannot publish or create a release.

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
