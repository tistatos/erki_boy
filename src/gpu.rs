use crate::memory_bus::VIDEO_RAM_SIZE;
use crate::memory_bus::VIDEO_RAM_START;
use crate::memory_bus::OAM_SIZE;

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

pub const SCREEN_PIXEL_COUNT: usize = SCREEN_WIDTH * SCREEN_HEIGHT;
pub const ONE_FRAME_IN_CYCLES: usize = 70224;

pub const OAM_NUMBER_OF_OBJECTS: usize = 40;

#[derive(PartialEq)]
pub enum TileData {
    Ox8000,
    Ox8800,
}

#[derive(Debug, PartialEq)]
pub enum TileMap {
    Ox9800,
    Ox9C00,
}

//Maps tile value to a palette value
#[derive(Copy, Clone, PartialEq)]
pub enum TilePixelValue {
    Zero,
    One,
    Two,
    Three
}

impl Default for TilePixelValue {
    fn default() -> Self {
        TilePixelValue::Zero
    }
}

type TileRow = [TilePixelValue; 8];
type Tile = [TileRow; 8];

fn empty_tile() -> Tile {
    [[Default::default(); 8]; 8]
}


#[derive(PartialEq)]
pub enum ObjSize {
    Size8x8,
    Size8x16,
}

#[derive(Copy, Clone)]
pub struct ObjectData {
    x: i16,
    y: i16,
    tile: u8,
    priority: bool,
    flip_x: bool,
    flip_y: bool,
    palette: ObjectPalette
}

impl Default for ObjectData {
    fn default() -> Self {
        ObjectData {
            x: -16,
            y: -8,
            tile: Default::default(),
            palette: Default::default(),
            flip_x: Default::default(),
            flip_y: Default::default(),
            priority: Default::default(),
        }
    }
}

pub enum Mode {
    HBlank,
    VBlank,
    OAMAccess,
    VRAMAccess, //OAM search 20 clocks
                //pixel transfer 43+ clocks
                //h-blank 51-pixeltransfer clocks
                //v-blank 10 clocks
}

#[derive(Copy, Clone)]
pub enum Color {
    White = 255,
    LightGray = 192,
    DarkGray = 96,
    Black = 0,
}

impl std::convert::From<u8> for Color {
    fn from(value: u8) -> Self {
        match value {
            0 => Color::White,
            1 => Color::LightGray,
            2 => Color::DarkGray,
            3 => Color::Black,
            _ => panic!("Failed to determine palette color: {}", value),
        }
    }
}

#[derive(Copy, Clone)]
enum ObjectPalette {
    Zero,
    One
}

impl Default for ObjectPalette {
    fn default() -> Self {
        ObjectPalette::Zero
    }
}

#[derive(Copy, Clone)]
pub struct Palette(pub Color, pub Color, pub Color, pub Color);
impl Palette {
    fn new() -> Palette {
        Palette(
            Color::White,
            Color::LightGray,
            Color::DarkGray,
            Color::Black,
        )
    }
}
impl std::convert::From<u8> for Palette {
    fn from(value: u8) -> Self {
        Palette(
            (value & 0b11).into(),
            ((value >> 2) & 0b11).into(),
            ((value >> 4) & 0b11).into(),
            (value >> 6).into(),
        )
    }
}

pub struct GPU {
    /* Display data
     * 160 x144 pixels on screen, background map is 256x256
     * 4 shades of gray
     * 8x8 pixel tile based, 20x18 tiles on screen show, 32x32 tiles in vram(background map)
     * 256 tiles in total,
     * 40 sprites (10 per line)
     * 8 KB VRAM
     * has different layers: background, window, sprite
     */

    /* VRAM memory map
     * 1KB window map 32x32 index
     * 1KB BG map 32x32 index
     * 4KB BG Tiles 256 x 16 bytes
     * 4KB sprite tiles 256 x 16 bytes
     * cant fit all so has different modes
     */

