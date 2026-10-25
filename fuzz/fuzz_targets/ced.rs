#![no_main]

use culvert::{CedCount, CedWarning, HdmiLanes, Scdc};
use hdmi_hal::scdc::ScdcTransport;
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
        // A register array holds still while it is read: one copy is one transaction.
        for (offset, byte) in buf.iter_mut().enumerate() {
            *byte = self.0[(reg as usize + offset) % 256];
        }
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 9 {
        return;
    }
    // A tenth byte picks the lane count (odd: four lanes); without one, three.
    let lanes = match data.get(9) {
        Some(byte) if byte & 1 != 0 => HdmiLanes::Four,
        _ => HdmiLanes::Three,
    };
    let block: [u8; 9] = data[..9].try_into().unwrap();

    let mut regs = [0u8; 256];
    // ERR_DET registers 0x50–0x58: low/high byte pairs for lanes 0–2, the CED
    // checksum at 0x56, and lane 3 at 0x57–0x58.
    regs[0x50..=0x58].copy_from_slice(&block);

    let mut scdc = Scdc::new(FuzzTransport(regs));
    // The transport cannot fail, and a CED read has no other error.
    let reading = scdc.read_ced(lanes).unwrap();

    // Each lane decodes from its own two bytes: `None` without the validity bit, else the
    // 15-bit count. Lane 3 is a count only with four lanes.
    let lane = |i: usize| {
        let (low, high) = (
            block[2 * i + (i == 3) as usize],
            block[2 * i + 1 + (i == 3) as usize],
        );
        (high & 0x80 != 0).then(|| (u16::from(high & 0x7F) << 8) | u16::from(low))
    };
    let value = |count: Option<CedCount>| count.map(CedCount::value);
    let counters = &reading.counters;
    assert_eq!(value(counters.lane0), lane(0));
    assert_eq!(value(counters.lane1), lane(1));
    assert_eq!(value(counters.lane2), lane(2));
    let lane3 = match lanes {
        HdmiLanes::Three => None,
        HdmiLanes::Four => lane(3),
    };
    assert_eq!(value(counters.lane3), lane3);

    // A checksum warning exactly when the covered bytes do not sum to zero, and its
    // `expected` is the checksum byte that would make them.
    let covered = match lanes {
        HdmiLanes::Three => 7,
        HdmiLanes::Four => 9,
    };
    let sum = |bytes: &[u8]| bytes.iter().fold(0u8, |sum, &byte| sum.wrapping_add(byte));
    let mismatch = reading.iter_warnings().find_map(|warning| match warning {
        CedWarning::ChecksumMismatch {
            expected, found, ..
        } => Some((*expected, *found)),
        _ => None,
    });
    assert_eq!(mismatch.is_some(), sum(&block[..covered]) != 0);
    if let Some((expected, found)) = mismatch {
        assert_eq!(found, block[6]);
        let mut fixed = block;
        fixed[6] = expected;
        assert_eq!(sum(&fixed[..covered]), 0);
    }

    // Lane 3 reported exactly when it is not in use and its validity bit is set, with its
    // count.
    let unused = reading.iter_warnings().find_map(|warning| match warning {
        CedWarning::UnusedLaneReported { count, .. } => Some(count.value()),
        _ => None,
    });
    let expected_unused = match lanes {
        HdmiLanes::Three => lane(3),
        HdmiLanes::Four => None,
    };
    assert_eq!(unused, expected_unused);

    // Nothing else: the two kinds above, each at most once, checksum first.
    let kinds = usize::from(mismatch.is_some()) + usize::from(unused.is_some());
    assert_eq!(reading.warnings().len(), kinds);
    if kinds == 2 {
        assert!(matches!(
            reading.warnings()[0],
            CedWarning::ChecksumMismatch { .. }
        ));
    }
});
