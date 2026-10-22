#![no_main]

use culvert::{codec, CedLanes, CedWarning, Scdc};
use hdmi_hal::scdc::{read_block_bytewise, ScdcTransport};
use libfuzzer_sys::fuzz_target;

struct FuzzTransport([u8; 256]);

impl ScdcTransport for FuzzTransport {
    type Error = core::convert::Infallible;

    fn read(&self, reg: u8) -> Result<u8, Self::Error> {
        Ok(self.0[reg as usize])
    }

    fn write(&mut self, reg: u8, value: u8) -> Result<(), Self::Error> {
        self.0[reg as usize] = value;
        Ok(())
    }

    fn read_block(&self, reg: u8, buf: &mut [u8]) -> Result<(), Self::Error> {
        // A register array has no bus transaction to read in one go: one `read` per byte.
        read_block_bytewise(self, reg, buf)
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 9 {
        return;
    }
    // A tenth byte picks the lane count (odd: four lanes); without one, three.
    let lanes = match data.get(9) {
        Some(byte) if byte & 1 != 0 => CedLanes::Four,
        _ => CedLanes::Three,
    };
    let block: [u8; 9] = data[..9].try_into().unwrap();

    let mut regs = [0u8; 256];
    // ERR_DET registers 0x50–0x58: low/high byte pairs for lanes 0–2, the CED
    // checksum at 0x56, and lane 3 at 0x57–0x58.
    regs[0x50..=0x58].copy_from_slice(&block);

    let mut scdc = Scdc::new(FuzzTransport(regs));
    // The transport cannot fail, and a CED read has no other error.
    let reading = scdc.read_ced(lanes).unwrap();

    // The client decodes what the codec does.
    assert_eq!(reading, codec::decode_ced_block(&block, lanes));
    // A checksum warning exactly when the covered bytes do not sum to zero.
    let covered = match lanes {
        CedLanes::Three => &block[..7],
        CedLanes::Four => &block[..],
    };
    let sum = covered
        .iter()
        .fold(0u8, |sum, &byte| sum.wrapping_add(byte));
    let mismatch = reading
        .iter_warnings()
        .any(|warning| matches!(warning, CedWarning::ChecksumMismatch { .. }));
    assert_eq!(mismatch, sum != 0);
    // Lane 3 is a count only with four lanes; at most one warning of each kind.
    if lanes == CedLanes::Three {
        assert!(reading.counters.lane3.is_none());
    }
    assert!(reading.warnings().len() <= 2);
});
