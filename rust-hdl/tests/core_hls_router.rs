use rust_hdl::core::prelude::*;
use rust_hdl::hls::prelude::*;

#[derive(LogicBlock)]
struct RouterTest {
    router: Router<16, 8, 6>,
    clock: Signal<In, Clock>,
}

struct DummyBridge(pub usize);

impl HLSNamedPorts for DummyBridge {
    fn ports(&self) -> Vec<String> {
        (0..self.0).map(|x| format!("port_{}", x)).collect()
    }
}

impl Default for RouterTest {
    fn default() -> Self {
        let dummy_bridges = [
            &DummyBridge(4) as &dyn HLSNamedPorts,
            &DummyBridge(8),
            &DummyBridge(12),
            &DummyBridge(4),
            &DummyBridge(4),
            &DummyBridge(4),
        ];

        let names = ["a", "b", "c", "d", "e", "f"];
        let router = Router::<16, 8, 6>::new(names, dummy_bridges);
        Self {
            router,
            clock: Default::default(),
        }
    }
}

impl Logic for RouterTest {
    #[hdl_gen]
    fn update(&mut self) {
        self.router.upstream.clock.next = self.clock.val();
    }
}

#[cfg(test)]
fn make_test_router() -> RouterTest {
    let mut uut = RouterTest::default();
    uut.router.upstream.address.connect();
    uut.router.upstream.address_strobe.connect();
    uut.router.upstream.from_controller.connect();
    uut.router.upstream.strobe.connect();
    uut.router.upstream.clock.connect();
    for i in 0..6 {
        uut.router.nodes[i].ready.connect();
        uut.router.nodes[i].to_controller.connect();
    }
    uut.router.connect_all();
    uut
}

#[test]
fn test_router_is_synthesizable() {
    let router = make_test_router();
    let vlog = generate_verilog(&router);
    yosys_validate("router", &vlog).unwrap();
}

#[test]
fn test_router_function() {
    let router = make_test_router();
    let mut sim = Simulation::new();
    sim.add_clock(5, |x: &mut Box<RouterTest>| x.clock.next = !x.clock.val());
    sim.add_testbench(move |mut sim: Sim<RouterTest>| {
        let mut x = sim.init()?;
        wait_clock_true!(sim, clock, x);
        x.router.upstream.address.next = 7.into();
        x.router.upstream.address_strobe.next = true;
        wait_clock_cycle!(sim, clock, x);
        x.router.upstream.address_strobe.next = false;
        x = sim.watch(|x| x.router.upstream.ready.val(), x)?;
        x.router.upstream.from_controller.next = 0xDEAD.into();
        x.router.upstream.strobe.next = true;
        wait_clock_cycle!(sim, clock, x);
        x.router.upstream.strobe.next = false;
        sim.done(x)
    });
    sim.add_testbench(move |mut sim: Sim<RouterTest>| {
        let mut x = sim.init()?;
        wait_clock_true!(sim, clock, x);
        x = sim.watch(|x| x.router.nodes[1].address.val() == 0x03, x)?;
        x.router.nodes[1].ready.next = true;
        x = sim.watch(|x| x.router.nodes[1].strobe.val(), x)?;
        sim_assert!(sim, x.router.nodes[1].from_controller.val() == 0xDEAD, x);
        wait_clock_cycles!(sim, clock, x, 10);
        sim.done(x)
    });
    sim.run_traced(
        Box::new(router),
        1000,
        std::fs::File::create(vcd_path!("router.vcd")).unwrap(),
    )
    .unwrap();
}

#[derive(LogicBlock)]
struct RouterTestDevice {
    pub upstream: SoCBusResponder<16, 8>,
    bridge: Bridge<16, 8, 5>,
    mosi_ports: [MOSIPort<16>; 5],
}

impl HLSNamedPorts for RouterTestDevice {
    fn ports(&self) -> Vec<String> {
        self.bridge.ports()
    }
}

impl Default for RouterTestDevice {
    fn default() -> Self {
        Self {
            upstream: Default::default(),
            bridge: Bridge::new(["mosi_0", "mosi_1", "mosi_2", "mosi_3", "mosi_4"]),
            mosi_ports: array_init::array_init(|_| Default::default()),
        }
    }
}

impl Logic for RouterTestDevice {
    #[hdl_gen]
    fn update(&mut self) {
        SoCBusResponder::<16, 8>::link(&mut self.upstream, &mut self.bridge.upstream);
        for i in 0..5 {
            SoCPortController::<16>::join(&mut self.bridge.nodes[i], &mut self.mosi_ports[i].bus);
            self.mosi_ports[i].ready.next = self.mosi_ports[i].bus.select.val();
        }
    }
}

