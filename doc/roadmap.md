# Roadmap

Registers defined by the HDMI 2.1 SCDC specification (§10.4) that are not wrapped
by culvert yet, plus the `Update_0` bit 7 (`LIP_Update`) that HDMI 2.2 adds. Methods for
these groups may be added in later releases.

---

## CED checksum (0x56)

`ERR_DET_Checksum` (0x56) is a checksum over the CED registers. Its address is
`codec::ERR_DET_CHECKSUM`, but culvert does not read or verify it yet. Verifying it would
let `read_ced` detect a counter pair torn between two reads. `hdmi-hal`'s
`ScdcTransport::read_block` now allows reading the CED block in one transaction; using it,
and deciding whether to verify the checksum then, is planned as its own change.

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
