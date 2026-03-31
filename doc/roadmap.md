# Roadmap

Registers defined by the HDMI 2.1 SCDC specification (§10.4) that are not wrapped
in culvert 0.1.0. All addresses are listed in `src/register/address.rs` for
completeness; methods for these groups will be added in later releases.

---

## RS Correction counters (0x58–0x5F)

FRL mode uses Reed-Solomon forward error correction on each lane. The `Rs_Correction`
registers mirror the `ERR_DET` layout: four low/high byte pairs, each holding a 15-bit
counter with a validity bit in the high byte's bit 7.

Future API surface:

```rust
pub struct RsCorrectionCounters {
    pub lane0: Option<RsCorrectionCount>,
    pub lane1: Option<RsCorrectionCount>,
    pub lane2: Option<RsCorrectionCount>,
    pub lane3: Option<RsCorrectionCount>,   // 4-lane FRL only
}

impl Scdc<T> {
    pub fn read_rs_correction(&mut self) -> Result<RsCorrectionCounters, ScdcError<T::Error>>;
}
```

The implementation would be a direct parallel of `read_ced()`.

---

## DSC status

`Update_1` bit 0 (`dsc_update`) notifies the source that DSC (Display Stream
Compression) status has changed. Culvert 0.1.0 surfaces this flag via `UpdateFlags`
but provides no method to read the corresponding DSC status registers.

The HDMI 2.1 spec defines DSC-related fields in `Status_Flags_1` and in additional
registers. These will be wrapped once the DSC path in the link training crate requires
them.

---

## Manufacturer identification (0xC0–0xDD)

HDMI 2.1 defines a range of SCDC registers for sink manufacturer OUI, device
identification, and manufacturer-specific data. These are not required for link
training and are deferred indefinitely. If they are ever needed, they would be
exposed through a separate `read_manufacturer_info()` method returning raw bytes
rather than typed fields, since the content is vendor-defined.
