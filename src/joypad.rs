#[derive(Debug)]
pub struct Joypad {
/* use:
 * - bit 5 for button data
 * - bit 4 for dpad data
 * 3-0 bits are either dpad data or button data
 */
    pub column: bool,

    up: bool,
    down: bool,
    left: bool,
    right: bool,
    a: bool,
    b: bool,
    select: bool,
    start: bool
}

impl Joypad {
    pub fn new() -> Joypad {
        Joypad {
            column: false,

            up: false,
            down: false,
            left: false,
            right: false,
            a: false,
            b: false,
            select: false,
            start: false
        }
    }

    pub fn poll(&self) -> u8 {
        let result = if self.column {
            let button_bit = (1 as u8) << 5;
            let start_bit = !(self.start as u8) << 3;
            let select_bit = !(self.select as u8) << 2;
            let b_bit = !(self.b as u8) << 1;
            let a_bit = !self.a as u8;

            button_bit | start_bit | select_bit | b_bit | a_bit
        }
        else {
            let dpad_bit = 1 << 4;
            let down_bit = !(self.down as u8) << 3;
            let up_bit = !(self.up as u8) << 2;
            let left_bit = !(self.left as u8) << 1;
            let right_bit = !self.right as u8;

            dpad_bit | down_bit | up_bit | left_bit | right_bit
        };

        result
    }

    pub fn reset(&mut self) {
        self.up = false;
        self.down = false;
        self.left = false;
        self.right = false;
        self.a = false;
        self.b = false;
        self.select = false;
        self.start = false;
    }

    pub fn up(&mut self) { self.up = true; }
    pub fn down(&mut self) { self.down = true; }
    pub fn left(&mut self) { self.left = true; }
    pub fn right(&mut self) { self.right = true; }

    pub fn a(&mut self) { self.a = true; }
    pub fn b(&mut self) { self.b = true; }

    pub fn select(&mut self) { self.select = true; }
    pub fn start(&mut self) { self.start = true; }
}
