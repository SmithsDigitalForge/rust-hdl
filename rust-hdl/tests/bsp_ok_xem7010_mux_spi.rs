use rust_hdl::core::prelude::*;
use rust_hdl::widgets::prelude::*;
use test_common::tools::*;
use test_common::mux_spi::*;
use rust_hdl::bsp::ok_xem7010::XEM7010;
use rust_hdl::bsp::ok_core::prelude::*;

mod test_common;

#[test]
fn test_opalkelly_xem_7010_mux_spi() {
    let mut uut = OpalKellySPIMuxTest::new::<XEM7010>();
    uut.hi.link_connect_dest();
    uut.connect_all();
    XEM7010::synth(uut, target_path!("xem_7010/mux_spi"));
    test_opalkelly_mux_spi_runtime(target_path!("xem_7010/mux_spi/top.bit")).unwrap()
}