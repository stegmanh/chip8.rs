extern crate rand;
extern crate minifb;

// std
use std::env;
use std::thread;
use std::collections::HashSet;
use std::fs::File;
use std::io::prelude::*;

// externs
use rand::prelude::*;
use minifb::{Key, WindowOptions, Window};

// Consts
const WIDTH: usize = 64;
const HEIGHT: usize = 32;
const SCALE: usize = 2;
const COLOR: u32 = 65280;

// I cant believe this
fn get_key_value(key: Key) -> u16 {
    match key {
        Key::Key0 => 0,
        Key::Key1 => 1,
        Key::Key2 => 2,
        Key::Key3 => 3,
        Key::Key4 => 4,
        Key::Key5 => 5,
        Key::Key6 => 6,
        Key::Key7 => 7,
        Key::Key8 => 8,
        Key::Key9 => 9,
        Key::A => 10,
        Key::B => 11,
        Key::C => 12,
        Key::D => 13,
        Key::E => 14,
        Key::F => 15,
        _ => 255
    }
}

fn main() {
    let mut file = File::open("./static/GUESS").expect("file not found");
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
    let mut window = Window::new("Test - ESC to exit",
                                WIDTH * SCALE,
                                HEIGHT * SCALE,
                                WindowOptions::default()).unwrap_or_else(|e| {
        panic!("{}", e);
    });

    while window.is_open() {
        let keys = window.get_keys();
        let mut selected_key = None;

        if keys.is_some() {
            let keys = keys.unwrap();
            for key in keys.iter() {
                if get_key_value(key.clone()) < 16 {
                    selected_key = Some(get_key_value(key.clone()));
                    break;
                }
            }
        }

        chip.step(&selected_key);

        //window.update_with_buffer(&buffer).unwrap();
        chip.render_to_window(&mut buffer, &mut window);

        //thread::sleep_ms(1);
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
        assert!(program.len() % 2 == 0);
        let mut memory = [0; 4096];

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

    pub fn step(&mut self, key: &Option<u16>) {
        let pc = self.pc as usize;
        let op: u16 = ((self.memory[pc] as u16) << 8) + self.memory[(pc + 1)] as u16;

        match get_first_nibble(op) {
            0x0 => {
                if get_last_byte(op) == 0xE0 {
                    // Clear screen
                    for pixel in self.screen.iter_mut() {
                        *pixel = false;
                    }

                    self.pc = self.pc + 2;
                } else if get_last_byte(op) == 0xEE {
                    // Return -- yay!
                    self.pc = self.stack[self.sp as usize];
                    self.stack[self.sp as usize] = 0;
                    self.sp = self.sp - 1;
                } else {
                    panic!("Hmm");
                }
            },
            0x1 => {
                // Jump to location nnn. 
                self.pc = get_addr(op);
            },
            0x2 => {
                let next_addr = get_addr(op);
                self.sp = self.sp + 1;
                self.stack[self.sp as usize] = self.pc + 2;

                self.pc = next_addr;
            },
            0x3 => {
                let reg = get_second_nibble(op) as usize;
                let reg_value = self.registers[reg];
                if reg_value == get_last_byte(op) {
                    self.pc = self.pc + 2;
                }

                self.pc = self.pc + 2;
            },
            0x4 => {
                let reg = get_second_nibble(op) as usize;
                let reg_value = self.registers[reg];
                if reg_value != get_last_byte(op) {
                    self.pc = self.pc + 2;
                }

                self.pc = self.pc + 2;
            },
            0x6 => {
                // The interpreter puts the value kk into register Vx. 
                let reg = get_second_nibble(op) as usize;
                let val = get_last_byte(op);
                self.registers[reg] = val;

                self.pc = self.pc + 2;
            },
            0x7 => {
                //Adds the value kk to the value of register Vx, then stores the result in Vx. 
                let reg = get_second_nibble(op) as usize;
                let add = get_last_byte(op);
                self.registers[reg] = self.registers[reg] + add;

                self.pc = self.pc + 2;
            },
            0x8 => {
                match get_last_nibble(op) {
                    0 => {
                        //Stores the value of register Vy in register Vx.
                        self.registers[get_second_nibble(op) as usize] = self.registers[get_third_nibble(op) as usize];

                        self.pc = self.pc + 2;
                    },
                    2 => {
                        let x = self.registers[get_second_nibble(op) as usize];
                        let y = self.registers[get_third_nibble(op) as usize];

                        self.registers[get_second_nibble(op) as usize] = x & y;

                        self.pc = self.pc + 2;
                    },
                    4 => {
                        let x = self.registers[get_second_nibble(op) as usize];
                        let y = self.registers[get_third_nibble(op) as usize];
                        let sum = x as u16 + y as u16;
                        if sum > 255 {
                            self.registers[0xF] = 1; 
                        }
                        
                        self.registers[get_second_nibble(op) as usize] = sum as u8;

                        self.pc = self.pc + 2;
                    },
                    _ => panic!("Unsupported 8 {:X}", op)
                }
            },
            0xA => {
                // The value of register I is set to nnn.
                self.i = get_addr(op);
                self.pc = self.pc + 2;
            },
            0xC => {
                // The interpreter generates a random number from 0 to 255, 
                // then ANDed with the value kk. The results are stored in Vx. 
                let rand: u8 = random();
                let kk = get_last_byte(op);
                let val = rand & kk;

                let reg = get_second_nibble(op) as usize;
                self.registers[reg] = val;
                self.pc = self.pc + 2;
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

                self.pc = self.pc + 2;
            },
            0xF => {
                match get_last_byte(op) {
                    0x0A => {
                        if key.is_some() {
                            let key = key.unwrap();
                            self.registers[get_second_nibble(op) as usize] = key as u8;
                            println!("{}", key as u8);

                            self.pc = self.pc + 2;
                        }
                    },
                    0x1E => {
                        let reg = get_second_nibble(op) as usize;
                        let reg_value = self.registers[reg];
                        self.i = self.i + reg_value as u16;

                        self.pc = self.pc + 2;
                    },
                    0x33 => {
                        let value = self.registers[get_second_nibble(op) as usize];
                        // There has to be another way to do this lol
                        self.memory[self.i as usize] = value / 100;
                        self.memory[self.i as usize + 1] = (value % 100) / 10;
                        self.memory[self.i as usize + 2] = value % 100 % 10;

                        self.pc = self.pc + 2;
                    },
                    0x65 => {
                        let x = get_second_nibble(op);
                        for idx in 0..=x {
                            self.registers[idx as usize] = self.memory[(self.i + idx as u16) as usize];
                        }

                        self.pc = self.pc + 2;
                    },
                    _ => panic!(format!("Unsupported 0xF instruction {:X}", op))
                }
            },
            _ => panic!(format!("Unsupported instruction {:X}", op))
        }
    }
}

#[inline(always)]
fn get_first_nibble(num: u16) -> u8 {
    ((num & 0xF000) >> 12) as u8
}

#[inline(always)]
fn get_second_nibble(num: u16) -> u8 {
    ((num & 0x0F00) >> 8) as u8
}

#[inline(always)]
fn get_third_nibble(num: u16) -> u8 {
    ((num & 0x00F0) >> 4) as u8
}

#[inline(always)]
fn get_last_nibble(num: u16) -> u8 {
    (num & 0x000F) as u8
}

#[inline(always)]
fn get_addr(num: u16) -> u16 {
    num & 0x0FFF
}

#[inline(always)]
fn get_last_byte(num: u16) -> u8 {
    (num & 0x00FF) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_addr() {
        assert_eq!(get_addr(0xABCD), 0xBCD);
    }

    #[test]
    fn test_get_last_nibble() {
        assert_eq!(get_last_nibble(0xABCD), 0xD);
    }

    #[test]
    fn test_get_third_nibble() {
        assert_eq!(get_third_nibble(0xABCD), 0xC);
    }

    #[test]
    fn test_get_second_nibble() {
        assert_eq!(get_second_nibble(0xABCD), 0xB);
    }

    #[test]
    fn test_get_first_nibble() {
        assert_eq!(get_first_nibble(0xABCD), 0xA);
    }

    #[test]
    fn test_get_last_byte() {
        assert_eq!(get_last_byte(0xABCD), 0xCD);
    }
}