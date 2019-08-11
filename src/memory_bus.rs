use crate::gpu::{ GPU, Mode, ObjSize, TileData, TileMap };
use crate::interrupts::{Interrupts};
use std::fs::{File};
use std::io::prelude::*;


const BOOT_ROM_START: usize = 0x00;
const BOOT_ROM_END: usize = 0xFF;
const BOOT_ROM_SIZE: usize = BOOT_ROM_END - BOOT_ROM_START + 1;

const ROM_BANK_START: usize = 0x0000;
const ROM_BANK_END: usize = 0x3FFF;
const ROM_BANK_SIZE: usize = ROM_BANK_END - ROM_BANK_START + 1;

const ROM_SWITCHABLE_BANK_START: usize = 0x4000;
const ROM_SWITCHABLE_BANK_END: usize = 0x7FFF;
const ROM_SWITCHABLE_BANK_SIZE: usize = ROM_SWITCHABLE_BANK_END - ROM_SWITCHABLE_BANK_START + 1;

pub const VIDEO_RAM_START: usize = 0x8000;
const VIDEO_RAM_END: usize = 0x9FFF;
pub const VIDEO_RAM_SIZE: usize = VIDEO_RAM_END - VIDEO_RAM_START + 1;

const EXTERNAL_RAM_START: usize = 0xA000;
const EXTERNAL_RAM_END: usize = 0xBFFF;
const EXTERNAL_RAM_SIZE: usize = EXTERNAL_RAM_END - EXTERNAL_RAM_START + 1;

const WORKING_RAM_START: usize = 0xC000;
const WORKING_RAM_END: usize = 0xDFFF;
const WORKING_RAM_SIZE: usize = WORKING_RAM_END - WORKING_RAM_START + 1;

const ECHO_RAM_START: usize = 0xE000;
const ECHO_RAM_END: usize = 0xFDFF;
//const ECHO_RAM_SIZE: usize = ECHO_RAM_END - ECHO_RAM_START + 1;

const OAM_START: usize = 0xFE00;
const OAM_END: usize = 0xFE9F;
pub const OAM_SIZE: usize = OAM_END - OAM_START + 1;

const IO_REGISTERS_START: usize = 0xFF00;
const IO_REGISTERS_END: usize = 0xFF7F;
//const IO_REGISTER_SIZE: usize = IO_REGISTERS_END - IO_REGISTERS_START + 1;

const HRAM_START: usize = 0xFF80;
const HRAM_END: usize = 0xFFFE;
const HRAM_SIZE: usize = HRAM_END - HRAM_START + 1;

const UNUSED_START: usize = 0xFEA0;
const UNUSED_END: usize = 0xFEFF;
//const UNUSED_SIZE: usize = UNUSED_END - UNUSED_START + 1;

const ENABLE_INTERRUPTS: usize = 0xFFFF;

enum TimerFrequency {
    F4096,
    F16384,
    F65536,
    F262144
}

impl TimerFrequency {
    fn in_ticks(&self) -> usize {
        match self {
            TimerFrequency::F4096 => 1024,
            TimerFrequency::F16384 => 256,
            TimerFrequency::F65536 => 64,
            TimerFrequency::F262144 => 16,
        }

    }
}

pub struct Timer {
    frequency: TimerFrequency,
    cycles: usize,
    value: u8,
    modulo: u8,
    active: bool
}


impl Timer {
    pub fn new() -> Self {
        Timer {
            frequency: TimerFrequency::F4096,
            cycles: 0,
            value: 0,
            modulo: 0,
            active: false
        }
    }

    pub fn step(&mut self, cycles: u16) -> bool {
        if !self.active {
            return false;
        }

        self.cycles += cycles as usize;
        let cycles_per_tick = self.frequency.in_ticks();
        let did_overflow = if self.cycles > cycles_per_tick {
            self.cycles = self.cycles % cycles_per_tick;
            let (new, overflow) = self.value.overflowing_add(1);
            self.value = new;
            overflow
        }
        else {
            false
        };

        if did_overflow {
            self.value = self.modulo;
        }
        did_overflow
    }

}

pub struct Divider {
    pub value: u8
}

impl Divider {
    pub fn step(&mut self, cycles: u16) {
        self.value = self.value.wrapping_add(cycles as u8);
    }
}

//pub struct Joypad {
/* use:
 * - bit 5 for button data
 * - bit 4 for dpad data
 * 3-0 bits are either dpad data or button data
 */
//}
//pub struct IO {
//joypad: u8,
//timer: u8,
//dma: u8,
//}

