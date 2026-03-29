//! Integration tests for the `Scdc` client against a simulated transport.
//!
//! `SimulatedScdc` backs the transport with a `[u8; 256]` register array.
//! Tests pre-load register state, run client operations, and assert on both
//! returned values and register contents.

use core::convert::Infallible;
use culvert::{
    FfeLevels, FrlConfig, FrlRate, LtpReq, ProtocolError, Scdc, ScdcError, TmdsConfig, UpdateFlags,
};
use hdmi_hal::scdc::ScdcTransport;

// ── simulated transport ───────────────────────────────────────────────────────

struct SimulatedScdc {
    regs: [u8; 256],
}

impl SimulatedScdc {
    fn new() -> Self {
        Self { regs: [0u8; 256] }
    }

    fn set(&mut self, addr: u8, val: u8) {
        self.regs[addr as usize] = val;
    }

    fn get(&self, addr: u8) -> u8 {
        self.regs[addr as usize]
    }
}

impl ScdcTransport for SimulatedScdc {
    type Error = Infallible;

    fn read(&mut self, reg: u8) -> Result<u8, Infallible> {
        Ok(self.regs[reg as usize])
    }

    fn write(&mut self, reg: u8, value: u8) -> Result<(), Infallible> {
        self.regs[reg as usize] = value;
        Ok(())
    }
}

// ── version ───────────────────────────────────────────────────────────────────

#[test]
fn read_sink_version() {
    let mut transport = SimulatedScdc::new();
    transport.set(0x01, 0x01);
    let mut scdc = Scdc::new(transport);

    assert_eq!(scdc.read_sink_version().unwrap(), 0x01);
}

#[test]
fn write_source_version() {
    let mut scdc = Scdc::new(SimulatedScdc::new());
    scdc.write_source_version(0x01).unwrap();

    assert_eq!(scdc.into_transport().get(0x02), 0x01);
}

// ── scrambling ────────────────────────────────────────────────────────────────

#[test]
fn write_tmds_config_encodes_bits() {
    let mut scdc = Scdc::new(SimulatedScdc::new());
    scdc.write_tmds_config(TmdsConfig {
        scrambling_enable: true,
        high_tmds_clock_ratio: true,
    })
    .unwrap();

    // scrambling_enable = bit 0, high_tmds_clock_ratio = bit 1
    assert_eq!(scdc.into_transport().get(0x20), 0x03);
}

#[test]
fn read_scrambler_status_active() {
    let mut transport = SimulatedScdc::new();
    transport.set(0x21, 0x01);
    let mut scdc = Scdc::new(transport);

    assert!(scdc.read_scrambler_status().unwrap().scrambling_active);
}

#[test]
fn read_scrambler_status_inactive() {
    let mut scdc = Scdc::new(SimulatedScdc::new());
    assert!(!scdc.read_scrambler_status().unwrap().scrambling_active);
}

// ── FRL training ─────────────────────────────────────────────────────────────

#[test]
fn write_frl_config_encodes_bits() {
    let mut scdc = Scdc::new(SimulatedScdc::new());
    scdc.write_frl_config(FrlConfig {
        frl_rate: FrlRate::Rate6Gbps4Lanes, // discriminant 3 → bits[3:0]
        dsc_frl_max: true,                  // → bit[4]
        ffe_levels: FfeLevels::Ffe3,        // discriminant 3 → bits[7:5]
    })
    .unwrap();

    // 0x03 | 0x10 | (3 << 5) = 0x03 | 0x10 | 0x60 = 0x73
    assert_eq!(scdc.into_transport().get(0x30), 0x73);
}

#[test]
fn read_status_flags_decodes_registers() {
    let mut transport = SimulatedScdc::new();
    // clock_detected=1 ch0_locked=1 ch2_locked=1 flt_ready=1 → 0b0101_0101 = 0x55
    transport.set(0x40, 0x55);
    // frl_start=1, ltp_req=Lfsr1 (2) in bits[7:4] → 0b0010_0001 = 0x21
    transport.set(0x41, 0x21);
    let mut scdc = Scdc::new(transport);

    let flags = scdc.read_status_flags().unwrap();
    assert!(flags.clock_detected);
    assert!(!flags.cable_connected);
    assert!(flags.ch0_locked);
    assert!(!flags.ch1_locked);
    assert!(flags.ch2_locked);
    assert!(!flags.ch3_locked);
    assert!(flags.flt_ready);
    assert!(flags.frl_start);
    assert_eq!(flags.ltp_req, LtpReq::Lfsr1);
}

#[test]
fn read_status_flags_unknown_ltp_req() {
    let mut transport = SimulatedScdc::new();
    transport.set(0x41, 0x50); // LTP_Req nibble = 5, undefined
    let mut scdc = Scdc::new(transport);

    assert!(matches!(
        scdc.read_status_flags(),
        Err(ScdcError::Protocol(ProtocolError::UnknownLtpReq(5)))
    ));
}

#[test]
fn read_update_flags_decodes_registers() {
    let mut transport = SimulatedScdc::new();
    transport.set(0x10, 0x07); // status_update | ced_update | frl_update
    transport.set(0x11, 0x01); // dsc_update
    let mut scdc = Scdc::new(transport);

    let flags = scdc.read_update_flags().unwrap();
    assert!(flags.status_update);
    assert!(flags.ced_update);
    assert!(flags.frl_update);
    assert!(flags.dsc_update);
}

#[test]
fn clear_update_flags_writes_w1c() {
    let mut transport = SimulatedScdc::new();
    transport.set(0x10, 0x07);
    transport.set(0x11, 0x01);
    let mut scdc = Scdc::new(SimulatedScdc::new());

    // Clear only frl_update and dsc_update.
    scdc.clear_update_flags(UpdateFlags::new(false, false, true, true))
        .unwrap();

    let transport = scdc.into_transport();
    assert_eq!(transport.get(0x10), 0x04); // only frl_update bit
    assert_eq!(transport.get(0x11), 0x01); // dsc_update bit
}

// ── CED ───────────────────────────────────────────────────────────────────────

#[test]
fn read_ced_decodes_valid_and_invalid_lanes() {
    let mut transport = SimulatedScdc::new();
    // Lane 0: valid, count = 0x0134
    transport.set(0x50, 0x34); // l0
    transport.set(0x51, 0x81); // h0: valid bit set, upper bits = 0x01
    // Lane 1: validity bit not set → None
    transport.set(0x52, 0xFF);
    transport.set(0x53, 0x00);
    // Lane 2: valid, count = 0x7FFF (max)
    transport.set(0x54, 0xFF); // l2
    transport.set(0x55, 0xFF); // h2: valid + all counter bits set
    // Lane 3: validity bit not set → None
    transport.set(0x56, 0x00);
    transport.set(0x57, 0x00);
    let mut scdc = Scdc::new(transport);

    let ced = scdc.read_ced().unwrap();

    assert_eq!(ced.lane0.map(|c| c.value()), Some(0x0134));
    assert_eq!(ced.lane1, None);
    assert_eq!(ced.lane2.map(|c| c.value()), Some(0x7FFF));
    assert_eq!(ced.lane3, None);
}

#[test]
fn read_ced_all_invalid() {
    let mut scdc = Scdc::new(SimulatedScdc::new());
    let ced = scdc.read_ced().unwrap();
    assert!(ced.lane0.is_none());
    assert!(ced.lane1.is_none());
    assert!(ced.lane2.is_none());
    assert!(ced.lane3.is_none());
}
