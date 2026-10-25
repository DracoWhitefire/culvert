use hdmi_hal::scdc::ScdcTransport;

use crate::codec;
use crate::error::ScdcError;
use crate::register::{CedReading, HdmiLanes, RsCorrectionCount};

use super::Scdc;

impl<T: ScdcTransport> Scdc<T> {
    /// Reads the per-lane character error counts: the `ERR_DET` block, 0x50–0x58 (lanes
    /// 0–2, the checksum at 0x56, lane 3), in one [`read_block`](ScdcTransport::read_block),
    /// and checks the checksum.
    ///
    /// `lanes` is the link's: [`HdmiLanes::Three`] for TMDS and 3-lane FRL, `Four` for 4-lane
    /// FRL (`rate.lanes()`). It decides what the checksum covers and whether lane 3 is read
    /// as a count.
    ///
    /// `Err` only for a transport failure. What the read finds wrong is a
    /// [`CedWarning`](crate::CedWarning) in the [`CedReading`], with the counters decoded
    /// either way: a checksum mismatch (most likely a read torn by a counter update), or a
    /// count for lane 3 when it is not in use. culvert does not read again; whether to is
    /// the caller's decision, and a sink that clears the counters on read returns different
    /// counts the second time.
    ///
    /// One transaction keeps the transport from splitting the block; whether the sink holds
    /// its registers still during it is up to the sink, and a transport that reads a
    /// register at a time ([`read_block_bytewise`]), or a bus driver that does, splits it
    /// anyway. The checksum is the one protection against a torn read that holds on every
    /// transport and adapter: it catches most torn reads, not all. [`CedReading`] says how
    /// the counts behave (saturation, clear on read, resets).
    ///
    /// `UpdateFlags::ced_update` is set by the sink and kept until the source clears it,
    /// but it is a hint at most: it has been seen set with every count zero, and a sink may
    /// never set it. Read the counters regularly rather than only after the flag, and clear
    /// the flag before reading them
    /// ([`clear_update_flags`](Self::clear_update_flags) says why).
    ///
    /// [`read_block_bytewise`]: hdmi_hal::scdc::read_block_bytewise
    pub fn read_ced(&mut self, lanes: HdmiLanes) -> Result<CedReading, ScdcError<T::Error>> {
        let mut block = [0; codec::CED_BLOCK_LEN];
        self.transport
            .read_block(codec::CED_BLOCK_START, &mut block)
            .map_err(ScdcError::Transport)?;
        Ok(codec::decode_ced_block(&block, lanes))
    }

    /// Reads the Reed-Solomon correction count from `RS_Correction_L/H` (0x59/0x5A).
    ///
    /// Returns `None` if the high byte's validity bit (bit 7) is not set: unknown, not
    /// zero. The count is maintained by the sink in FRL mode.
    ///
    /// Both bytes are one [`read_block`](ScdcTransport::read_block): one transaction, so
    /// the transport does not split them; whether the sink holds them still during it is
    /// up to the sink, and a transport that reads a register at a time
    /// ([`read_block_bytewise`]), or a bus driver that does, splits them anyway. The count
    /// has no checksum, so a torn read cannot be detected: unlike the CED counters, nothing
    /// protects it on every adapter.
    ///
    /// The count accumulates and saturates at 0x7FFF, so a saturated count means "at
    /// least". A sink may clear it when it is read, so a reading is a running total on
    /// some sinks and the count since the previous read on others; a reading lower than
    /// the previous one means a reset (clear on read, hot plug, an SCDC clear), and
    /// counting starts again from it. Count afresh after retraining, a rate change or a
    /// hot plug too.
    ///
    /// `UpdateFlags::rsed_update` is set by the sink and kept until the source clears it,
    /// a hint at most, as `ced_update` is; a sink may never set it. Read the count regularly
    /// rather than only after the flag.
    ///
    /// [`read_block_bytewise`]: hdmi_hal::scdc::read_block_bytewise
    pub fn read_rs_correction(&mut self) -> Result<Option<RsCorrectionCount>, ScdcError<T::Error>> {
        let mut bytes = [0; 2];
        self.transport
            .read_block(codec::RS_CORRECTION_L, &mut bytes)
            .map_err(ScdcError::Transport)?;
        let [low, high] = bytes;
        Ok(codec::decode_rs_correction(low, high))
    }
}

#[cfg(test)]
mod tests {
    use super::super::Scdc;
    use super::super::test_transport::{Op, TestTransport};
    use crate::error::ScdcError;
    use crate::register::{CedCount, CedCounters, CedWarning, FrlRate, HdmiLanes};

    /// A transport whose CED block holds `block`, with the checksum at 0x56 made valid for
    /// `lanes`.
    fn with_block(mut block: [u8; 9], lanes: HdmiLanes) -> TestTransport {
        let covered = match lanes {
            HdmiLanes::Three => 7,
            HdmiLanes::Four => 9,
        };
        block[6] = 0;
        let sum = block[..covered]
            .iter()
            .fold(0u8, |sum, &b| sum.wrapping_add(b));
        block[6] = sum.wrapping_neg();
        let mut sim = TestTransport::new();
        for (reg, byte) in (0x50..).zip(block) {
            sim.set(reg, byte);
        }
        sim
    }

