use crate::gpu::{ObjSize, TileData, TileMap, GPU};

const BOOT_ROM_START: usize = 0x00;
const BOOT_ROM_END: usize = 0xFF;
const BOOT_ROM_SIZE: usize = BOOT_ROM_END - BOOT_ROM_START + 1;

const ROM_BANK_START: usize = 0x0000;
const ROM_BANK_END: usize = 0x3FFF;
const ROM_BANK_SIZE: usize = ROM_BANK_END - ROM_BANK_START + 1;

const ROM_SWITCHABLE_BANK_START: usize = 0x4000;
const ROM_SWITCHABLE_BANK_END: usize = 0x7FFF;
const ROM_SWITCHABLE_BANK_SIZE: usize = ROM_SWITCHABLE_BANK_END - ROM_BANK_START + 1;

const VIDEO_RAM_START: usize = 0x8000;
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
const ECHO_RAM_SIZE: usize = ECHO_RAM_END - ECHO_RAM_START + 1;

const OAM_START: usize = 0xFE00;
const OAM_END: usize = 0xFE9F;
const OAM_SIZE: usize = OAM_END - OAM_START + 1;

const IO_REGISTERS_START: usize = 0xFF00;
const IO_REGISTERS_END: usize = 0xFF7F;
const IO_REGISTER_SIZE: usize = IO_REGISTERS_END - IO_REGISTERS_START + 1;

const HRAM_START: usize = 0xFF80;
const HRAM_END: usize = 0xFFFE;
const HRAM_SIZE: usize = HRAM_END - HRAM_START + 1;

//pub struct interrupt {
//joypad: 4 (RST 60)
//serial: 3 (RST 58)
//time: 2 (RST 50)
//lcd stat: 1 RST (48)
//v-blank: 0 RST (40)
//}
//pub struct Timer {
//}
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

pub struct MemoryBus {
    /* Interrupt Enable Register    FFFF-FFFF
     * High RAM (RAM on CPU chip)   FF80-FFFE
     * Unusable                     FF4C-FF7F
     * I/O Ports                    FF00-FF4B
     * Unusable                     FEA0-FEFF
     * Sprite Attrib Memory (OAM)   FE00-FE9F
     * Echo of Internal RAM         E000-FDFF
     * 8kB Internal RAM             C000-DFFF
     * 8kB switchable RAM bank      A000-BFFF
     * 8kB Video RAM                8000-9FFF
     * 16kB switchable ROM bank     4000-7FFF
     * 16kB ROM bank #0             0000-3FFF
     */
    boot_rom: Option<[u8; BOOT_ROM_SIZE]>,
    rom_bank: [u8; ROM_BANK_SIZE],
    switchable_rom_bank: [u8; ROM_SWITCHABLE_BANK_SIZE],
    external_ram: [u8; EXTERNAL_RAM_SIZE],
    working_ram: [u8; WORKING_RAM_SIZE],
    oam: [u8; OAM_SIZE],
    high_ram: [u8; HRAM_SIZE],

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

        //Load non-switchable bank and first part of switchable rom data
        let mut rom_bank = [0; ROM_BANK_SIZE];
        let mut switchable_rom_bank = [0; ROM_SWITCHABLE_BANK_SIZE];

        MemoryBus {
            boot_rom,
            rom_bank,
            switchable_rom_bank,
            external_ram: [0; EXTERNAL_RAM_SIZE],
            working_ram: [0; WORKING_RAM_SIZE],
            oam: [0; OAM_SIZE],
            high_ram: [0; HRAM_SIZE],

            gpu: GPU::new(),
        }
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
            // VIDEO_RAM_START...VIDEO_RAM_END => self.gpu.video_ram[address - VIDEO_RAM_START], FIXME: cannot be read?
            EXTERNAL_RAM_START...EXTERNAL_RAM_END => {
                self.external_ram[address - EXTERNAL_RAM_START]
            }
            WORKING_RAM_START...WORKING_RAM_END => self.working_ram[address - WORKING_RAM_START],
            ECHO_RAM_START...ECHO_RAM_END => self.working_ram[address - ECHO_RAM_START],
            OAM_START...OAM_END => self.oam[address - OAM_START],
            IO_REGISTERS_START...IO_REGISTERS_END => self.read_from_io(address),
            HRAM_START...HRAM_END => self.high_ram[address - HRAM_START],
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
            VIDEO_RAM_START...VIDEO_RAM_END => self.gpu.write_vram(address - VIDEO_RAM_START, byte),
            EXTERNAL_RAM_START...EXTERNAL_RAM_END => {
                self.external_ram[address - EXTERNAL_RAM_START] = byte
            }
            WORKING_RAM_START...WORKING_RAM_END => {
                self.working_ram[address - WORKING_RAM_START] = byte
            }
            ECHO_RAM_START...ECHO_RAM_END => self.working_ram[address - ECHO_RAM_START] = byte,
            OAM_START...OAM_END => self.oam[address - OAM_START] = byte,
            IO_REGISTERS_START...IO_REGISTERS_START => self.write_to_io(address, byte),
            HRAM_START...HRAM_END => self.high_ram[address - HRAM_START] = byte,

