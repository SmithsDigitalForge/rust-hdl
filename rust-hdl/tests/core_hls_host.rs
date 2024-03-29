use rand::Rng;
use rust_hdl::core::check_timing::check_timing;
use rust_hdl::core::prelude::*;
use rust_hdl::hls::prelude::*;

#[derive(LogicBlock)]
struct HostTest {
    pc_to_host: SyncFIFO<Bits<8>, 3, 4, 1>,
    host_to_pc: SyncFIFO<Bits<8>, 3, 4, 1>,
    bidi_dev: BidiSimulatedDevice<Bits<8>>,
    host: Host<8>,
    bridge: Bridge<16, 8, 3>,
    port: MOSIPort<16>,
    iport: MISOPort<16>,
    fport: MISOFIFOPort<16, 3, 4, 1>,
    pub bidi_clock: Signal<In, Clock>,
    pub sys_clock: Signal<In, Clock>,
}

impl Default for HostTest {
    fn default() -> Self {
        Self {
            pc_to_host: Default::default(),
            host_to_pc: Default::default(),
            bidi_dev: Default::default(),
            host: Default::default(),
            bridge: Bridge::new(["port", "iport", "fport"]),
            port: Default::default(),
            iport: Default::default(),
            fport: Default::default(),
            bidi_clock: Default::default(),
            sys_clock: Default::default(),
        }
    }
}

impl Logic for HostTest {
    #[hdl_gen]
    fn update(&mut self) {
        FIFOReadController::<Bits<8>>::join(
            &mut self.bidi_dev.data_to_bus,
            &mut self.pc_to_host.bus_read,
        );
        FIFOWriteController::<Bits<8>>::join(
            &mut self.bidi_dev.data_from_bus,
            &mut self.host_to_pc.bus_write,
        );
        clock!(self, bidi_clock, host_to_pc, pc_to_host);
        self.bidi_dev.clock.next = self.bidi_clock.val();
        BidiBusD::<Bits<8>>::join(&mut self.bidi_dev.bus, &mut self.host.bidi_bus);
        self.host.bidi_clock.next = self.bidi_clock.val();
        self.host.sys_clock.next = self.sys_clock.val();
        SoCBusController::<16, 8>::join(&mut self.host.bus, &mut self.bridge.upstream);
        SoCPortController::<16>::join(&mut self.bridge.nodes[0], &mut self.port.bus);
        SoCPortController::<16>::join(&mut self.bridge.nodes[1], &mut self.iport.bus);
        SoCPortController::<16>::join(&mut self.bridge.nodes[2], &mut self.fport.bus);
        self.port.ready.next = true;
    }
}

#[cfg(test)]
fn make_host_test() -> HostTest {
    let mut uut = HostTest::default();
    uut.iport.port_in.connect();
    uut.iport.ready_in.connect();
    uut.pc_to_host.bus_write.data.connect();
    uut.pc_to_host.bus_write.write.connect();
    uut.host_to_pc.bus_read.read.connect();
    uut.fport.fifo_bus.link_connect_dest();
    uut.connect_all();
    uut
}

#[test]
fn test_host_test_synthesizes() {
    let uut = make_host_test();
    let vlog = generate_verilog(&uut);
    yosys_validate("host", &vlog).unwrap();
    check_timing(&make_host_test())
}

#[test]
fn test_ping_works() {
    let uut = make_host_test();
    let mut sim = Simulation::new();
    sim.add_clock(5, |x: &mut Box<HostTest>| {
        x.bidi_clock.next = !x.bidi_clock.val()
    });
    sim.add_clock(4, |x: &mut Box<HostTest>| {
        x.sys_clock.next = !x.sys_clock.val()
    });
    sim.add_testbench(move |mut sim: Sim<HostTest>| {
        let mut x = sim.init()?;
        // Wait for reset to complete
        wait_clock_cycles!(sim, bidi_clock, x, 20);
        for iter in 0..10 {
            wait_clock_cycles!(sim, bidi_clock, x, 5);
            // Send a ping command with ID of 0x67, followed by a NOOP
            hls_host_ping!(sim, bidi_clock, x, pc_to_host, 0x67_u8 + iter);
            hls_host_noop!(sim, bidi_clock, x, pc_to_host);
        }
        sim.done(x)
    });
    sim.add_testbench(move |mut sim: Sim<HostTest>| {
        let mut x = sim.init()?;
        for iter in 0..10 {
            let word = hls_host_get_word!(sim, bidi_clock, x, host_to_pc);
            sim_assert!(sim, word == 0x0167 + iter, x);
        }
        sim.done(x)
    });
    sim.run_traced(
        Box::new(uut),
        10000,
        std::fs::File::create(vcd_path!("host_ping.vcd")).unwrap(),
    )
    .unwrap();
}

