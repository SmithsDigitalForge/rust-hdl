use std::time::Duration;

use rust_hdl_core::prelude::*;
use rust_hdl_ok_core::prelude::*;
use rust_hdl_widgets::prelude::*;

use crate::pins::*;

pub mod ddr_fifo;
pub mod mcb_if;
pub mod mig;
pub mod ok_download_ddr;
pub mod pins;
pub mod synth;
pub mod serdes;
pub mod pll;
pub mod clock_buffer;

#[derive(Clone, Debug)]
pub struct XEM6010 {}

impl OpalKellyBSP for XEM6010 {
    fn hi() -> OpalKellyHostInterface {
        OpalKellyHostInterface::xem_6010()
    }
    fn ok_host() -> OpalKellyHost {
        OpalKellyHost::xem_6010()
    }

    fn leds() -> Signal<Out, Bits<8>> {
        xem_6010_leds()
    }
    fn clocks() -> Vec<Signal<In, Clock>> {
        vec![xem_6010_base_clock()]
    }

    fn synth<U: Block>(uut: U, dir: &str) {
        crate::synth::synth_obj(uut, dir)
    }
}

#[derive(LogicBlock)]
pub struct OKTest1 {
    pub hi: OpalKellyHostInterface,
    pub ok_host: OpalKellyHost,
    pub led: Signal<Out, Bits<8>>,
    pub pulser: Pulser,
}

impl OKTest1 {
    pub fn new() -> Self {
        Self {
            hi: OpalKellyHostInterface::xem_6010(),
            ok_host: OpalKellyHost::xem_6010(),
            led: pins::xem_6010_leds(),
            pulser: Pulser::new(MHZ48, 1.0, Duration::from_millis(500)),
        }
    }
}

impl Logic for OKTest1 {
    #[hdl_gen]
    fn update(&mut self) {
        self.hi.link(&mut self.ok_host.hi);
        self.pulser.clock.next = self.ok_host.ti_clk.val();
        self.pulser.enable.next = true;
        if self.pulser.pulse.val() {
            self.led.next = 0xFF_u8.into();
        } else {
            self.led.next = 0x00_u8.into();
        }
    }
}

#[test]
fn test_ok_host_synthesizable() {
    let mut uut = OKTest1::new();
    uut.hi.sig_in.connect();
    uut.hi.sig_out.connect();
    uut.hi.sig_inout.connect();
    uut.hi.sig_aa.connect();
    uut.connect_all();
    check_connected(&uut);
    let vlog = generate_verilog(&uut);
    println!("{}", vlog);
    let ucf = rust_hdl_toolchain_ise::ucf_gen::generate_ucf(&uut);
    println!("{}", ucf);
    rust_hdl_yosys_synth::yosys_validate("vlog", &vlog).unwrap();
}
