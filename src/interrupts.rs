pub enum InterruptLocation {
    VBlank = 0x40,
    LCD = 0x48,
    Timer = 0x50,
    Serial = 0x58,
    Joypad = 0x60,
}

pub struct Interrupts {
    pub vertical_blank_interrupt: bool,
    pub lcd_c_interrupt: bool,
    pub timer_interrupt: bool,
    pub serial_transfer_interrupt: bool,
    pub control_interrupt: bool,
}

impl Interrupts {
    pub fn new() -> Interrupts {
        Interrupts{
            vertical_blank_interrupt: false,
            lcd_c_interrupt: false,
            timer_interrupt: false,
            serial_transfer_interrupt: false,
            control_interrupt: false
        }
    }

    pub fn from_byte(&mut self, byte: u8) {
        self.vertical_blank_interrupt = (byte & 0b1) == 1;
        self.lcd_c_interrupt = ((byte >> 1) & 0b1) == 1;
        self.timer_interrupt = ((byte >> 2) & 0b1) == 1;
        self.serial_transfer_interrupt = ((byte >> 3) & 0b1) == 1;
        self.control_interrupt = ((byte >> 4) & 0b1) == 1;
    }

    pub fn to_byte(&self) -> u8 {
        self.vertical_blank_interrupt as u8 |
        (self.lcd_c_interrupt as u8) << 1 |
        (self.timer_interrupt as u8) << 2 |
        (self.serial_transfer_interrupt as u8) << 3 |
        (self.control_interrupt as u8) << 4
    }
}
