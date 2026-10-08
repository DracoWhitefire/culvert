# Roadmap

Registers defined by the HDMI 2.1 SCDC specification (§10.4) that are not wrapped
by culvert yet. All addresses are listed in `src/register/address.rs` for
completeness; methods for these groups will be added in later releases.

---

## CED checksum and RS correction count (0x56, 0x59–0x5A)

`ERR_DET_Checksum` (0x56) is a checksum over the CED registers, and
`RS_Correction_L/H` (0x59/0x5A) hold the Reed-Solomon correction count used in FRL mode
(signalled by `RSED_Update` in `Update_0`). Both addresses are listed in
`src/register/address.rs`, but their exact formats are not confirmed by the sources the
register map is based on (see [`architecture.md`](architecture.md#sources)), so culvert
does not decode them yet.

Possible future API surface, once the format is confirmed:

```rust
impl Scdc<T> {
    pub fn read_rs_correction(&mut self) -> Result<Option<RsCorrectionCount>, ScdcError<T::Error>>;
}
```

---

## DSC status

`Status_Flags_0` bit 7 (`DSC_Decode_Fail`) is read by `read_status_flags`, and the
sink-written `DSC_FRL_Max` test flag by `read_source_test_config`. Any further DSC-related
registers will be wrapped once the DSC path in the link training crate requires them.

---

## Manufacturer identification (0xD0–0xDD)

HDMI 2.1 defines a range of SCDC registers for sink manufacturer OUI (0xD0–0xD2), device
identification (0xD3–0xDA), hardware and software revisions (0xDB–0xDD), and
manufacturer-specific data (0xDE onwards). These are not required for link
training and are deferred indefinitely. If they are ever needed, they would be
exposed through a separate `read_manufacturer_info()` method returning raw bytes
rather than typed fields, since the content is vendor-defined.
