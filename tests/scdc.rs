//! Integration tests for the `Scdc` client against a simulated transport.
//!
//! `SimulatedScdc` backs the transport with a `[u8; 256]` register array.
//! Tests pre-load register state, run client operations, and assert on both
//! returned values and register contents.

use core::convert::Infallible;
use culvert::{
    ClearableUpdateFlags, Config0, FfeLevels, FrlConfig, FrlRate, LtpReq, Scdc, TmdsConfig,
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

    fn read(&self, reg: u8) -> Result<u8, Infallible> {
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
        frl_rate: FrlRate::Rate10Gbps4Lanes, // discriminant 5 → bits[3:0]
        ffe_levels: FfeLevels::new(3).unwrap(), // → bits[7:4]
    })
    .unwrap();
    let transport = scdc.into_transport();
    assert_eq!(transport.get(0x31), 0x35);
    assert_eq!(transport.get(0x30), 0x00); // Config_0 is a different register
}

#[test]
fn write_config_0_and_read_source_test_config() {
    let mut transport = SimulatedScdc::new();
    transport.set(0x35, 0x60); // FLT_No_Timeout (bit 5) | DSC_FRL_Max (bit 6)
    let mut scdc = Scdc::new(transport);

    scdc.write_config_0(Config0 {
        rr_enable: false,
        flt_no_retrain: true,
    })
    .unwrap();
    let test_config = scdc.read_source_test_config().unwrap();
    assert!(test_config.flt_no_timeout);
    assert!(test_config.dsc_frl_max);
    assert_eq!(scdc.into_transport().get(0x30), 0x02);
}

#[test]
fn read_status_flags_decodes_registers() {
    let mut transport = SimulatedScdc::new();
    // clock_detected (bit 0) | ch0_locked (bit 1) | ch2_locked (bit 3) | flt_ready (bit 6)
    // → 0b0100_1011 = 0x4B
    transport.set(0x40, 0x4B);
    let mut scdc = Scdc::new(transport);

    let flags = scdc.read_status_flags().unwrap();
    assert!(flags.clock_detected);
    assert!(flags.ch0_locked);
    assert!(!flags.ch1_locked);
    assert!(flags.ch2_locked);
    assert!(!flags.ln3_locked);
    assert!(flags.flt_ready);
    assert!(!flags.dsc_decode_fail);
}

#[test]
fn read_ltp_requests_decodes_lanes() {
    let mut transport = SimulatedScdc::new();
    transport.set(0x41, 0x76); // lane0 = LFSR1 (0x6), lane1 = LFSR2 (0x7)
    transport.set(0x42, 0xE0); // lane2 = none, lane3 = FFE change (0xE)
    let mut scdc = Scdc::new(transport);

    let requests = scdc.read_ltp_requests().unwrap();
    assert_eq!(requests.lane0, LtpReq::Lfsr1);
    assert_eq!(requests.lane1, LtpReq::Lfsr2);
    assert_eq!(requests.lane2, LtpReq::None);
    assert_eq!(requests.lane3, LtpReq::FfeChange);
}

#[test]
fn read_ltp_requests_undefined_value() {
    let mut transport = SimulatedScdc::new();
    transport.set(0x42, 0xA0); // lane 3 = 0xA, undefined
    let mut scdc = Scdc::new(transport);

    assert_eq!(
        scdc.read_ltp_requests().unwrap().lane3,
        LtpReq::Reserved(0xA)
    );
}

#[test]
fn read_update_flags_decodes_registers() {
    let mut transport = SimulatedScdc::new();
    transport.set(0x10, 0x33); // status_update | ced_update | frl_start | flt_update
    let mut scdc = Scdc::new(transport);

    let flags = scdc.read_update_flags().unwrap();
    assert!(flags.status_update);
    assert!(flags.ced_update);
    assert!(!flags.rr_test);
    assert!(!flags.source_test_update);
    assert!(flags.frl_start);
    assert!(flags.flt_update);
    assert!(!flags.rsed_update);
}

#[test]
fn clear_update_flags_writes_w1c() {
    let mut scdc = Scdc::new(SimulatedScdc::new());

    // Clear only flt_update and frl_start.
    scdc.clear_update_flags(ClearableUpdateFlags::new(
        false, false, false, true, true, false,
    ))
    .unwrap();

    let transport = scdc.into_transport();
    assert_eq!(transport.get(0x10), 0x30); // frl_start (bit 4) | flt_update (bit 5)
    assert_eq!(transport.get(0x11), 0x00); // Update_1 untouched
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
    // CED checksum sits between lane 2 and lane 3.
    transport.set(0x56, 0x80);
    // Lane 3: validity bit not set → None
    transport.set(0x57, 0x00);
    transport.set(0x58, 0x00);
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