    #[test]
    fn read_ced_is_one_block_read_of_0x50_to_0x58() {
        for lanes in [HdmiLanes::Three, HdmiLanes::Four] {
            let mut scdc = Scdc::new(TestTransport::new());
            scdc.read_ced(lanes).unwrap();
            assert_eq!(
                scdc.into_transport().log()[..2],
                [Some(Op::ReadBlock(0x50, 9)), None]
            );
        }
    }

    #[test]
    fn ced_validity_bit_required() {
        // High byte with bit 7 clear → None regardless of counter value.
        let sim = with_block([0xFF, 0x7F, 0, 0, 0, 0, 0, 0, 0], HdmiLanes::Three);
        let reading = Scdc::new(sim).read_ced(HdmiLanes::Three).unwrap();
        assert_eq!(reading.counters.lane0, None);
        assert_eq!(reading.warnings(), []);
    }

    #[test]
    fn ced_counter_validity_bit_stripped() {
        // High byte: bit 7 set (valid), counter bits = 0x01; low byte = 0x23.
        let sim = with_block([0x23, 0x81, 0, 0, 0, 0, 0, 0, 0], HdmiLanes::Three);
        let reading = Scdc::new(sim).read_ced(HdmiLanes::Three).unwrap();
        assert_eq!(reading.counters.lane0, Some(CedCount::new(0x0123)));
    }

    #[test]
    fn ced_each_lane_from_its_own_registers() {
        // A distinct, asymmetric low/high pair per lane, so a swapped byte or lane shows.
        let block = [0x02, 0x83, 0x04, 0x85, 0x06, 0x87, 0, 0x08, 0x89];
        let reading = Scdc::new(with_block(block, HdmiLanes::Four))
            .read_ced(HdmiLanes::Four)
            .unwrap();
        assert_eq!(
            reading.counters,
            CedCounters::new(
                Some(CedCount::new(0x0302)),
                Some(CedCount::new(0x0504)),
                Some(CedCount::new(0x0706)),
                Some(CedCount::new(0x0908)),
            )
        );
        assert_eq!(reading.warnings(), []);
    }

    #[test]
    fn ced_lane3_is_not_a_count_in_3_lane_mode() {
        // The checksum at 0x56 is not lane 3; lane 3 is reported, but not in use.
        let block = [0, 0, 0, 0, 0, 0, 0, 0x01, 0x80];
        let reading = Scdc::new(with_block(block, HdmiLanes::Three))
            .read_ced(HdmiLanes::Three)
            .unwrap();
        assert_eq!(reading.counters.lane3, None);
        assert_eq!(
            reading.warnings(),
            [CedWarning::UnusedLaneReported {
                count: CedCount::new(1)
            }]
        );
    }

    #[test]
    fn ced_checksum_mismatch_is_a_warning_with_the_counts() {
        let mut sim = with_block([0x34, 0x81, 0, 0, 0, 0, 0, 0, 0], HdmiLanes::Three);
        let checksum = sim.get(0x56);
        sim.set(0x56, checksum ^ 0x01);
        let reading = Scdc::new(sim).read_ced(HdmiLanes::Three).unwrap();
        assert_eq!(reading.counters.lane0, Some(CedCount::new(0x0134)));
        assert_eq!(
            reading.warnings(),
            [CedWarning::ChecksumMismatch {
                expected: checksum,
                found: checksum ^ 0x01,
            }]
        );
    }

    #[test]
    fn ced_lanes_from_the_link_rate() {
        let block = [0, 0, 0, 0, 0, 0, 0, 0x08, 0x89];
        let reading = Scdc::new(with_block(block, HdmiLanes::Four))
            .read_ced(FrlRate::Rate12Gbps4Lanes.lanes())
            .unwrap();
        assert_eq!(reading.counters.lane3, Some(CedCount::new(0x0908)));
        assert_eq!(reading.warnings(), []);
    }

    #[test]
    fn read_rs_correction_reads_low_and_high_in_one_block() {
        let mut scdc = Scdc::new(TestTransport::new());
        scdc.read_rs_correction().unwrap();
        assert_eq!(
            scdc.into_transport().log()[..2],
            [Some(Op::ReadBlock(0x59, 2)), None]
        );
    }

    #[test]
    fn ced_transport_error() {
        // The block is one read.
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .read_ced(HdmiLanes::Three)
                .is_err_and(|error| matches!(error, ScdcError::Transport(())))
        );
        assert!(
            Scdc::new(TestTransport::failing_after(1))
                .read_ced(HdmiLanes::Three)
                .is_ok()
        );
    }

    #[test]
    fn rs_correction_valid_and_invalid() {
        let mut sim = TestTransport::new();
        sim.set(0x59, 0x34);
        sim.set(0x5A, 0x81); // valid, upper counter bits = 0x01
        assert_eq!(
            Scdc::new(sim)
                .read_rs_correction()
                .unwrap()
                .map(|c| c.value()),
            Some(0x0134)
        );

        let mut sim = TestTransport::new();
        sim.set(0x59, 0xFF);
        sim.set(0x5A, 0x7F); // validity bit clear
        assert_eq!(Scdc::new(sim).read_rs_correction().unwrap(), None);
    }

    #[test]
    fn rs_correction_transport_error() {
        // The count is one block read.
        assert!(
            Scdc::new(TestTransport::failing_after(0))
                .read_rs_correction()
                .is_err_and(|error| matches!(error, ScdcError::Transport(())))
        );
        assert!(
            Scdc::new(TestTransport::failing_after(1))
                .read_rs_correction()
                .is_ok()
        );
    }
}