            _ => panic!("Error writing to memory location 0x{:X}", address),
        };
    }

    fn read_from_io(&self, address: usize) -> u8 {
        match address {
            0xFF00 => { /* P1 - joy pad info */ }

            0xFF01 => { /* SB - Serial transfer data */ }
            0xFF02 => { /* SC - Serial transfer control */ }

            0xFF04 => { /* DIV - Divider register */ }
            0xFF05 => { /* TIMA - Timer Counter */ }
            0xFF06 => { /* TMA - Timer modulo */ }
            0xFF07 => { /* TAC - Timer control */ }

            0xFF44 => { /* LY - LCDC Y-coord Read only*/ }

            0xFF0F => { /* IF - Interrupt Flag */ }
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
                let query_dpad = ((byte >> 4) & 0b1) == 1;
                let query_buttons = ((byte >> 5) & 0b1) == 1;
            }

            0xFF01 => { /* SB - Serial transfer data */ }
            0xFF02 => { /* SC - Serial transfer control */ }

            0xFF04 => { /* DIV - Divider register */ }
            0xFF05 => { /* TIMA - Timer Counter */ }
            0xFF06 => { /* TMA - Timer modulo */ }
            0xFF07 => {
                /* TAC - Timer control */
                let start_timer = ((byte >> 2) & 0b1) == 1;
                let clock_select = byte & 0b11;
            }

            0xFFFF => {
                /* IF - Interrupt Flag */
                let v_blank = (byte & 0b1) == 1;
                let lcdc = ((byte >> 1) & 0b1) == 1; //see stat
                let timer_overflow = ((byte >> 2) & 0b1) == 1;
                let serial_io_transfer_complete = ((byte >> 3) & 0b1) == 1;
                let input = ((byte >> 3) & 0b1) == 1;
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
                    TileMap::Ox9800
                } else {
                    TileMap::Ox9C00
                };
                self.gpu.window_display_enabled = ((byte >> 5) & 0b1) == 1;
                self.gpu.background_window_tile_data = if ((byte >> 4) & 0b1) == 1 {
                    TileData::Ox8800
                } else {
                    TileData::Ox8000
                };
                self.gpu.background_tile_map = if ((byte >> 3) & 0b1) == 1 {
                    TileMap::Ox9800
                } else {
                    TileMap::Ox9C00
                };
                self.gpu.obj_size = if ((byte >> 2) & 0b1) == 1 {
                    ObjSize::Size8x8
                } else {
                    ObjSize::Size8x16
                };
                self.gpu.obj_display_enable = ((byte >> 1) & 0b1) == 1;
                self.gpu.background_display_enabled = (byte & 0b1) == 1;
            }
            0xFF41 => {
                /* STAT - LCDC Status */
                //interrupt select:

                //self.lyc_interrupt = ((byte >> 6) & 0b1) == 1;
                //self.OAM_interrupt = ((byte >> 5) & 0b1) == 1;
                //self.vblank_interrupt = ((byte >> 4) & 0b1) == 1;
                //self.hblank_interrupt = ((byte >> 3) & 0b1) == 1;
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
            0xFF46 => { /* DMA - DMA Transfer and Start Address Write only*/ }

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

            0xFF50 => self.boot_rom = None, /* Unload ROM boot */

            _ => {
                panic!("Error writing to IO at 0x{:X}", address);
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_echo_ram() {
        let mut mem = MemoryBus::new_empty_memory();
        let value = 100;
        mem.write_byte((ECHO_RAM_START + 5) as u16, value);
        let memory_value = mem.read_byte((WORKING_RAM_START + 5) as u16);
        assert_eq!(value, memory_value);
    }
}
