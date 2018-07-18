// Locals
mod utils;
// Externs
extern crate rand;
extern crate minifb;

// Standard Library
use std::env;
use std::thread;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::prelude::*;
use std::time::{Instant, Duration};

// Local
use utils::*;

// Externs
use rand::prelude::*;
use minifb::{Key, KeyRepeat, WindowOptions, Window, Scale};

// Constants
const WIDTH: usize = 64;
const HEIGHT: usize = 32;
const SCALE: usize = 2;
const COLOR: u32 = 65280;
const SPRITES: [[u8; 5]; 16] = [
    [0xF0, 0x90, 0x90, 0x90, 0xF0], // 0
    [0x20, 0x60, 0x20, 0x20, 0x70], // 1
    [0xF0, 0x10, 0xF0, 0x80, 0xF0], // 2
    [0xF0, 0x10, 0xF0, 0x10, 0xF0], // 3
    [0x90, 0x90, 0xF0, 0x10, 0x10], // 4
    [0xF0, 0x80, 0xF0, 0x10, 0xF0], // 5
    [0xF0, 0x80, 0xF0, 0x90, 0xF0], // 6
    [0xF0, 0x10, 0x20, 0x40, 0x40], // 7
    [0xF0, 0x90, 0xF0, 0x90, 0xF0], // 8
    [0xF0, 0x90, 0xF0, 0x10, 0xF0], // 9
    [0xF0, 0x90, 0xF0, 0x90, 0x90], // A
    [0xE0, 0x90, 0xE0, 0x90, 0xE0], // B
    [0xF0, 0x80, 0x80, 0x80, 0xF0], // C
    [0xE0, 0x90, 0x90, 0x90, 0xE0], // D
    [0xF0, 0x80, 0xF0, 0x80, 0xF0], // E
    [0xF0, 0x80, 0xF0, 0x80, 0x80], // F
];

fn main() {
    let mut file = File::open("./static/BRIX").expect("file not found");
    let mut buffer: Vec<u8> = Vec::new();
    file.read_to_end(&mut buffer).unwrap();

    /*let mut idx = 0;
    loop {
        if idx >= buffer.len() {
            break;
        }
        let op: u16 = ((buffer[idx] as u16) << 8) + buffer[(idx + 1)] as u16;
        println!("{}: {:04X}", idx, op);

        idx = idx + 2;
    }*/

    let mut chip = Chip::new(buffer);

    let mut buffer: Vec<u32> = vec![0; WIDTH * SCALE * HEIGHT * SCALE];

    let mut window_options = WindowOptions::default();
    window_options.scale = Scale::X4;
    let mut window = Window::new("Chip8",
                                WIDTH * SCALE,
                                HEIGHT * SCALE,
                                window_options).unwrap_or_else(|e| {
        panic!("{}", e);
    });

    let mut prev_pressed_keys = BTreeSet::new();
    let mut prev = Instant::now();

    while window.is_open() {
        let mut current_pressed_keys = BTreeSet::new();
        window.get_keys()
            .unwrap().iter()
            .for_each(|k| { current_pressed_keys.insert(get_key_value(k)); });

        let ppkc = prev_pressed_keys.clone();
        let mut pressed_keys = ppkc.difference(&current_pressed_keys);
        let selected_key = pressed_keys.nth(0).map(|k| *k);
        if selected_key.is_some() {
            prev_pressed_keys.clear();
        } else {
            prev_pressed_keys = current_pressed_keys.clone();
        }

        let now = Instant::now();
        let elapsed = now.duration_since(prev);

        // Meh
        if elapsed > Duration::from_micros(16666) {
            prev = now;
            chip.decrement_delay();
        }

        chip.step(&selected_key, &current_pressed_keys);

        chip.render_to_window(&mut buffer, &mut window);
    }
}

struct Chip {
    memory: [u8; 4096],
    registers: [u8; 16],
    screen: [bool; WIDTH * HEIGHT],
    i: u16,
    delay: u8,
    sound: u8,
    pc: u16,
    sp: u8,
    stack: [u16; 16]
}

impl Chip {
    pub fn new(program: Vec<u8>) -> Self {
        //assert!(program.len() % 2 == 0);
        let mut memory = [0; 4096];

        for (sprite_idx, sprite) in SPRITES.iter().enumerate() {
            for (index, byte) in sprite.iter().enumerate() {
                memory[5 * sprite_idx + index] = *byte;
            }
        }

        for i in 0..program.len() {
            memory[i + 0x200] = program[i]
        }

        Chip {
            memory: memory,
            registers: [0; 16],
            screen: [false; WIDTH * HEIGHT],
            i: 0,
            delay: 0,
            sound: 0,
            pc: 0x200,
            sp: 0,
            stack: [0; 16]
        }
    }

