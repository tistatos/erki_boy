extern crate minifb;

use std::fs::File;
use std::io::Read;

use erki_boy::cpu::CPU;
use minifb::{Key, Scale, Window, WindowOptions};

fn main() {
    let boot_rom_path = "./dmg_boot.bin";
    let game_rom_path = "./cpu_instrs.gb";

    let mut boot_rom_file = File::open(boot_rom_path).expect("Missing boot ROM");
    let mut boot_rom = Vec::new();
    boot_rom_file
        .read_to_end(&mut boot_rom)
        .expect("error reading boot ROM");

    let mut game_rom_file = File::open(game_rom_path).expect("No game ROM");
    let mut game_rom = Vec::new();
    game_rom_file
        .read_to_end(&mut game_rom)
        .expect("error reading game ROM");

    let mut dmg_cpu = CPU::new(Some(boot_rom), game_rom);

    let mut window = Window::new(
        "Erki Boy",
        160, 144,
        WindowOptions::default()
        ).unwrap();
    let mut buffer = [0; 160*144];
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let cycles = dmg_cpu.step();
        dmg_cpu.bus.gpu.step(cycles);
        for (i, pixel) in dmg_cpu.bus.gpu.screen_buffer.chunks(4).enumerate() {
            buffer[i] =
                (pixel[3] as u32) << 24 |
                (pixel[2] as u32) << 16 |
                (pixel[1] as u32) << 8 |
                (pixel[0] as u32);
        }
        window.update_with_buffer(&buffer).unwrap();
    }
}
