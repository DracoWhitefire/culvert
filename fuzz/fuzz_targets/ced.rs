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
    if data.len() < 8 {
        return;
    }

    let mut regs = [0u8; 256];
    // ERR_DET registers 0x50–0x57: low/high byte pairs for lanes 0–3.
    regs[0x50..=0x57].copy_from_slice(&data[..8]);

    let mut scdc = Scdc::new(FuzzTransport(regs));
    // Must not panic; always returns Ok(CedCounters) — no error path in read_ced.
    let _ = scdc.read_ced();
});