//pub struct AudioDriver {
//}
//

pub struct MemoryBus {
    /*  Memory Map:
     * Interrupt Enable Register    FFFF-FFFF
     * High RAM (RAM on CPU chip)   FF80-FFFE
     * Unusable                     FF4C-FF7F
     * Memory Maped I/O             FF00-FF4B
     * Unusable                     FEA0-FEFF
     * Sprite Attrib Memory (OAM)   FE00-FE9F
     * Echo of Internal RAM         E000-FDFF
     * 8kB Internal RAM             C000-DFFF
     * 8kB switchable RAM bank      A000-BFFF
     * 8kB Video RAM                8000-9FFF
     * 16kB switchable ROM bank     4000-7FFF
     * 16kB ROM bank #0             0000-3FFF
     */
    pub boot_rom: Option<[u8; BOOT_ROM_SIZE]>,
    rom_bank: [u8; ROM_BANK_SIZE],
    switchable_rom_bank: [u8; ROM_SWITCHABLE_BANK_SIZE],
    external_ram: [u8; EXTERNAL_RAM_SIZE],
    working_ram: [u8; WORKING_RAM_SIZE],
    high_ram: [u8; HRAM_SIZE],

    timer: Timer,
    divider: Divider,

    pub interrupts_enabled: Interrupts,
    pub interrupt_flags: Interrupts,

    pub gpu: GPU,
}

impl MemoryBus {
    pub fn new_empty_memory() -> MemoryBus {
        MemoryBus::new(None, vec![0; 0x10000])
    }

    pub fn new(boot_rom_buffer: Option<Vec<u8>>, game_rom_buffer: Vec<u8>) -> MemoryBus {
        let boot_rom = boot_rom_buffer.map(|boot_rom_data| {
            let mut boot_rom = [0; BOOT_ROM_SIZE];
            boot_rom.copy_from_slice(&boot_rom_data);
            boot_rom
        });

        let mut rom_bank = [0xFF; ROM_BANK_SIZE];
        rom_bank.copy_from_slice(&game_rom_buffer[..=ROM_BANK_END]);

        let mut switchable_rom_bank = [0xFF; ROM_SWITCHABLE_BANK_SIZE];
        switchable_rom_bank.copy_from_slice(&game_rom_buffer[ROM_SWITCHABLE_BANK_START..=ROM_SWITCHABLE_BANK_END]);


        MemoryBus {
            boot_rom,
            rom_bank,
            switchable_rom_bank,
            external_ram: [0xFF; EXTERNAL_RAM_SIZE],
            working_ram: [0xFF; WORKING_RAM_SIZE],
            high_ram: [0xFF; HRAM_SIZE],

            interrupts_enabled: Interrupts::new(),
            interrupt_flags: Interrupts::new(),

            timer: Timer::new(),
            divider: Divider{ value: 0 },
            gpu: GPU::new(),
        }
    }

    pub fn step(&mut self, cycles: u16) {
        if self.timer.step(cycles) {
            self.interrupt_flags.timer_interrupt = true;
        }

        self.divider.step(cycles);

        let (vblank, lcd) = self.gpu.step(cycles);
        self.interrupt_flags.vertical_blank_interrupt = vblank;
        self.interrupt_flags.lcd_c_interrupt = lcd;
    }

    pub fn interrupted(&self) -> bool {
        return
        (self.interrupts_enabled.vertical_blank_interrupt &&
            self.interrupt_flags.vertical_blank_interrupt) ||
        (self.interrupts_enabled.lcd_c_interrupt &&
            self.interrupt_flags.lcd_c_interrupt) ||
        (self.interrupts_enabled.timer_interrupt &&
            self.interrupt_flags.timer_interrupt) ||
        (self.interrupts_enabled.serial_transfer_interrupt &&
            self.interrupt_flags.serial_transfer_interrupt) ||
        (self.interrupts_enabled.control_interrupt &&
            self.interrupt_flags.control_interrupt);
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        let address = address as usize;
        match address {
            BOOT_ROM_START...BOOT_ROM_END => {
                if let Some(boot_rom) = self.boot_rom {
                    boot_rom[address]
                } else {
                    self.rom_bank[address]
                }
            }
            ROM_BANK_START...ROM_BANK_END => self.rom_bank[address],
            ROM_SWITCHABLE_BANK_START...ROM_SWITCHABLE_BANK_END => {
                self.switchable_rom_bank[address - ROM_SWITCHABLE_BANK_START]
            }
            VIDEO_RAM_START...VIDEO_RAM_END => {
                self.gpu.video_ram[address - VIDEO_RAM_START]
            },
            EXTERNAL_RAM_START...EXTERNAL_RAM_END => {
                self.external_ram[address - EXTERNAL_RAM_START]
            }
            WORKING_RAM_START...WORKING_RAM_END => self.working_ram[address - WORKING_RAM_START],
            ECHO_RAM_START...ECHO_RAM_END => self.working_ram[address - ECHO_RAM_START],
            OAM_START...OAM_END => self.gpu.oam[address - OAM_START],
            IO_REGISTERS_START...IO_REGISTERS_END => self.read_from_io(address),
            HRAM_START...HRAM_END => self.high_ram[address - HRAM_START],
            ENABLE_INTERRUPTS => { return self.interrupts_enabled.to_byte(); }
            _ => {
                panic!("Error reading from memory location 0x{:X}", address);
            }
        }
    }

