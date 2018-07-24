// Locals
mod utils;
// Externs
extern crate rand;
extern crate minifb;
extern crate rodio;

// Standard Library
use std::env;
use std::thread;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::prelude::*;
use std::time::{Instant, Duration};
use std::sync::mpsc::channel;

// Local
use utils::*;

// Externs
use rand::prelude::*;
use minifb::{Key, KeyRepeat, WindowOptions, Window, Scale};
use rodio::Sink;

// Constants
const WIDTH: usize = 64;
const HEIGHT: usize = 32;
const SCALE: usize = 2;
const COLOR: u32 = 65280;
const STATIC_SPRITES: [[u8; 5]; 16] = [
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

static HELP: &'static str = "usage: rust8 [FILE]...";



fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        return println!("{}", HELP);
    }

    let mut file = match File::open(format!("{}", args[1])) {
        Ok(file) => file,
        Err(e) => {
            return println!("Error opening file:\n{:?}", e.kind());
        }
    };

    let mut buffer: Vec<u8> = Vec::new();
    if let Err(e) = file.read_to_end(&mut buffer) {
        return println!("Error reading file:\n{:?}", e.kind());
    };

    let mut chip = match Chip::try_new(buffer) {
        Ok(c) => c,
        Err(e) => {
            return println!("Failed to create chip:\n{:?}", e);
        }
    };

    // Create screen to render too
    let mut screen_buffer: Vec<u32> = vec![0; WIDTH * SCALE * HEIGHT * SCALE];
    let mut window_options = WindowOptions::default();
    window_options.scale = Scale::X4;
    let mut window = match Window::new("Chip8", WIDTH * SCALE, HEIGHT * SCALE, window_options) {
        Ok(w) => w,
        Err(e) => {
            return println!("Error creating Chip8 graphical window:\n{:?}", e);
        }
    };

    let mut prev_pressed_keys = BTreeSet::new();
    let mut prev = Instant::now();

    // Sound handler thread
    let (tx, rx) = channel();
    thread::spawn(move || {
        // Sound set up https://docs.rs/rodio/0.8.0/rodio
        let device = match rodio::default_output_device() {
            Some(d) => d,
            None => {
                return println!("Error binding to sound output device:\nNon Found")
            }
        };
        let mut sink = Sink::new(&device);
        sink.set_volume(0.10);

        // Create source
        let source = rodio::source::SineWave::new(250);
        sink.append(source);

        // WHEW - Alrighty then
        // So this basically just loops in a seprate thread constantly trying to read if it 
        // should play or pause
        loop {
            match rx.recv() {
                Ok(should_play) => {
                    if should_play && sink.is_paused() {
                        sink.play();
                    } else if !should_play {
                        sink.pause();
                    }
                },
                Err(e) => {}
            };
        }
    });

    while window.is_open() {
        // A series of hacks to determine if it's a keypress or if it's a key hold(en) ha
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
            // Ignoring result here because it doesnt _really_ matter if send fails
            tx.send(chip.sound > 0);
            chip.decrement_delay();
            chip.decrement_sound();
        }

        chip.step(&selected_key, &current_pressed_keys);

        chip.render_to_window(&mut screen_buffer, &mut window);
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
    pub fn try_new(program: Vec<u8>) -> Result<Self, String> {
        if program.len() > (4096 - 0x200) {
            return Err(format!("Game program too large. Got size: {}", program.len()));
        }

        // Create memory
        let mut memory = [0; 4096];

        // Load sprites
        for (sprite_idx, sprite) in STATIC_SPRITES.iter().enumerate() {
            for (byte_index, byte) in sprite.iter().enumerate() {
                memory[5 * sprite_idx + byte_index] = *byte;
            }
        }

        // Load program into memory
        for i in 0..program.len() {
            memory[i + 0x200] = program[i]
        }

        Ok(Chip {
            memory: memory,
            registers: [0; 16],
            screen: [false; WIDTH * HEIGHT],
            i: 0,
            delay: 0,
            sound: 0,
            pc: 0x200,
            sp: 0,
            stack: [0; 16]
        })
    }

    #[inline(always)]
    pub fn decrement_delay(&mut self) {
        if self.delay > 0 {
            self.delay = self.delay - 1;
        }
    }

    #[inline(always)]
    pub fn decrement_sound(&mut self) {
        if self.sound > 0 {
            self.sound = self.sound - 1;
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
                let result = self.get_x_reg_value(op).wrapping_add(get_last_byte(op));
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
                        let x = self.get_x_reg_value(op) as u16;
                        let y = self.get_y_reg_value(op) as u16;

                        let sum = x + y;
                        if sum > 255 {
                            self.registers[0xF] = 1; 
                        } else {
                            self.registers[0xF] = 0; 
                        }

                        self.set_x_reg_value(op, sum as u8);

                        self.increment_pc();
                    },
                    0x5 => {
                        let x = self.get_x_reg_value(op);
                        let y = self.get_y_reg_value(op);

                        if x > y {
                            self.registers[0xF] = 1;
                        } else {
                            self.registers[0xF] = 0;
                        }

                        self.set_x_reg_value(op, x.wrapping_sub(y));

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
                let val = rand & get_last_byte(op);

                self.set_x_reg_value(op, val);
                self.increment_pc();
            },
            0xD => {
                let i = self.i as usize;
                let n = get_last_nibble(op) as usize;
                let x = self.get_x_reg_value(op) as usize;
                let y = self.get_y_reg_value(op) as usize;

                let mut should_set = false;
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
                            should_set = true;
                        }
                        self.screen[loc] = next; 
                    }
                }

                if should_set {
                    self.registers[0xF] = 1;
                } else {
                    self.registers[0xF] = 0;
                }

                self.increment_pc();
            },
            0xE => {
                match get_last_byte(op) {
                    0x9E => {
                        if keys.contains(&(self.get_x_reg_value(op) as u16)) {
                            self.increment_pc();
                        }

                        self.increment_pc();
                    },
                    0xA1 => {
                        if !keys.contains(&(self.get_x_reg_value(op) as u16)) {
                            self.increment_pc();
                        }

                        self.increment_pc();
                    },
                    _ => invalid_instruction(op)
                }
            },
            0xF => {
                match get_last_byte(op) {
                    0x07 => {
                        self.registers[get_second_nibble(op) as usize] = self.delay;

                        self.increment_pc();
                    },
                    0x0A => {
                        if key.is_some() {
                            let key = key.unwrap();
                            self.registers[get_second_nibble(op) as usize] = key as u8;

                            self.increment_pc();
                        }
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
                    _ => invalid_instruction(op)
                }
            },
            _ => panic!(format!("Unsupported 0xF instruction {:X}", op))
        }
    }
}