/* Lab 2, Hadrian Ward, Calculator
Functions, encoded by first 3 switches
0 -> XOR
1 -> AND
2 -> OR
3 -> +
4 -> - (2's comp)

scp "C:/Users/hfward/Desktop/lab_2_calc/lab_2_calc.srcs/sources_1/new/lab_2_calc.v" hward@142.0.97.104:/home/hward/rust/evolution_model/resources/http
*/


// Produces lower-speed clock signals for switch debouncer and 7 segment driver
module clk_divider(
	input clk,// 100 MHz
	output led_driver_enable,// 763 Hz
	output debouncer_enable// 23 Hz
	);

	reg [22:0] counter;// 23 bits needed to divide 100 MHz to ~23 Hz for the debouncer
	assign led_driver_enable = (counter[16:0] == 17'b0) ? 1'b1 : 1'b0;
	assign debouncer_enable = (counter[21:0] == 22'b0) ? 1'b1 : 1'b0;

	always @ (posedge clk) begin
		counter <= counter + 1;
	end
endmodule

// Poll button at slow enough rate to prevent sampling more than once during a bounce
module debouncer(
	input clk,
	input clk_en,
	input button_in,
	output button_edge
	);

	reg [0:0] button_state;
	reg [0:0] button_state_prev;
	
	
	assign button_edge = (button_state_prev == 1'b0 && button_state == 1'b1)? 1'b1 : 1'b0;
	
	always @ (posedge clk) begin
		if(clk_en == 1'b1) begin
			button_state <= button_in;
		end
		button_state_prev <= button_state;
	end
endmodule

// When an input signal goes high, the output will go high only for 1 clock cycle
// Usefull for state machines
module edge_detect(
	input clk,
	input state_in,
	output state_edge
);
	reg [0:0] prev_state;
	assign state_edge = (prev_state == 1'b0 && state_in == 1'b1)? 1'b1 : 1'b0;
	
	always @ (posedge clk) begin
		prev_state <= state_in;
	end
endmodule

// 7-segment LED multiplexer
// Help from https://www.fpga4student.com/2017/09/seven-segment-led-display-controller-basys3-fpga.html
module numeric_leds_multiplexer(
	input clk,
	input clk_en,
	input [3:0] thousands,
	input [3:0] hundreds,
	input [3:0] tens,
	input [3:0] units,
	output reg [3:0] anodes,// 3 digits, 0=units, 1=tens, 2=hundreds
	output reg [6:0] cathodes// 7 segments per digit
	);

	reg [1:0] digit_activation;// 0, 1, 2, 3
	reg [3:0] current_displayed_bcd;

	// Assign cathodes
	always @ (*) begin
		case(current_displayed_bcd)
			4'b0000: cathodes <= 7'b1000000; // "0"
			4'b0001: cathodes <= 7'b1111001; // "1"
			4'b0010: cathodes <= 7'b0100100; // "2"
			4'b0011: cathodes <= 7'b0110000; // "3"
			4'b0100: cathodes <= 7'b0011001; // "4"
			4'b0101: cathodes <= 7'b0010010; // "5"
			4'b0110: cathodes <= 7'b0000010; // "6"
			4'b0111: cathodes <= 7'b1111000; // "7"
			4'b1000: cathodes <= 7'b0000000; // "8"
			4'b1001: cathodes <= 7'b0010000; // "9"
			4'b1010: cathodes <= 7'b0001000;    //Display A
			4'b1011: cathodes <= 7'b0000011;    //Display b
			4'b1100: cathodes <= 7'b1000110;    //Display C
			4'b1101: cathodes <= 7'b0100001;    //Display d
			4'b1110: cathodes <= 7'b0000110;    //Display E
			4'b1111: cathodes <= 7'b0001110;    //Display F
		endcase
	end

	// Assign anodes
	always @ (*) begin
		case(digit_activation)
			2'b0: begin// Units
				anodes = 4'b1110;
				current_displayed_bcd = units;
			end
			2'b1: begin// Tens
				anodes = 4'b1101;
				current_displayed_bcd = tens;
			end
			2'b10: begin// Hundreds
				anodes = 4'b1011;
				current_displayed_bcd = hundreds;
			end
			2'b11: begin// Thousands
				anodes = 4'b0111;
				current_displayed_bcd = thousands;
			end
		endcase
	end

	// Increment digit counter
	always @ (posedge clk) begin
		if(clk_en) begin
			digit_activation <= digit_activation + 2'b1;
		end
	end
endmodule

// Math & Logic
module alu(
	input clk,
	input [7:0] sw,
	input load_a,
	input load_b,
	input load_op,
	output reg [7:0] display_out
);
	reg [7:0] a;
	reg [7:0] b;
	
	always @ (posedge clk) begin
		if(load_a == 1'b1) begin
			a <= sw;
			display_out <= sw;
		end
		if(load_b == 1'b1) begin
			b <= sw;
			display_out <= sw;
		end
		if(load_op == 1'b1) begin
			case(sw[2:0])
				3'b000: display_out <= a ^ b;
				3'b001: display_out <= a & b;
				3'b010: display_out <= a | b;
				3'b011: display_out <= a + b;
				3'b100: display_out <= ((~b) + 8'b1) + (~((~a) + 8'b1)) + 8'b1;// 2's comp
				default: display_out <= 8'b0;
			endcase
		end
	end
endmodule

module central_timing(
	input btn_edge_l,// Left button
	input btn_edge_r,// Right button
	input clk,// 100 MHz
	input reset,// Active high
	output load_a,
	output load_b,
	output load_op,
	output [1:0] state_debug
	);
	
	localparam STATE_DEFAULT = 2'B0;
	localparam STATE_WAITING_2ND_OPERAND = 2'B1;
	localparam STATE_WAITING_OPCODE = 2'B10;
	localparam STATE_SHOWING_RESULT = 2'B11;
	
	// 0 -> Default, waiting to load 1st operand
	// 1 -> Waiting to load 2nd operand
	// 2 -> Waiting to load operation
	// 3 -> Showing result, waiting for left button to clear
	reg [1:0] curr_state;
	assign state_debug = curr_state;
	
	// Next state
	always @ (posedge clk) begin
		if(reset) begin
			curr_state <= STATE_DEFAULT;
		end
		case(curr_state)
			STATE_DEFAULT: begin
				if(btn_edge_r) begin
					curr_state <= STATE_WAITING_2ND_OPERAND;
				end
				else begin
					curr_state <= STATE_DEFAULT;
				end
			end
			STATE_WAITING_2ND_OPERAND: begin
				if(btn_edge_r) begin
					curr_state <= STATE_WAITING_OPCODE;
				end
				else begin
					curr_state <= STATE_WAITING_2ND_OPERAND;
				end
			end
			STATE_WAITING_OPCODE: begin
				if(btn_edge_r) begin
					curr_state <= STATE_SHOWING_RESULT;
				end
				else begin
					curr_state <= STATE_WAITING_OPCODE;
				end
			end
			STATE_SHOWING_RESULT: begin
				if(btn_edge_l) begin
					curr_state <= STATE_DEFAULT;
				end
				else begin
					curr_state <= STATE_SHOWING_RESULT;
				end
			end
		endcase
	end
	
	// Combinational output from  state
	edge_detect load_a_edge_detector(
		.clk(clk),
		.state_in((curr_state == STATE_WAITING_2ND_OPERAND)? 1'b1 : 1'b0),
		.state_edge(load_a)
	);
	edge_detect load_b_edge_detector(
		.clk(clk),
		.state_in((curr_state == STATE_WAITING_OPCODE)? 1'b1 : 1'b0),
		.state_edge(load_b)
	);
	edge_detect load_op_edge_detector(
		.clk(clk),
		.state_in((curr_state == STATE_SHOWING_RESULT)? 1'b1 : 1'b0),
		.state_edge(load_op)
	);
endmodule

// Toplevel, instantiate and connect everything
module top_microwave_timer(
	input [7:0] sw,// Switches
	input btnR,// Right button
	input btnL,// Left button
	input clk,// 100 MHz
	output [15:0] led,
	output [3:0] an,// 7 segment anodes
	output [6:0] seg// 7 segment cathodes
	);
	
	// Internal wires
	wire led_driver_clk_enable;// When lower 17 bits are zero (763 Hz)
	wire debouncer_clk_enable;
	wire btn_edge_l;
	wire btn_edge_r;
	wire load_a;
	wire load_b;
	wire load_op;
	wire [7:0] result;
	
	// LED Assignments
	// 1:0 - State machine state
	// 15:8 ALU Output
	assign led[15:8] = result;
	assign led[7:2] = 6'b0;
	
	// Instantiate submodules
	clk_divider clk_divider_instance(
		.clk(clk),
		.led_driver_enable(led_driver_clk_enable),
		.debouncer_enable(debouncer_clk_enable)
	);
	
	debouncer debouncer_left(
		.clk(clk),
		.clk_en(debouncer_clk_enable),
		.button_in(btnL),
		.button_edge(btn_edge_l)
	);
	
	debouncer debouncer_right(
		.clk(clk),
		.clk_en(debouncer_clk_enable),
		.button_in(btnR),
		.button_edge(btn_edge_r)
	);
	
	numeric_leds_multiplexer numeric_leds_multiplexer_instance(
		.clk(clk),
		.clk_en(led_driver_clk_enable),//.clk_en(led_driver_clk_enable),
		.thousands(8'b0),
		.hundreds(8'b0),
		.tens(result[7:4]),
		.units(result[3:0]),
		.anodes(an),// 3 digits, 0=units, 1=tens, 2=hundreds, 3=thousands
		.cathodes(seg)// 7 segments per digit
	);
	
	alu alu_instance(
		.clk(clk),
		.sw(sw),
		.load_a(load_a),
		.load_b(load_b),
		.load_op(load_op),
		.display_out(result)
	);
	
	central_timing central_timing_instance(
		.btn_edge_l(btn_edge_l),
		.btn_edge_r(btn_edge_r),
		.clk(clk),// 100 MHz,
		.reset(1'b0),
		.load_a(load_a),
		.load_b(load_b),
		.load_op(load_op),
		.state_debug(led[1:0])
	);
endmodule