    pub fn write_byte(&mut self, address: u16, byte: u8) {
        let address = address as usize;
        match address {
            ROM_BANK_START...ROM_BANK_END => self.rom_bank[address] = byte,
            ROM_SWITCHABLE_BANK_START...ROM_SWITCHABLE_BANK_END => {
                self.switchable_rom_bank[address - ROM_SWITCHABLE_BANK_START] = byte
            }
            VIDEO_RAM_START...VIDEO_RAM_END => {
                self.gpu.write_vram(address - VIDEO_RAM_START, byte)
            }
            EXTERNAL_RAM_START...EXTERNAL_RAM_END => {
                self.external_ram[address - EXTERNAL_RAM_START] = byte
            }
            WORKING_RAM_START...WORKING_RAM_END => {
                self.working_ram[address - WORKING_RAM_START] = byte
            }
            ECHO_RAM_START...ECHO_RAM_END => {
                self.working_ram[address - ECHO_RAM_START] = byte
            }
            OAM_START...OAM_END => {
                self.gpu.write_oam(address - OAM_START, byte)
            }
            IO_REGISTERS_START...IO_REGISTERS_START => {
                self.write_to_io(address, byte)
            }
            HRAM_START...HRAM_END =>  {
                self.high_ram[address - HRAM_START] = byte
            }
            IO_REGISTERS_START...IO_REGISTERS_END => {
                self.write_to_io(address, byte)
            }
            UNUSED_START...UNUSED_END => {},
            ENABLE_INTERRUPTS => {
                self.interrupts_enabled.from_byte(byte);
            },
            _ => panic!("Error writing to memory location 0x{:X}", address),
        };
    }

    fn read_from_io(&self, address: usize) -> u8 {
        match address {
            0xFF00 => { /* P1 - joy pad info */ }

            0xFF01 => { /* SB - Serial transfer data */ }
            0xFF02 => { /* SC - Serial transfer control */ }

            0xFF04 => { return self.divider.value; }
            0xFF05 => { return self.timer.value; }
            0xFF06 => { return self.timer.modulo;  }
            0xFF07 => {
                let freq = match self.timer.frequency {
                   TimerFrequency::F4096 => 0,
                   TimerFrequency::F262144 => 1,
                   TimerFrequency::F65536 => 2,
                   TimerFrequency::F16384 => 3
                };
                return (self.timer.active as u8) << 2 | freq;
            }

            0xFF0F => { return self.interrupt_flags.to_byte(); }

            0xFF40 => {
                return
                    (self.gpu.lcd_display_enabled as u8)                                << 7 |
                    ((self.gpu.window_tile_map == TileMap::Ox9C00) as u8)               << 6 |
                    (self.gpu.window_display_enabled as u8)                             << 5 |
                    ((self.gpu.background_window_tile_data  == TileData::Ox8000) as u8) << 4 |
                    ((self.gpu.background_tile_map == TileMap::Ox9C00) as u8)           << 3 |
                    ((self.gpu.obj_size == ObjSize::Size8x16) as u8)                    << 2 |
                    (self.gpu.obj_display_enable as u8)                                 << 1 |
                    self.gpu.background_display_enabled as u8;
            }
            0xFF41 => {
                let mode = match self.gpu.lcd_mode {
                    Mode::HBlank => 0,
                    Mode::VBlank => 1,
                    Mode::OAMAccess => 2,
                    Mode::VRAMAccess => 3
                };

                return
                    (self.gpu.lyc_interrupt_enabled as u8) << 6 |
                    (self.gpu.oam_interrupt_enabled as u8) << 5 |
                    (self.gpu.vblank_interrupt_enabled as u8) << 4 |
                    (self.gpu.hblank_interrupt_enabled as u8) << 3 |
                    mode;
            }
            0xFF42 => { return self.gpu.scroll_y; }
            0xFF43 => { return self.gpu.scroll_x; }
            0xFF44 => { return self.gpu.lcd_y_coordinate; }
            0xFF45 => { return self.gpu.lcd_y_compare; }

            0xFF4D => { return 0; }

            _ => {
                panic!("Error reading from IO at 0x{:X}", address);
            }
        }
        return 0;
    }

