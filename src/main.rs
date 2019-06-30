use std::fs::File;
use std::io::Read;

use erki_boy::cpu::CPU;

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

    loop {
        dmg_cpu.step();
    }
}
