// Lab 1 microwave timer
// Hadrian Ward
// scp "C:/Users/hfward/lab_1_timer/lab_1_timer.srcs/sources_1/new/lab_1_timer.v" hward@142.0.97.104:/home/hward/rust/evolution_model/resources/http

// CHECKED
module down_counter(
	input [7:0] sw,
	input load_sw,// Active high, read on +edge
	input clk_en,// Active high
	input clk,
	output done,
	output reg [7:0] count
	);

	assign done = (count == 8'b0);
	
	always @ (posedge clk) begin
		if(load_sw == 1'b1)
			count <= sw;
		else begin
			if(clk_en)
				count <= count - 1;
		end
	end
endmodule

module clk_divider(
	input clk,// 100 MHz
	output down_counter_enable,// Enable the down counter once per second
	output led_driver_enable,// When lower 17 bits are zero (763 Hz)
	output debouncer_enable// When lower 22 bits are zero (24 Hz)
	);

	parameter [26:0] MAX_COUNT = 100000000;
	reg [26:0] counter;// 27 bits needed for 100 million
	assign down_counter_enable = (counter == 27'b0) ? 1'b1 : 1'b0;
	assign led_driver_enable = (counter[16:0] == 17'b0) ? 1'b1 : 1'b0;
	assign debouncer_enable = (counter[21:0] == 22'b0) ? 1'b1 : 1'b0;

	always @ (posedge clk) begin
		if(counter == MAX_COUNT) begin
			counter <= 27'b0;
		end
		else begin
		  counter <= counter + 1;
		end
	end
endmodule

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

// Flash the LEDs (not 7-segment) when timer reaches zero
module led_flasher(
	input clk,
	input clk_en,
	input start,// This signal is controlled by the main timing so 100 MHz clock
	output reg [15:0] leds
	);

	reg [7:0] count;

	always @ (posedge clk) begin
		if(start) begin
			count <= 8'b0;
			leds <= 16'b0000111100001111;// Pattern that gets shifted 64 times to look coool
		end
		if(clk_en && !(count == 8'd64)) begin
			leds <= {leds[0], leds[15:1]};// Shift
			count <= count + 8'b1;
		end
	end
endmodule

// 7-segment LED multiplexer
// Help from https://www.fpga4student.com/2017/09/seven-segment-led-display-controller-basys3-fpga.html
// CHECKED
module numeric_leds_multiplexer(
	input clk,
	input clk_en,
	input [3:0] hundreds,
	input [3:0] tens,
	input [3:0] units,
	output reg [3:0] anodes,// 3 digits, 0=units, 1=tens, 2=hundreds
	output reg [6:0] cathodes// 7 segments per digit
	);

	reg [1:0] digit_activation;// 0, 1, 2
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
			default: cathodes <= 7'b0111111; // "-"
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
			default begin
		        anodes = 4'b1111;
				current_displayed_bcd = 4'b0;
			end
		endcase
	end

	// Increment digit counter
	always @ (posedge clk) begin
		if(clk_en) begin
			digit_activation <= digit_activation + 2'b1;
		end
		if(digit_activation == 2'b11) begin
			digit_activation <= 2'b0;
		end
	end
endmodule

// Uses double dabble algorithm: https://en.wikipedia.org/wiki/Double_dabble
module byte_to_bcd(
	input clk,
	input start_conversion,
	input [7:0] binary_in,
	output reg [3:0] hundreds,
	output reg [3:0] tens,
	output reg [3:0] units
	);
	
	reg [7:0] original_byte;
	reg [3:0] cycles;
	reg [1:0] curr_state;
	reg [1:0] next_state;
	
	// State machine
	// For each cycle: Add, then Shift. So increment `cycles` on add then check on shift if done
	localparam STATE_IDLE = 2'b0;
	localparam STATE_SHIFT = 2'b1;
	localparam STATE_ADD = 2'b10;

	// Synchronous state update
	always @ (posedge clk) begin
	   curr_state <= next_state;
    end
    
    // State update
    always @ (posedge clk) begin
        case(curr_state)
            STATE_IDLE: begin
                if(start_conversion) begin
                    // Init stuff
                    cycles <= 4'b0;
                end
            end
            STATE_SHIFT: begin
                if(cycles == 4'd8) begin
                    next_state <= STATE_IDLE;
                end
                else begin
                    next_state <= STATE_ADD;
                end
            end
            STATE_ADD: begin
                next_state <= STATE_SHIFT;
            end
        endcase
    end
    
    // Update data based on current state
    always @ (posedge clk) begin
        if(curr_state == STATE_ADD) begin
            // As far as I can tell from wiki, the carry is ignored
            if(units > 4'd4) begin
                units <= units + 4'd3;
            end
            if(tens > 4'd4) begin
                tens <= tens + 4'd3;
            end
            if(hundreds > 4'd4) begin
                hundreds <= hundreds + 4'd3;
            end
        end
        if(curr_state == STATE_SHIFT) begin
            original_byte <= {original_byte[6:0], 1'b0};
            units <= {units[2:0], original_byte[7:7]};
            tens <= {tens[2:0], units[3:3]};
            hundreds <= {hundreds[2:0], tens[3:3]};
        end
        else begin
            original_byte <= binary_in;
        end
    end
endmodule

module central_timing(
	input btn_edge,// Right button
	input clk,// 100 MHz
	input down_counter_done,
	input reset,// Active high
	output load_down_counter,
	output down_counter_enable,
	output [1:0] state_debug
	);
	
	localparam STATE_WAITING = 2'B0;
	localparam STATE_LOADED = 2'B1;
	localparam STATE_COUNTING = 2'B10;
	localparam STATE_PAUSED = 2'B11;
	
	// 0 -> Waiting for user to enter time with switches
	// 1 -> User has has pushed button to enter time, waiting for start
	// 2 -> Counting down, will go back to state 0 when done
	// 3 -> Paused
	reg [1:0] curr_state;
	reg [1:0] next_state;
	assign state_debug = curr_state;
	
	// Synchronous state update
	always @ (posedge clk) begin
	   if(reset == 1'b1) begin
	       curr_state <= 2'b0;
	   end
	   else begin
	       curr_state <= next_state;
	   end
    end
    
    // Next state
    always @ (posedge clk) begin
        case(curr_state)
            STATE_WAITING: begin
                if(btn_edge) begin
                    next_state <= STATE_LOADED;
                end
                else begin
                    next_state <= STATE_WAITING;
                end
            end
            STATE_LOADED: begin
                if(btn_edge) begin
                    next_state <= STATE_COUNTING;
                end
                else begin
                    next_state <= STATE_LOADED;
                end
            end
            STATE_COUNTING: begin
                if(btn_edge) begin
                    next_state <= STATE_PAUSED;
                end
                if(down_counter_done) begin
                    next_state <= STATE_WAITING;
                end
                else begin
                    next_state <= STATE_COUNTING;
                end
            end
            STATE_PAUSED: begin
                if(btn_edge) begin
                    next_state <= STATE_COUNTING;
                end
                else begin
                    next_state <= STATE_PAUSED;
                end
            end
        endcase
    end
    
    /*always @ (*) begin
        case(curr_state)
            STATE_WAITING: begin
                if(btn_edge) begin
                    curr_state = STATE_LOADED;
                end
            end
            STATE_LOADED: begin
                if(btn_edge) begin
                    curr_state = STATE_COUNTING;
                end
            end
            STATE_COUNTING: begin
                if(btn_edge) begin
                    curr_state = STATE_PAUSED;
                end
                if(down_counter_done) begin
                    curr_state = STATE_WAITING;
                end
            end
            STATE_PAUSED: begin
                if(btn_edge) begin
                    curr_state = STATE_COUNTING;
                end
            end
        endcase
    end*/
    
    // Combinational output from  state
    assign down_counter_enable = (curr_state == STATE_COUNTING)? 1'b1 : 1'b0;
endmodule

module top_microwave_timer(
	input [7:0] sw,// Switches
	input btnR,// Right button
	input clk,// 100 MHz
	output [15:0] led,
	output [3:0] an,// 7 segment anodes
	output [6:0] seg// 7 segment cathodes
	);
	
	// Internal wires
	wire down_counter_clk_enable;// Enable the down counter once per second
	wire led_driver_clk_enable;// When lower 17 bits are zero (763 Hz)
	wire debouncer_clk_enable;
	wire load_sw;
	wire counter_done;
	wire [7:0] counter_value;
	wire btn_edge;
	wire [3:0] bcd_units;
	wire [3:0] bcd_tens;
	wire [3:0] bcd_hundreds;
	wire down_counter_enable;
	wire down_counter_enable_gated;
	//assign down_counter_clk_enable_gated = down_counter_enable & down_counter_clk_enable;
	
	/* LED Debugging:
	2:3 - central timing state
	7:14 - down counter value
	*/

	wire [1:0] central_timing_state;
	assign led[1:0] = 2'b0;
	assign led[3:2] = central_timing_state[1:0];
	assign led[14:7] = counter_value[7:0];
	assign led[15] = 1'b0;
	assign down_counter_clk_enable_gated = down_counter_clk_enable;
	
	// Instanciate submodules
	clk_divider clk_divider_instance(
		.clk(clk),
		.down_counter_enable(down_counter_clk_enable),
		.led_driver_enable(led_driver_clk_enable),
		.debouncer_enable(debouncer_clk_enable)
	);
	
	assign down_counter_enable_gated = down_counter_enable & down_counter_clk_enable;
	down_counter down_counter_instance(
		.sw(sw),
		.load_sw(load_sw),// Active high, read on +edge
		.clk_en(down_counter_clk_enable_gated),// Active high
		.clk(clk),
		.done(counter_done),
		.count(counter_value)
	);
	
	debouncer debouncer_instance(
		.clk(clk),
		.clk_en(debouncer_clk_enable),
		.button_in(btnR),
		.button_edge(btn_edge)
	);
	
	/*led_flasher led_flasher_instance(
		.clk(clk),
		.clk_en(led_driver_clk_enable),
		.start(counter_done),
		.leds(led)
	);*/
	
	numeric_leds_multiplexer numeric_leds_multiplexer_instance(
		.clk(clk),
		.clk_en(led_driver_clk_enable),//.clk_en(led_driver_clk_enable),
		.hundreds(bcd_hundreds),
		.tens(bcd_tens),
		.units(bcd_units),
		.anodes(an),// 3 digits, 0=units, 1=tens, 2=hundreds
		.cathodes(seg)// 7 segments per digit
	);
	
	byte_to_bcd byte_to_bcd_instance(
		.clk(clk),
		.start_conversion(btn_edge),//.start_conversion(down_counter_clk_enable),
		.binary_in(sw[7:0]),//.binary_in(counter_value),
		.hundreds(bcd_hundreds),
		.tens(bcd_tens),
		.units(bcd_units)
	);
	
	central_timing central_timing_instance(
		.btn_edge(btn_edge),// Right button
		.clk(clk),// 100 MHz
		.down_counter_done(counter_done),
		.load_down_counter(load_sw),
		.down_counter_enable(down_counter_enable),
		.reset(1'b0),// TODO
		.state_debug(central_timing_state)
	);
	
endmodule