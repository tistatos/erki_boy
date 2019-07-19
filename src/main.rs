extern crate minifb;

use std::fs::File;
use std::io::Read;
use std::time::{Duration, Instant};

use erki_boy::cpu::CPU;
use erki_boy::gpu::{ONE_FRAME_IN_CYCLES, SCREEN_WIDTH, SCREEN_HEIGHT, SCREEN_PIXEL_COUNT};
use minifb::{Key, Window, WindowOptions};

const ONE_SECOND_IN_MICROS: usize = 1000000000;
const ONE_SECOND_IN_CYCLES: usize = 4190000;

fn main() {
    let boot_rom_path = "./dmg_boot.bin";
    let game_rom_path = "./tetris.gb";
    //let game_rom_path = "./cpu_instrs.gb";

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
        SCREEN_WIDTH, SCREEN_HEIGHT,
        WindowOptions::default()
        ).unwrap();

    let mut buffer = [0; SCREEN_PIXEL_COUNT];
    let mut cycles_this_frame = 0usize;
    let mut now = Instant::now();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mut cycles_elapsed = 0;
        let time_delta = now.elapsed().subsec_nanos();
        now = Instant::now();
        let delta = time_delta as f64 / ONE_SECOND_IN_MICROS as f64;
        let cycles_to_run = delta * ONE_SECOND_IN_CYCLES as f64;

        while cycles_elapsed <= cycles_to_run as usize {
            cycles_elapsed += dmg_cpu.step() as usize;
        }
        cycles_this_frame += cycles_elapsed;
        if cycles_this_frame >= ONE_FRAME_IN_CYCLES {
            for (i, pixel) in dmg_cpu.bus.gpu.screen_buffer.chunks(4).enumerate() {
                buffer[i] =
                    (pixel[3] as u32) << 24 |
                    (pixel[2] as u32) << 16 |
                    (pixel[1] as u32) << 8 |
                    (pixel[0] as u32);
            }
            window.update_with_buffer(&buffer).unwrap();
            cycles_this_frame = 0;
        }

        window.get_keys().map(|keys| {
            for k in keys {
                match k {
                    Key::O => dmg_cpu.debug_output(),
                    _ => {}
                }

            }
        });
    }
}
