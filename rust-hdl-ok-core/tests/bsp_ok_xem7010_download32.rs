mod test_common;
use rust_hdl_ok_core::core::prelude::*;

use rust_hdl::core::prelude::*;
use rust_hdl_ok_core::xem7010::XEM7010;

use test_common::download::*;

#[test]
fn test_opalkelly_xem_7010_synth_download32() {
    let mut uut = OpalKellyDownload32FIFOTest::new::<XEM7010>();
    uut.hi.link_connect_dest();
    uut.connect_all();
    XEM7010::synth(uut, target_path!("xem_7010/download32"));
    test_opalkelly_download32_runtime(target_path!("xem_7010/download32/top.bit")).unwrap();
}
