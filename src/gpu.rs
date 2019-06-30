use crate::memory_bus::VIDEO_RAM_SIZE;

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

pub enum TileData {
    Ox8800,
    Ox8000,
}
pub enum TileMap {
    Ox9800,
    Ox9C00,
}

pub enum ObjSize {
    Size8x8,
    Size8x16,
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
            3 => Color::DarkGray,
            4 => Color::Black,
            _ => panic!("Failed to determine palette color: {}", value),
        }
    }
}

pub struct Palette(Color, Color, Color, Color);
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
    screen_buffer: [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
    video_ram: [u8; VIDEO_RAM_SIZE],
    cycles: u16,

    //LCD Control
    pub lcd_display_enabled: bool, //LCD is complete on/off
    pub lcd_y_compare: u8,

    pub background_window_tile_data: TileData, // location of tile data for background and window
    pub background_tile_map: TileMap,          //background tile map location
    pub background_display_enabled: bool,      //background display status
    pub background_window_palette: Palette,

    pub obj_size: ObjSize,        //Size of objs (sprites)
    pub obj_display_enable: bool, //Obj display status
    pub obj_0_palette: Palette,
    pub obj_1_palette: Palette,

    //LCD Status
    //ly_coincidence_interrupt: bool,
    //oam_interrupt: bool,
    //vblank_interrupt: bool,
    //hblank_interrupt: bool,
    //coincidence_flag: bool,
    //lcd_y_coordinate: u8, //current line being drawn
    lcd_mode: Mode, // LCD current mode

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
            video_ram: [0; VIDEO_RAM_SIZE],
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
            lcd_mode: Mode::HBlank,
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
        }
    }
}