#[test]
fn test_write_command_works() {
    let uut = make_host_test();
    let mut sim = Simulation::new();
    sim.add_clock(5, |x: &mut Box<HostTest>| {
        x.bidi_clock.next = !x.bidi_clock.val()
    });
    sim.add_clock(4, |x: &mut Box<HostTest>| {
        x.sys_clock.next = !x.sys_clock.val()
    });
    sim.add_testbench(move |mut sim: Sim<HostTest>| {
        let mut x = sim.init()?;
        wait_clock_cycles!(sim, bidi_clock, x, 20); // Wait for reset
        for iter in 0..10 {
            wait_clock_cycles!(sim, bidi_clock, x, 5);
            // Write a sequence of bytes to the end point
            let to_send = (0..iter + 1).map(|x| 0x7870_u16 + x).collect::<Vec<_>>();
            hls_host_write!(sim, bidi_clock, x, pc_to_host, 0x00, to_send);
            hls_host_noop!(sim, bidi_clock, x, pc_to_host);
        }
        sim.done(x)
    });
    sim.add_testbench(move |mut sim: Sim<HostTest>| {
        let mut x = sim.init()?;
        wait_clock_true!(sim, sys_clock, x);
        for iter in 0..10 {
            for ndx in 0..(iter + 1) {
                x = sim.watch(|x| x.port.strobe_out.val(), x)?;
                sim_assert!(sim, x.port.port_out.val() == (0x7870 + ndx), x);
                wait_clock_cycle!(sim, sys_clock, x);
            }
        }
        sim.done(x)
    });
    sim.run_traced(
        Box::new(uut),
        10000,
        std::fs::File::create(vcd_path!("host_write.vcd")).unwrap(),
    )
    .unwrap();
}

#[test]
fn test_read_command_works() {
    let uut = make_host_test();
    let mut sim = Simulation::new();
    sim.add_clock(5, |x: &mut Box<HostTest>| {
        x.bidi_clock.next = !x.bidi_clock.val()
    });
    sim.add_clock(4, |x: &mut Box<HostTest>| {
        x.sys_clock.next = !x.sys_clock.val()
    });
    sim.add_testbench(move |mut sim: Sim<HostTest>| {
        let mut x = sim.init()?;
        wait_clock_cycles!(sim, bidi_clock, x, 20); // Wait for reset
        wait_clock_true!(sim, bidi_clock, x);
        for iter in 0..10 {
            wait_clock_cycles!(sim, bidi_clock, x, 5);
            // Issue a read command to the host
            hls_host_issue_read!(sim, bidi_clock, x, pc_to_host, 0x01, (iter + 1));
            let vals = hls_host_get_words!(sim, bidi_clock, x, host_to_pc, (iter + 1));
            println!("{:x?}", vals);
            for ndx in 0..(iter + 1) {
                sim_assert!(sim, vals[ndx as usize] == 0xBEE0 + ndx, x);
            }
            wait_clock_cycles!(sim, bidi_clock, x, 5);
        }
        sim.done(x)
    });
    sim.add_testbench(move |mut sim: Sim<HostTest>| {
        let mut x = sim.init()?;
        wait_clock_true!(sim, sys_clock, x);
        for iter in 0..10 {
            wait_clock_cycles!(sim, sys_clock, x, 10);
            for ndx in 0..(iter + 1) {
                x.iport.port_in.next = (0xBEE0 + ndx).into();
                x.iport.ready_in.next = true;
                x = sim.watch(|x| x.iport.strobe_out.val(), x)?;
                wait_clock_cycle!(sim, sys_clock, x);
                x.iport.ready_in.next = false;
            }
        }
        sim.done(x)
    });
    sim.run_traced(
        Box::new(uut),
        20000,
        std::fs::File::create(vcd_path!("host_read.vcd")).unwrap(),
    )
    .unwrap();
}

#[test]
fn test_stream_command_works() {
    let uut = make_host_test();
    let mut sim = Simulation::new();
    sim.add_clock(5, |x: &mut Box<HostTest>| {
        x.bidi_clock.next = !x.bidi_clock.val()
    });
    sim.add_clock(4, |x: &mut Box<HostTest>| {
        x.sys_clock.next = !x.sys_clock.val()
    });
    sim.add_testbench(move |mut sim: Sim<HostTest>| {
        let mut x = sim.init()?;
        wait_clock_cycles!(sim, bidi_clock, x, 20); // Wait for reset
                                                    // Send a PING command
        wait_clock_true!(sim, bidi_clock, x);
        wait_clock_cycles!(sim, bidi_clock, x, 5);
        // A stream command looks like 0x05XX, where XX is the address to stream from
        // Write the command
        hls_host_put_word!(sim, bidi_clock, x, pc_to_host, 0x0502);
        let vals = hls_host_get_words!(sim, bidi_clock, x, host_to_pc, 100);
        // Wait until we have collected 100 items
        for iter in 0..100 {
            sim_assert_eq!(sim, vals[iter as usize], 0xBAB0 + iter, x);
        }
        // Send a stop command (anything non-zero)
        hls_host_put_word!(sim, bidi_clock, x, pc_to_host, 0x0502);
        hls_host_drain!(sim, bidi_clock, x, host_to_pc);
        hls_host_ping!(sim, bidi_clock, x, pc_to_host, 0xFF);
        let ping = hls_host_get_word!(sim, bidi_clock, x, host_to_pc);
        sim_assert_eq!(sim, ping, 0x01FF, x);
        sim.done(x)
    });
    sim.add_testbench(move |mut sim: Sim<HostTest>| {
        let mut x = sim.init()?;
        wait_clock_true!(sim, sys_clock, x);
        wait_clock_cycles!(sim, sys_clock, x, 20);
        let o_data = (0..100).map(|x| 0xBAB0_u16 + x).collect::<Vec<_>>();
        hls_fifo_write_lazy!(sim, sys_clock, x, fport.fifo_bus, &o_data);
        sim.done(x)
    });
    sim.run_traced(
        Box::new(uut),
        50000,
        std::fs::File::create(vcd_path!("host_stream.vcd")).unwrap(),
    )
    .unwrap();
}
