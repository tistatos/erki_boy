extern crate minifb;
extern crate rusttype;

use std::fs::File;
use std::io::Read;
use std::time::{Instant, Duration};
use std::env;
use std::thread::sleep;

use erki_boy::cpu::CPU;
use erki_boy::gpu::{ONE_FRAME_IN_CYCLES, SCREEN_WIDTH, SCREEN_HEIGHT, SCREEN_PIXEL_COUNT};
use erki_boy::register_output::{RegisterOutput};

use minifb::{Key, KeyRepeat, Window, WindowOptions};


const ONE_SECOND_IN_MICROS: usize = 1000000000;
const ONE_SECOND_IN_CYCLES: usize = 4190000;


fn main() {
    let boot_rom_path = "./dmg_boot.bin";
    let args: Vec<String> = env::args().collect();

    let game_rom_path = if args.len() == 2 {
        &args[1]
    }
    else {
        "./ROMS/cpu_instrs.gb"
    };

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
        SCREEN_WIDTH, SCREEN_HEIGHT + 48,
        WindowOptions::default()
        ).unwrap();

    let mut buffer = [0; SCREEN_PIXEL_COUNT + SCREEN_WIDTH * 48];
    let mut cycles_this_frame = 0usize;
    let mut now = Instant::now();

    let mut halt_execution = false;
    let mut step_execution = false;
    let mut run_to_next_frame = false;
    let register_output = RegisterOutput::new();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let time_delta = now.elapsed().subsec_nanos();
        now = Instant::now();
        let delta = time_delta as f64 / ONE_SECOND_IN_MICROS as f64;
        let cycles_to_run = delta * ONE_SECOND_IN_CYCLES as f64;

        if !halt_execution || step_execution || run_to_next_frame {

            let mut cycles_elapsed = 0;

            if !halt_execution || run_to_next_frame {
                while cycles_elapsed <= cycles_to_run as usize {
                    cycles_elapsed += dmg_cpu.step() as usize;
                }
            }
            else {
                if step_execution {
                    cycles_elapsed += dmg_cpu.step() as usize;
                    dmg_cpu.debug_output();
                    step_execution = false;
                }
            }
            cycles_this_frame += cycles_elapsed;
            if cycles_this_frame >= ONE_FRAME_IN_CYCLES {
                let text = generate_register_output(
                    &register_output, &dmg_cpu);
                for (i, pixel) in dmg_cpu.bus.gpu.screen_buffer.chunks(4).enumerate() {
                    buffer[i] =
                        (pixel[3] as u32) << 24 |
                        (pixel[2] as u32) << 16 |
                        (pixel[1] as u32) << 8 |
                        (pixel[0] as u32);
                }

                for (i, val) in text.iter().enumerate() {
                    buffer[i + SCREEN_PIXEL_COUNT] = *val;
                }
                window.update_with_buffer(&buffer).unwrap();
                cycles_this_frame = 0;
                if run_to_next_frame {
                    dmg_cpu.debug_output();
                }
                run_to_next_frame = false;
            } else {
                sleep(Duration::from_nanos(2))
            }
        }
        window.update();

        dmg_cpu.bus.joypad.reset();
        window.get_keys().map(|keys| {
            for k in keys {
                match k {
                    Key::Up => dmg_cpu.bus.joypad.up(),
                    Key::Down => dmg_cpu.bus.joypad.down(),
                    Key::Left => dmg_cpu.bus.joypad.left(),
                    Key::Right => dmg_cpu.bus.joypad.right(),

                    Key::X => dmg_cpu.bus.joypad.b(),
                    Key::Z => dmg_cpu.bus.joypad.a(),

                    Key::Enter => dmg_cpu.bus.joypad.start(),
                    Key::RightShift => dmg_cpu.bus.joypad.select(),
                    _ => {}
                }
            }
        });

        window.get_keys_pressed(KeyRepeat::Yes).map(|keys| {
            for k in keys {
                match k {
                    Key::S => {
                        step_execution = true;
                    },
                    Key::N => {
                        run_to_next_frame = true;
                    },
                    Key::F5 => {
                        halt_execution = !halt_execution;
                        if halt_execution {
                            println!("halting!");
                        }
                        else {
                            println!("Continuing...");
                        }
                    }
                    _ => {}
                }

            }
        });
    }
}

fn generate_register_output(ro: &RegisterOutput, cpu: &CPU) -> Vec<u32> {
    let upper_text =
        format!("A:{} B:{} C:{} D:{}",
        cpu.registers.a, cpu.registers.b, cpu.registers.c, cpu.registers.d);
    let mut upper_text_pixels = ro.output(upper_text.as_str());
    let mid_text =
        format!("E:{} H:{} L:{} F:{}",
        cpu.registers.e, cpu.registers.h, cpu.registers.l, cpu.registers.f);
    let mut mid_text_pixels = ro.output(mid_text.as_str());
    let lower_text = format!("sp:0x{:04X} pc:0x{:04X}", cpu.sp, cpu.pc);
    let mut lower_text_pixels = ro.output(lower_text.as_str());


    let mut res = vec!();
    res.append(&mut upper_text_pixels);
    res.append(&mut mid_text_pixels);
    res.append(&mut lower_text_pixels);
    res
}
