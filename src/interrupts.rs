pub enum InterruptLocation {
    VBlank = 0x40,
    LCD = 0x48,
    Timer = 0x50,
    Serial = 0x58,
    Joypad = 0x60,
}

#[derive(Debug)]
pub struct Interrupts {
    pub vertical_blank: bool,
    pub lcd_c: bool,
    pub timer: bool,
    pub serial_transfer: bool,
    pub joypad: bool,
}

impl Interrupts {
    pub fn new() -> Interrupts {
        Interrupts{
            vertical_blank: false,
            lcd_c: false,
            timer: false,
            serial_transfer: false,
            joypad: false
        }
    }

    pub fn from_byte(&mut self, byte: u8) {
        self.vertical_blank = (byte & 0b1) == 1;
        self.lcd_c = ((byte >> 1) & 0b1) == 1;
        self.timer = ((byte >> 2) & 0b1) == 1;
        self.serial_transfer = ((byte >> 3) & 0b1) == 1;
        self.joypad = ((byte >> 4) & 0b1) == 1;
    }

    pub fn to_byte(&self) -> u8 {
        0b11100000 |
        self.vertical_blank as u8 |
        (self.lcd_c as u8) << 1 |
        (self.timer as u8) << 2 |
        (self.serial_transfer as u8) << 3 |
        (self.joypad as u8) << 4
    }
}