    pub fn decrement_delay(&mut self) {
        if self.delay > 0 {
            self.delay = self.delay - 1;
        }
    }

    #[inline(always)]
    fn increment_pc(&mut self) {
        self.pc = self.pc + 2;
    }

    #[inline(always)]
    pub fn get_x_reg_value(&self, op: u16) -> u8 {
        self.registers[get_second_nibble(op) as usize]
    }

    #[inline(always)]
    pub fn get_y_reg_value(&self, op: u16) -> u8 {
        self.registers[get_third_nibble(op) as usize]
    }

    #[inline(always)]
    pub fn set_x_reg_value(&mut self, op: u16, val: u8) {
        let reg = get_second_nibble(op) as usize;
        self.registers[reg] = val;
    }

    #[inline(always)]
    pub fn render_to_window(&self, buffer: &mut Vec<u32>, window: &mut Window) {
        // This was a toughy to write..
        for (original_index, is_on) in self.screen.iter().enumerate() {
            // Get the column the unscaled pixel would be on
            let initial_height = original_index / WIDTH;
            // Calculate index in the scaled buffer where we should draw
            let scaled_idx = initial_height * WIDTH * SCALE * SCALE;

            for outer in 0..SCALE {
                for inner in 0..SCALE {
                    // Calculate offset for Y scale
                    let mut loc = scaled_idx + (inner * WIDTH * SCALE);
                    // Calculate offset for X scale
                    loc = ((original_index % WIDTH) * SCALE) + loc + outer;
                    // Render
                    buffer[loc] = if *is_on { COLOR } else { 0 };
                }
            }
        }
        window.update_with_buffer(&buffer).unwrap();
    }