    fn write_to_io(&mut self, address: usize, byte: u8) {
        match address {
            0xFF00 => {
                /* P1 - joy pad info */
                //let query_dpad = ((byte >> 4) & 0b1) == 1;
                //let query_buttons = ((byte >> 5) & 0b1) == 1;
            }

            0xFF01 => {
                /* SB - Serial transfer data */
                self.interrupt_flags.serial_transfer_interrupt = true;
            }
            0xFF02 => { /* SC - Serial transfer control */ }

            0xFF04 => { self.divider.value = 0; }
            0xFF05 => {
                /* TIMA - Timer Counter */
                self.timer.value = byte;
            }
            0xFF06 => {
                /* TMA - Timer modulo */
                self.timer.modulo = byte;
            }
            0xFF07 => {
                /* TAC - Timer control */
                self.timer.active = ((byte >> 2) & 0b1) == 1;
                let clock_select = byte & 0b11;
                self.timer.frequency = match clock_select {
                    0 => TimerFrequency::F4096,
                    1 => TimerFrequency::F262144,
                    2 => TimerFrequency::F65536,
                    3 => TimerFrequency::F16384,
                    _ => panic!("Incorrect timer frequency")
                };
            }

            0xFF0F => {
                /* IF - Interrupt Flag */
                self.interrupt_flags.from_byte(byte);
            }

            0xFF10 => { /* NR 10 - Sound Mode 1 Sweep register */ }
            0xFF11 => { /* NR 11 - Sound Mode 1 Length wave pattern duty*/ }
            0xFF12 => { /* NR 12 - Sound Mode 1 Volume Envelope */ }
            0xFF13 => { /* NR 13 - Sound Mode 1 lo Frequency data Write only */ }
            0xFF14 => { /* NR 14 - Sound Mode 1 hi Frequency data */ }

            0xFF16 => { /* NR 21 - Sound Mode 2 Length wave pattern duty */ }
            0xFF17 => { /* NR 22 - Sound Mode 2 Volume Envelope */ }
            0xFF18 => { /* NR 23 - Sound Mode 2 lo Frequency data Write only */ }
            0xFF19 => { /* NR 24 - Sound Mode 2 hi Frequency data */ }

            0xFF1A => { /* NR 30 - Sound Mode 3 sound on/off */ }
            0xFF1B => { /* NR 31 - Sound Mode 3 sound length */ }
            0xFF1C => { /* NR 32 - Sound Mode 3 select ouput level */ }
            0xFF1D => { /* NR 33 - Sound Mode 3 lo Frequency data Write only*/ }
            0xFF1E => { /* NR 34 - Sound Mode 3 hi Frequency data */ }

            0xFF20 => { /* NR 41 - Sound Mode 4 Sound length */ }
            0xFF21 => { /* NR 42 - Sound Mode 4 Volume Envelope */ }
            0xFF22 => { /* NR 43 - Sound Mode 4 Polynomial counter */ }
            0xFF23 => { /* NR 44 - Sound Mode 4 counter/consecutive */ }
            0xFF24 => { /* NR 50 - Channel control / ON-OFF / Volume */ }
            0xFF25 => { /* NR 51 - Sound output terminal */ }
            0xFF26 => { /* NR 52 - Sound on/off */ }

            0xFF30...0xFF3F => { /* Wave Pattern RAM */ } //FIXME: find documentation for this

            0xFF40 => {
                //LCDC - LCD Control
                self.gpu.lcd_display_enabled = (byte >> 7) == 1;
                self.gpu.window_tile_map = if ((byte >> 6) & 0b1) == 1 {
                    TileMap::Ox9C00
                } else {
                    TileMap::Ox9800
                };
                self.gpu.window_display_enabled = ((byte >> 5) & 0b1) == 1;
                self.gpu.background_window_tile_data = if ((byte >> 4) & 0b1) == 1 {
                    TileData::Ox8000
                } else {
                    TileData::Ox8800
                };
                self.gpu.background_tile_map = if ((byte >> 3) & 0b1) == 1 {
                    TileMap::Ox9C00
                } else {
                    TileMap::Ox9800
                };
                self.gpu.obj_size = if ((byte >> 2) & 0b1) == 1 {
                    ObjSize::Size8x16
                } else {
                    ObjSize::Size8x8
                };
                self.gpu.obj_display_enable = ((byte >> 1) & 0b1) == 1;
                self.gpu.background_display_enabled = (byte & 0b1) == 1;
            }

            0xFF41 => {
                /* STAT - LCDC Status */
                //interrupt select:
                self.gpu.lyc_interrupt_enabled = ((byte >> 6) & 0b1) == 1;
                self.gpu.oam_interrupt_enabled = ((byte >> 5) & 0b1) == 1;
                self.gpu.vblank_interrupt_enabled = ((byte >> 4) & 0b1) == 1;
                self.gpu.hblank_interrupt_enabled = ((byte >> 3) & 0b1) == 1;
            }

            0xFF42 => {
                // SCY - Scroll Y
                self.gpu.scroll_y = byte;
            }
            0xFF43 => {
                // SCX - scroll X
                self.gpu.scroll_x = byte;
            }

            0xFF45 => {
                /* LYC - LY Compare */
                self.gpu.lcd_y_compare = byte;
            }
            0xFF46 => {
                /* DMA - DMA Transfer and Start Address Write only*/
            }

            0xFF47 => {
                /* BGP - BG & Window Palette Data */
                self.gpu.background_window_palette = byte.into();
            }
            0xFF48 => {
                /* OBP0 - Object Palette 0 data*/
                self.gpu.obj_0_palette = byte.into();
            }
            0xFF49 => {
                /* OBP1 - Object Palette 1 data*/
                self.gpu.obj_1_palette = byte.into();
            }

            0xFF4A => {
                /* WY - Window Y Position */
                self.gpu.window_y = byte;
            }
            0xFF4B => {
                /* WX - Window X Position */
                self.gpu.window_x = byte;
            }

            0xFF4D => {
                /* GBC register */
                println!("FF4D writing {}", byte)
            }
            0xFF4F => { /* GBC register */ }

            0xFF50 => {
                self.boot_rom = None; /* Unload ROM boot */
            }

            0xFF68 => { /* GBC register */ }
            0xFF69 => { /* GBC register */ }

            0xFF7F => {}
            _ => {
                panic!("Error writing to IO at 0x{:X}", address);
            }
        };
    }

