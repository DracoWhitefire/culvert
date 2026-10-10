#![no_main]

use culvert::Scdc;
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
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 3 {
        return;
    }

    let mut regs = [0u8; 256];
    regs[0x40] = data[0]; // Status_Flags_0
    regs[0x41] = data[1]; // Status_Flags_1: LTP requests, lanes 0–1
    regs[0x42] = data[2]; // Status_Flags_2: LTP requests, lanes 2–3

    let mut scdc = Scdc::new(FuzzTransport(regs));
    // Must not panic. Neither has a protocol error path: every byte decodes, undefined
    // LTP request values as LtpReq::Reserved.
    let _ = scdc.read_status_flags();
    let _ = scdc.read_ltp_requests();
});
