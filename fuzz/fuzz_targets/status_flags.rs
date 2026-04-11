#![no_main]

use culvert::Scdc;
use hdmi_hal::scdc::ScdcTransport;
use libfuzzer_sys::fuzz_target;

struct FuzzTransport([u8; 256]);

impl ScdcTransport for FuzzTransport {
    type Error = core::convert::Infallible;

    fn read(&mut self, reg: u8) -> Result<u8, Self::Error> {
        Ok(self.0[reg as usize])
    }

    fn write(&mut self, reg: u8, value: u8) -> Result<(), Self::Error> {
        self.0[reg as usize] = value;
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }

    let mut regs = [0u8; 256];
    regs[0x40] = data[0]; // Status_Flags_0
    regs[0x41] = data[1]; // Status_Flags_1

    let mut scdc = Scdc::new(FuzzTransport(regs));
    // Must not panic; may return Ok(StatusFlags) or Err(ScdcError::Protocol(UnknownLtpReq(_))).
    let _ = scdc.read_status_flags();
});