    pub fn dump_memory_to_file(&self) {
        print!("Dumping...");
        let mut ram = File::create("./RAM.bin").unwrap();
        ram.write_all(&self.working_ram).unwrap();
        println!("OK!");
    }
}

#[cfg(test)]
mod tests {

    mod timerTests {
        use super::*;
        #[test]
        fn timer_overflows() {
            let mut timer = Timer::new();
            timer.active = true;
            timer.modulo = 128;
            let overflow = timer.step(1024);
            assert_eq!(timer.value, 0);
            let overflow = timer.step(1);
            assert_eq!(timer.value, 1);
            assert_eq!(timer.cycles, 1);
            assert_eq!(overflow, false);
            timer.value = 255;
            let overflow = timer.step(1024);
            assert_eq!(overflow, true);
            assert_eq!(timer.value, 128);
        }
    }

    use super::*;
    use crate::gpu::{Color};

    #[test]
    fn divider_zeroed() {
        let mut mem = MemoryBus::new_empty_memory();
        mem.step(10);
        assert_eq!(mem.divider.value, 10);
        assert_eq!(mem.read_byte(0xFF04), 10);
        mem.write_byte(0xFF04, 1);
        assert_eq!(mem.divider.value, 0);
    }

    #[test]
    fn write_echo_ram() {
        let mut mem = MemoryBus::new_empty_memory();
        let value = 100;
        mem.write_byte((ECHO_RAM_START + 5) as u16, value);
        let memory_value = mem.read_byte((WORKING_RAM_START + 5) as u16);
        assert_eq!(value, memory_value);
    }

    #[test]
    fn write_palette_data() {
        let mut mem = MemoryBus::new_empty_memory();
        let value = 0xFC;
        mem.write_byte((0xFF47), value);
        assert_eq!(mem.gpu.background_window_palette.0 as u8, Color::White as u8);
        assert_eq!(mem.gpu.background_window_palette.1 as u8, Color::Black as u8);
        assert_eq!(mem.gpu.background_window_palette.2 as u8, Color::Black as u8);
        assert_eq!(mem.gpu.background_window_palette.3 as u8, Color::Black as u8);
    }
}