    /* OAM Entry 4 bytes in RAM
     * position x
     * position y
     * tile number
     * priority
     * flip x
     * flip y
     * palette
     */
    pub screen_buffer: [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
    pub video_ram: [u8; VIDEO_RAM_SIZE],
    pub oam: [u8; OAM_SIZE],
    cycles: u16,

    //LCD Control
    pub lcd_display_enabled: bool, //LCD is complete on/off
    pub lcd_y_compare: u8,

    pub background_window_tile_data: TileData, // location of tile data for background and window
    pub background_tile_map: TileMap,          //background tile map location
    pub background_display_enabled: bool,      //background display status
    pub background_window_palette: Palette,

    pub tile_set: [Tile; 384],

    pub obj_size: ObjSize,        //Size of objs (sprites)
    pub obj_display_enable: bool, //Obj display status
    pub obj_0_palette: Palette,
    pub obj_1_palette: Palette,

    pub obj_data: [ObjectData; OAM_NUMBER_OF_OBJECTS],

    //LCD Status
    //ly_coincidence_interrupt: bool,
    //oam_interrupt: bool,
    //vblank_interrupt: bool,
    //hblank_interrupt: bool,
    //coincidence_flag: bool,
    pub lcd_y_coordinate: u8, //current line being drawn
    pub lcd_mode: Mode, // LCD current mode

    pub window_tile_map: TileMap, //Window tile map location
    pub window_display_enabled: bool,
    pub window_x: u8,
    pub window_y: u8,

    pub scroll_x: u8,
    pub scroll_y: u8,
}

impl GPU {
    pub fn new() -> GPU {
        GPU {
            screen_buffer: [0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
            video_ram: [0xFF; VIDEO_RAM_SIZE],
            oam: [0xFF; OAM_SIZE],
            cycles: 0,
            lcd_display_enabled: false,
            background_window_tile_data: TileData::Ox8000,
            background_tile_map: TileMap::Ox9800,
            background_display_enabled: false,
            background_window_palette: Palette::new(),
            obj_size: ObjSize::Size8x8,
            obj_display_enable: false,
            obj_0_palette: Palette::new(),
            obj_1_palette: Palette::new(),
            obj_data: [Default::default(); OAM_NUMBER_OF_OBJECTS],
            tile_set: [empty_tile(); 384],

            lcd_mode: Mode::HBlank,
            lcd_y_coordinate: 0,
            lcd_y_compare: 0,

            window_tile_map: TileMap::Ox9800,
            window_display_enabled: false,
            window_x: 0,
            window_y: 0,
            scroll_x: 0,
            scroll_y: 0,
        }
    }

    pub fn step(&mut self, cycles: u16) {
        if !self.lcd_display_enabled {
            return;
        }

        self.cycles += cycles;

        match self.lcd_mode {
            Mode::OAMAccess => {
                if self.cycles >= 80 {
                    self.lcd_mode = Mode::VRAMAccess;
                    self.cycles = self.cycles % 80;
                }
            },
            Mode::VRAMAccess => {
                if self.cycles >= 172 {
                    self.cycles = self.cycles % 172;
                    self.lcd_mode = Mode::HBlank;
                    //TODO: add vram related interrupts
                    self.render_scanline();
                }
            },
            Mode::HBlank => {
                if self.cycles >= 200 {

                    self.cycles = self.cycles % 200;
                    self.lcd_y_coordinate += 1;

                    if self.lcd_y_coordinate >= 144 {
                        self.lcd_mode = Mode::VBlank;
                        //TODO: add vblank related interrupts
                    }
                    else {
                        self.lcd_mode = Mode::OAMAccess;
                        //TODO: add hblank related interrupts
                    }
                }
            },
            Mode::VBlank => {
                if self.cycles >= 456 {
                    self.cycles = self.cycles % 456;
                    self.lcd_y_coordinate += 1;

                    if self.lcd_y_coordinate == 154 {
                        self.lcd_mode = Mode::OAMAccess;
                        self.lcd_y_coordinate = 0;
                        //TODO: add vblank related interrupts
                    }
                }
            }
        }
    }

    pub fn write_vram(&mut self, address: usize, value: u8) {
        self.video_ram[address] = value;
        if address >= 0x1800 {
            return;
        }

        //tile background maps
        let index = address;
        let normalized_index = index & 0xFFFE;
        let first_byte = self.video_ram[normalized_index];
        let second_byte = self.video_ram[normalized_index + 1];

        let tile_index = index / 16;
        let row_index = (index % 16) / 2;

        for pixel_index in 0..8 {
            let mask = 1 << (7 - pixel_index);
            let lsb = first_byte & mask;
            let msb = second_byte & mask;

            let value = match (lsb != 0, msb != 0) {
                (true, true) => TilePixelValue::Three,
                (false, true) => TilePixelValue::Two,
                (true, false) => TilePixelValue::One,
                (false, false) => TilePixelValue::Zero
            };

            self.tile_set[tile_index][row_index][pixel_index] = value;
        }
    }

    pub fn write_oam(&mut self, index: usize, value: u8) {
        self.oam[index] = value;
        let object_index = index / 4;
        if object_index > OAM_NUMBER_OF_OBJECTS {
            return;
        }

        let byte = index % 4;

        let mut data = self.obj_data.get_mut(object_index).unwrap();
        match byte {
            0 => data.y = (value as i16) - 0x10,
            1 => data.x = (value as i16) - 0x8,
            2 => data.tile = value,
            _ => {
                data.palette = if (value & 0x10) != 0 {
                    ObjectPalette::One
                }
                else {
                    ObjectPalette::Zero
                };
                data.flip_x = (value & 0x20) != 0;
                data.flip_y = (value & 0x40) != 0;
                data.priority = (value & 0x80) != 0;
            }
        }

    }


    fn render_scanline(&mut self) {
        let mut scanline: [TilePixelValue; SCREEN_WIDTH] = [Default::default(); SCREEN_WIDTH];

        if self.background_display_enabled {
            let mut tile_x_index = self.scroll_x / 8;
            let tile_y_index = self.lcd_y_coordinate.wrapping_add(self.scroll_y);
            let tile_offset = (tile_y_index as u16 / 8) * 32u16;

            //FIXME: background_tile_map to u16 here?
            let background_tile_map = match self.background_tile_map {
                TileMap::Ox9800 => 0x9800,
                TileMap::Ox9C00 => 0x9C00,
            };
            let tile_map_begin = background_tile_map - VIDEO_RAM_START;
            let tile_map_offset = tile_map_begin + tile_offset as usize;

            let row_y_offset = tile_y_index % 8;
            let mut pixel_x_index = self.scroll_x % 8;

            //if self.background_window_tile_data == TileData::Ox8800 {
                //panic!("Unsupported window and tile data area");
            //}

            let mut screen_buffer_offset =
                self.lcd_y_coordinate as usize * SCREEN_WIDTH * 4;
            for line_x in 0..SCREEN_WIDTH {
                let tile_index = self.video_ram[tile_map_offset + tile_x_index as usize];
                let tile_value = self.tile_set
                    [tile_index as usize]
                    [row_y_offset as usize]
                    [pixel_x_index as usize];

                let color = self.tile_value_to_background_color(&tile_value);

                self.screen_buffer[screen_buffer_offset] = color as u8;
                self.screen_buffer[screen_buffer_offset + 1] = color as u8;
                self.screen_buffer[screen_buffer_offset + 2] = color as u8;
                self.screen_buffer[screen_buffer_offset + 3] = 255;
                screen_buffer_offset += 4;

                scanline[line_x] = tile_value;
                pixel_x_index = (pixel_x_index + 1) % 8;
                if pixel_x_index == 0 {
                    tile_x_index += 1;
                }
                //if self.background_window_tile_data == TileData::Ox8800 {
                    //panic!("Unsupported window and tile data area");
                //}
            }
        }
        if self.obj_display_enable {
            let object_height = match self.obj_size {
                ObjSize::Size8x8 => 8,
                ObjSize::Size8x16 => 16
            };

            for obj in self.obj_data.iter() {
                let line = self.lcd_y_coordinate as i16;
                if obj.y <= line && obj.y + object_height > line {
                    let pixel_y_offset = line - obj.y;
                    let tile_index = if object_height == 16 &&
                        (!obj.flip_y && pixel_y_offset > 7) ||
                        (obj.flip_y && pixel_y_offset < 7) {
                            obj.tile + 1
                        }
                    else {
                        obj.tile
                    };

                    let tile = self.tile_set[tile_index as usize];
                    let tile_row = if obj.flip_y {
                        tile[(7 - (pixel_y_offset % 8)) as usize]
                    }
                    else {
                        tile[(pixel_y_offset % 8) as usize]
                    };

                    let screen_y_offset = line as i32 * SCREEN_WIDTH as i32;
                    let mut screen_offset =
                        ((screen_y_offset + obj.x as i32) * 4) as usize;
                    for x in 0..8i16 {
                        let pixel_x_offset = if obj.flip_x {
                            (7-x)
                        }
                        else {
                            x
                        } as usize;
                        let x_offset = obj.x + x;
                        let pixel = tile_row[pixel_x_offset];
                        if x_offset >= 0 &&
                            x_offset < SCREEN_WIDTH as i16 &&
                            pixel != TilePixelValue::Zero &&
                            (obj.priority || scanline[x_offset as usize] == TilePixelValue::Zero) {
                                let color = self.tile_value_to_background_color(&pixel);
                                self.screen_buffer[screen_offset] = color as u8;
                                self.screen_buffer[screen_offset + 1] = color as u8;
                                self.screen_buffer[screen_offset + 2] = color as u8;
                                self.screen_buffer[screen_offset + 3] = 255;
                            }
                            screen_offset += 4;
                    }
                }
            }

        }

        if self.window_display_enabled {
        }
    }

    fn tile_value_to_background_color(&self, tile_value: &TilePixelValue) -> Color {
        match tile_value {
            TilePixelValue::Zero => self.background_window_palette.0,
            TilePixelValue::One => self.background_window_palette.1,
            TilePixelValue::Two => self.background_window_palette.2,
            TilePixelValue::Three => self.background_window_palette.3
        }
    }
}