#[test]
fn test_device_synthesizes() {
    let mut uut = RouterTestDevice::default();
    uut.upstream.clock.connect();
    uut.upstream.from_controller.connect();
    uut.upstream.address.connect();
    uut.upstream.address_strobe.connect();
    uut.upstream.strobe.connect();
    for i in 0..5 {
        uut.mosi_ports[i].ready.connect();
    }
    uut.connect_all();
    let vlog = generate_verilog(&uut);
    yosys_validate("router_test_device", &vlog).unwrap();
}

#[derive(LogicBlock)]
struct RouterTestSetup {
    pub upstream: SoCBusResponder<16, 8>,
    router: Router<16, 8, 3>,
    dev_a: [RouterTestDevice; 3],
}

impl Default for RouterTestSetup {
    fn default() -> Self {
        let dev_a = array_init::array_init(|_| Default::default());
        let names = ["a", "b", "c"];
        Self {
            upstream: Default::default(),
            router: Router::new(names, [&dev_a[0], &dev_a[1], &dev_a[2]]),
            dev_a,
        }
    }
}

impl Logic for RouterTestSetup {
    #[hdl_gen]
    fn update(&mut self) {
        SoCBusResponder::<16, 8>::link(&mut self.upstream, &mut self.router.upstream);
        for i in 0..3 {
            SoCBusController::<16, 8>::join(&mut self.router.nodes[i], &mut self.dev_a[i].upstream);
        }
    }
}

#[cfg(test)]
fn make_router_test_setup() -> RouterTestSetup {
    let mut uut = RouterTestSetup::default();
    for dev in 0..3 {
        for port in 0..5 {
            uut.dev_a[dev].mosi_ports[port].ready.connect();
        }
    }
    uut.connect_all();
    uut
}

#[test]
fn test_router_test_setup_synthesizes() {
    let uut = make_router_test_setup();
    let vlog = generate_verilog(&uut);
    println!("{}", vlog);
    yosys_validate("router_test_setup", &vlog).unwrap();
}

#[test]
fn test_router_test_setup_works() {
    let uut = make_router_test_setup();
    let mut sim = Simulation::new();
    sim.add_clock(5, |x: &mut Box<RouterTestSetup>| {
        x.upstream.clock.next = !x.upstream.clock.val()
    });
    let dataset = [
        0xBEAF, 0xDEED, 0xCAFE, 0xBABE, 0x1234, 0x5678, 0x900B, 0xB001, 0xDEAD, 0xBEEF, 0x5EA1,
        0x5AFE, 0xAAAA, 0x5A13, 0x8675,
    ];
    sim.add_testbench(move |mut sim: Sim<RouterTestSetup>| {
        let mut x = sim.init()?;
        wait_clock_true!(sim, upstream.clock, x);
        for address in 0..15 {
            // Sweep the address space...
            x.upstream.address.next = address.into();
            x.upstream.from_controller.next = dataset[address as usize].into();
            x.upstream.address_strobe.next = true;
            wait_clock_cycle!(sim, upstream.clock, x);
            x.upstream.address_strobe.next = false;
            x = sim.watch(|x| x.upstream.ready.val(), x)?;
            x.upstream.strobe.next = true;
            wait_clock_cycle!(sim, upstream.clock, x);
            x.upstream.strobe.next = false;
            wait_clock_cycle!(sim, upstream.clock, x);
        }
        sim.done(x)
    });
    sim.add_testbench(move |mut sim: Sim<RouterTestSetup>| {
        let mut x = sim.init()?;
        wait_clock_true!(sim, upstream.clock, x);
        for dev in 0..3 {
            for node in 0..5 {
                x = sim.watch(
                    move |x| {
                        x.dev_a[dev.clone()].mosi_ports[node.clone()]
                            .bus
                            .select
                            .val()
                    },
                    x,
                )?;
                x = sim.watch(
                    move |x| {
                        x.dev_a[dev.clone()].mosi_ports[node.clone()]
                            .strobe_out
                            .val()
                    },
                    x,
                )?;
                println!(
                    "Dataset {} {} {:x} {:x}",
                    dev,
                    node,
                    dataset[dev * 5 + node],
                    x.dev_a[dev.clone()].mosi_ports[node.clone()].port_out.val()
                );
                sim_assert_eq!(
                    sim,
                    x.dev_a[dev.clone()].mosi_ports[node.clone()].port_out.val(),
                    dataset[dev * 5 + node],
                    x
                );
                wait_clock_cycle!(sim, upstream.clock, x);
            }
        }
        sim.done(x)
    });
    sim.run_traced(
        Box::new(uut),
        10000,
        std::fs::File::create(vcd_path!("router_test_setup_function.vcd")).unwrap(),
    )
    .unwrap();
}
