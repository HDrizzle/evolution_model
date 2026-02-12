`timescale 1ns / 1ps
//////////////////////////////////////////////////////////////////////////////////
// Company: 
// Engineer: 
// 
// Create Date: 02/10/2026 02:57:40 PM
// Design Name: 
// Module Name: test_state_machine
// Project Name: 
// Target Devices: 
// Tool Versions: 
// Description: 
// 
// Dependencies: 
// 
// Revision:
// Revision 0.01 - File Created
// Additional Comments:
// scp "C:/Users/hfward/lab_2_calc/lab_2_calc.srcs/sources_1/new/test_state_machine.v" hward@142.0.97.104:/home/hward/rust/evolution_model/resources/http
//////////////////////////////////////////////////////////////////////////////////


module test_state_machine();
    reg clk;
    reg reset;
    reg btn_edge_l;
    reg btn_edge_r;
    wire load_a;
    wire load_b;
    wire load_op;
    wire [1:0] state_debug;
    
    always begin
        #5 clk <= ~clk;
    end
    
    initial begin
        // Initialize everything
        clk <= 1'b1;
        reset <= 1'b1;
        btn_edge_l <= 1'b0;
        btn_edge_r <= 1'b0;
        // Wait for rising then falling edge, reset low
        #15 reset <= 1'b0;
        // Set right button high for 1 cycle
        btn_edge_r <= 1'b1;
        #10 btn_edge_r <= 1'b0;
        #20;
        // Again to load 2nd operand
        btn_edge_r <= 1'b1;
        #10 btn_edge_r <= 1'b0;
        #20;
        // Again to load operation and done
        btn_edge_r <= 1'b1;
        #10 btn_edge_r <= 1'b0;
        #20;
        // Reset
        btn_edge_l <= 1'b1;
        #10 btn_edge_l <= 1'b0;
        #20;
        $stop;
    end

    uut central_timing(
        .btn_edge_l(btn_edge_l),
        .btn_edge_r(btn_edge_r),
        .clk(clk),
        .reset(reset),
        .load_a(load_a),
        .load_b(load_b),
        .load_op(load_op),
        .state_debug(state_debug)
    );
endmodule