    pub fn step(&mut self, key: &Option<u16>, keys: &BTreeSet<u16>) {
        let pc = self.pc as usize;
        let op: u16 = ((self.memory[pc] as u16) << 8) + self.memory[(pc + 1)] as u16;

        match get_first_nibble(op) {
            0x0 => {
                match get_last_byte(op) {
                    0xE0 => {
                        // Clear screen
                        for pixel in self.screen.iter_mut() {
                            *pixel = false;
                        }

                        self.increment_pc();
                    },
                    0xEE => {
                        // Return
                        self.pc = self.stack[self.sp as usize];
                        self.stack[self.sp as usize] = 0;
                        self.sp = self.sp - 1;
                    },
                    _ => invalid_instruction(op)
                }
            },
            0x1 => {
                // Jump to location nnn. 
                self.pc = get_addr(op);
            },
            0x2 => {
                // Call subroutine
                let next_addr = get_addr(op);
                self.sp = self.sp + 1;
                self.stack[self.sp as usize] = self.pc + 2;

                self.pc = next_addr;
            },
            0x3 => {
                // Jmp if Eq
                if self.get_x_reg_value(op) == get_last_byte(op) {
                    self.increment_pc();
                }

                self.increment_pc();
            },
            0x4 => {
                // Jmp if NEq
                if self.get_x_reg_value(op) != get_last_byte(op) {
                    self.increment_pc();
                }

                self.increment_pc();
            },
            0x5 => {
                // Jump if reg x == reg y
                if self.get_x_reg_value(op) == self.get_y_reg_value(op) {
                    self.increment_pc();
                }

                self.increment_pc();
            },
            0x6 => {
                // The interpreter puts the value kk into register Vx. 
                let val = get_last_byte(op);
                self.set_x_reg_value(op, val);

                self.increment_pc();
            },
            0x7 => {
                //Adds the value kk to the value of register Vx, then stores the result in Vx. 
                let result = self.get_x_reg_value(op) as u16 + get_last_byte(op) as u16;
                self.set_x_reg_value(op, result as u8);

                self.increment_pc();
            },
            0x8 => {
                match get_last_nibble(op) {
                    0x0 => {
                        // Stores the value of register Vy in register Vx.
                        let y = self.get_y_reg_value(op);
                        self.set_x_reg_value(op, y);

                        self.increment_pc();
                    },
                    0x2 => {
                        // X & Y stores in X
                        let x = self.get_x_reg_value(op);
                        let y = self.get_y_reg_value(op);
                        self.set_x_reg_value(op, x & y);

                        self.increment_pc();
                    },
                    0x3 => {
                        // X XOR Y stores in X
                        let x = self.get_x_reg_value(op);
                        let y = self.get_y_reg_value(op);
                        self.set_x_reg_value(op, x ^ y);

                        self.increment_pc();
                    },
                    0x4 => {
                        let x = self.get_x_reg_value(op);
                        let y = self.get_y_reg_value(op);

                        let sum = x as u16 + y as u16;
                        if sum > 255 {
                            self.registers[0xF] = 1; 
                        }
                        self.set_x_reg_value(op, sum as u8);

                        self.increment_pc();
                    },
                    0x5 => {
                        let x = self.registers[get_second_nibble(op) as usize];
                        let y = self.registers[get_third_nibble(op) as usize];
                        if x > y {
                            self.registers[0xF] = 1;
                            // ?? Moved here from below
                            self.registers[get_second_nibble(op) as usize] = x - y;
                        } else {
                            self.registers[0xF] = 0;
                        }

                        self.increment_pc();
                    },
                    _ => invalid_instruction(op)
                }
            },
            0xA => {
                // The value of register I is set to nnn.
                self.i = get_addr(op);
                self.increment_pc();
            },
            0xC => {
                // The interpreter generates a random number from 0 to 255, 
                // then ANDed with the value kk. The results are stored in Vx. 
                let rand: u8 = random();
                let kk = get_last_byte(op);
                let val = rand & kk;

                let reg = get_second_nibble(op) as usize;
                self.registers[reg] = val;
                self.increment_pc();
            },
            0xD => {
                let i = self.i as usize;
                let n = get_last_nibble(op) as usize;
                let x = self.registers[get_second_nibble(op) as usize] as usize;
                let y = self.registers[get_third_nibble(op) as usize] as usize;

                for count in 0..n {
                    let byte = self.memory[i + count];
                    for byte_idx in 0..8 {
                        let y_offset = if y + count >= HEIGHT {
                            (y + count) * WIDTH % (WIDTH * HEIGHT)
                            //((y + count) - 32) * WIDTH 
                        } else {
                            // Prev working
                            (y + count) * WIDTH
                        };
                        let x_offset = if x + byte_idx >= WIDTH {
                            (x + byte_idx) - 64
                        } else {
                            // Prev working
                            x + byte_idx
                        };
                        let loc = y_offset + x_offset;
                        let next = self.screen[loc] ^ ((byte >> (7 - byte_idx) & 1) == 1);
                        if self.screen[loc] && !next {
                            self.registers[0xF] = 1;
                        }
                        self.screen[loc] = next; 
                    }
                }

                self.increment_pc();
            },
            0xE => {
                match get_last_byte(op) {
                    0x9E => {
                        if keys.contains(&(self.registers[get_second_nibble(op) as usize] as u16)) {
                            self.increment_pc();
                        }
                    },
                    0xA1 => {
                        if !keys.contains(&(self.registers[get_second_nibble(op) as usize] as u16)) {
                            self.increment_pc();
                        }
                    },
                    _ => panic!("Bad op code {:X}", op)
                }

                self.increment_pc();
            },
            0xF => {
                match get_last_byte(op) {
                    0x0A => {
                        if key.is_some() {
                            let key = key.unwrap();
                            self.registers[get_second_nibble(op) as usize] = key as u8;

                            self.increment_pc();
                        }
                    },
                    0x07 => {
                        self.registers[get_second_nibble(op) as usize] = self.delay;

                        self.increment_pc();
                    },
                    0x15 => {
                        self.delay = self.registers[get_second_nibble(op) as usize];

                        self.increment_pc();
                    },
                    0x18 => {
                        self.sound = self.registers[get_second_nibble(op) as usize];

                        self.increment_pc();
                    },
                    0x1E => {
                        let reg = get_second_nibble(op) as usize;
                        let reg_value = self.registers[reg];
                        self.i = self.i + reg_value as u16;

                        self.increment_pc();
                    },
                    0x29 => {
                        self.i = self.registers[get_second_nibble(op) as usize] as u16 * 5;

                        self.increment_pc();
                    }
                    0x33 => {
                        let value = self.registers[get_second_nibble(op) as usize];
                        // There has to be another way to do this lol
                        self.memory[self.i as usize] = value / 100;
                        self.memory[self.i as usize + 1] = (value % 100) / 10;
                        self.memory[self.i as usize + 2] = value % 100 % 10;

                        self.increment_pc();
                    },
                    0x55 => {
                        let x = get_second_nibble(op);
                        for idx in 0..=x {
                            self.memory[(self.i + idx as u16) as usize] = self.registers[idx as usize];
                        }

                        self.increment_pc();
                    },
                    0x65 => {
                        let x = get_second_nibble(op);
                        for idx in 0..=x {
                            self.registers[idx as usize] = self.memory[(self.i + idx as u16) as usize];
                        }

                        self.increment_pc();
                    },
                    _ => panic!(format!("Unsupported 0xF instruction {:X}", op))
                }
            },
            _ => panic!(format!("Unsupported instruction {:X}", op))
        }
    }
}