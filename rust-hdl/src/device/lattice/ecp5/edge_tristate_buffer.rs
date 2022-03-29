use crate::core::prelude::*;
use crate::widgets::prelude::{TristateBuffer, DFF};

#[derive(Clone, Debug, Copy, Default)]
pub struct IODelays {
    pub to_pin: u8,
    pub from_pin: u8,
}

#[derive(LogicBlock, Default)]
pub struct EdgeTristateBuffer<T: Synth> {
    pub to_pin: Signal<In, T>,
    pub from_pin: Signal<Out, T>,
    pub output_enable: Signal<In, Bit>,
    pub clk: Signal<In, Clock>,
    pub pin: Signal<InOut, T>,
    dff_out: DFF<T>,
    dff_in: DFF<T>,
    buffer: TristateBuffer<T>,
    _delay: IODelays,
}

impl<T: Synth> EdgeTristateBuffer<T> {
    pub fn new(delay: IODelays) -> Self {
        Self {
            _delay: delay,
            ..Default::default()
        }
    }
}

fn wrapper_once(delay: IODelays) -> String {
    format!(
        r##"
    wire bb_to_pin_a;
    wire bb_to_pin_z;
    wire bb_from_pin_a;
    wire bb_from_pin_z;

    OFS1P3DX obuf(.D(to_pin), .CD(0), .SP(1), .SCLK(clk), .Q(bb_to_pin_a));
    IFS1P3DX ibuf(.D(bb_from_pin_z), .CD(0), .SP(1), .SCLK(clk), .Q(from_pin));
    BB bb(.I(bb_to_pin_z), .O(bb_from_pin_a), .B(pin), .T(~output_enable));

    defparam dg.DEL_VALUE = {delay_to_pin};
    defparam dg.DEL_MODE = "USER_DEFINED";
    DELAYG dg(.A(bb_to_pin_a),.Z(bb_to_pin_z));

    defparam dg.DEL_VALUE = {delay_from_pin};
    defparam dg.DEL_MODE = "USER_DEFINED";
    DELAYG dg(.A(bb_from_pin_a),.Z(bb_from_pin_z));
"##,
        delay_to_pin = delay.to_pin,
        delay_from_pin = delay.from_pin
    )
}

fn wrapper_multiple(count: usize, delay: IODelays) -> String {
    let bufs = (0..count)
        .map(|x| {
            format!(
                r#"
    OFS1P3DX obuf_{x}(.D(to_pin[{x}]), .CD(0), .SP(1), .SCLK(clk), .Q(bb_to_pin_a[{x}]));
    IFS1P3DX ibuf_{x}(.D(bb_from_pin_z[{x}]), .CD(0), .SP(1), .SCLK(clk), .Q(from_pin[{x}]));
    BB bb_{x}(.I(bb_to_pin_z[{x}]), .O(bb_from_pin_a[{x}]), .B(pin[{x}]), .T(~output_enable));

    defparam dg_to_pin_{x}.DEL_VALUE = {delay_to_pin};
    defparam dg_to_pin_{x}.DEL_MODE = "USER_DEFINED";
    DELAYG dg_to_pin_{x}(.A(bb_to_pin_a[{x}]),.Z(bb_to_pin_z[{x}]));

    defparam dg_from_pin_{x}.DEL_VALUE = {delay_from_pin};
    defparam dg_from_pin_{x}.DEL_MODE = "USER_DEFINED";
    DELAYG dg_from_pin_{x}(.A(bb_from_pin_a[{x}]),.Z(bb_from_pin_z[{x}]));
        "#,
                x = x,
                delay_to_pin = delay.to_pin,
                delay_from_pin = delay.from_pin
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r##"
wire [{B}:0] bb_to_pin_a;
wire [{B}:0] bb_to_pin_z;
wire [{B}:0] bb_from_pin_a;
wire [{B}:0] bb_from_pin_z;

{bufs}
    "##,
        B = count,
        bufs = bufs
    )
}

impl<T: Synth> Logic for EdgeTristateBuffer<T> {
    fn update(&mut self) {
        self.dff_out.clk.next = self.clk.val();
        self.dff_in.clk.next = self.clk.val();
        self.buffer.write_enable.next = self.output_enable.val();
        self.dff_in.d.next = self.buffer.read_data.val();
        self.dff_out.d.next = self.to_pin.val();
        self.buffer.write_data.next = self.dff_out.q.val();
        self.from_pin.next = self.dff_in.q.val();
        Signal::<InOut, T>::link(&mut self.pin, &mut self.buffer.bus);
    }
    fn connect(&mut self) {
        self.dff_out.clk.connect();
        self.dff_in.clk.connect();
        self.buffer.write_enable.connect();
        self.dff_in.d.connect();
        self.dff_out.d.connect();
        self.buffer.write_data.connect();
        self.from_pin.connect();
    }
    fn hdl(&self) -> Verilog {
        Verilog::Wrapper(Wrapper {
            code: if T::BITS == 1 {
                wrapper_once(self._delay).to_string()
            } else {
                wrapper_multiple(T::BITS, self._delay)
            },
            cores: r##"
(* blackbox *)
module IFS1P3DX(input D, input SP, input SCLK, input CD, output Q);
endmodule

(* blackbox *)
module OFS1P3DX(input D, input SP, input SCLK, input CD, output Q);
endmodule

(* blackbox *)
module BB(input I, input T, output O, inout B);
endmodule

(* blackbox *)
module DELAYG(input A, output Z);
parameter DEL_MODE = "USER_DEFINED";
parameter DEL_VALUE = 0;
endmodule

            "##
            .into(),
        })
    }
}

#[test]
fn test_edge_buffer_synthesizes() {
    let mut uut = TopWrap::new(EdgeTristateBuffer::<Bits<8>>::default());
    uut.uut.output_enable.connect();
    uut.uut.to_pin.connect();
    uut.uut.clk.connect();
    uut.uut.pin.connect();
    uut.connect_all();
    std::fs::write("edge_tristate_buffer.v", generate_verilog(&uut)).unwrap();
    yosys_validate("edge_tristate_buffer", &generate_verilog(&uut)).unwrap();
}

#[test]
fn test_edge_buffer_synthesizes_with_delay() {
    let mut uut = TopWrap::new(EdgeTristateBuffer::<Bits<8>>::new(IODelays {
        to_pin: 25,
        from_pin: 40,
    }));
    uut.uut.output_enable.connect();
    uut.uut.to_pin.connect();
    uut.uut.clk.connect();
    uut.uut.pin.connect();
    uut.connect_all();
    std::fs::write("edge_tristate_buffer_delay.v", generate_verilog(&uut)).unwrap();
    yosys_validate("edge_tristate_buffer", &generate_verilog(&uut)).unwrap();
}