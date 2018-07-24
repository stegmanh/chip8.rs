## Chip 8 Interpreter
A chip 9 interpreter written in Rust. Built by referencing Cowgod's great chip-8 reference found here: http://devernay.free.fr/hacks/chip8/C8TECH10.HTM.

This is now _mostly_ complete. There might be some instructions missing - but it runs PONG and BRIX (along with a few others).

## Usage
Run: 

`$ cargo run PATH_TO_ROM`

Build

`$ cargo build`

As noted below - running with release is not recommended right now.

## Notes

#### Code
This was my first crack at an emulator like program. I went with a rather rudimentary approach where I `switch`ed on each `opcode` and stepped accordingly.

I tried (in most places) to try to have somewhat clean code. I wrote some utility functions for bit manipulation, using Cowgod's convention for referring to bits in the op codes. 

The code surrounding sound & key presses leaves a bit to be desired. Particularly trying to determine if something was a complete key press vs. if the key is still held down. My implementation for distinguishing between the two is guess work, but it _appears_ to work.

#### Current Issues
It would appear that building the interpreter with `--release` makes it run too quickly. I think I can solve this my emulating cycles or something along those lines.

I would bet there's an op code or two missing. I'll add those later if needed.

#### Why?
Went down an emulator rabbit hole, someone mentioned chip-8 and it seemed like a fun project to learn a bit about emulators, op codes and some computer concepts I wasn't very familiar with. 