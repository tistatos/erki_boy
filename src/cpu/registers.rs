use super::flags_register::FlagsRegister;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Registers {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub f: FlagsRegister, //Flags register, lower 4 bits are always 0
    pub h: u8,
    pub l: u8,
}

impl Registers {
    pub fn new() -> Registers {
        Registers {
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            f: FlagsRegister::new(),
            h: 0,
            l: 0,
        }
    }

    pub fn get_af(&self) -> u16 {
        (self.a as u16) << 8 | u8::from(self.f) as u16
    }
    pub fn set_af(&mut self, value: u16) {
        self.a = ((value & 0xFF00) >> 8) as u8;
        self.f = FlagsRegister::from((value & 0x00FF) as u8);
    }

    pub fn get_bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }
    pub fn set_bc(&mut self, value: u16) {
        self.b = ((value & 0xFF00) >> 8) as u8;
        self.c = (value & 0x00FF) as u8;
    }

    pub fn get_de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }
    pub fn set_de(&mut self, value: u16) {
        self.d = ((value & 0xFF00) >> 8) as u8;
        self.e = (value & 0x00FF) as u8;
    }

    pub fn get_hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }
    pub fn set_hl(&mut self, value: u16) {
        self.h = ((value & 0xFF00) >> 8) as u8;
        self.l = (value & 0x00FF) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_bc_registers() {
        let mut registers = Registers::new();
        let value = 0b1010_1100_0010_0101;
        registers.set_bc(value);
        assert_eq!(registers.b, 0b1010_1100u8);
        assert_eq!(registers.c, 0b0010_0101u8);
        assert_eq!(registers.get_bc(), value);
    }

    #[test]
    fn setting_de_registers() {
        let mut registers = Registers::new();
        let value = 0b1010_1100_0010_0101;
        registers.set_de(value);
        assert_eq!(registers.d, 0b1010_1100u8);
        assert_eq!(registers.e, 0b0010_0101u8);
        assert_eq!(registers.get_de(), value);
    }

    #[test]
    fn setting_hl_registers() {
        let mut registers = Registers::new();
        let value = 0b1010_1100_0010_0101;
        registers.set_hl(value);
        assert_eq!(registers.h, 0b1010_1100u8);
        assert_eq!(registers.l, 0b0010_0101u8);
        assert_eq!(registers.get_hl(), value);
    }

    #[test]
    fn setting_af_registers() {
        let mut registers = Registers::new();
        let value = 0b1010_1100_0010_0000;
        registers.set_af(value);
        let flags: u8 = registers.f.into();
        assert_eq!(registers.a, 0b1010_1100u8);
        assert_eq!(flags, 0b0010_0000u8);
        assert_eq!(registers.get_af(), value);
    }
    #[test]
    fn setting_f_as_u8() {
        let mut registers = Registers::new();
        let value = 0b1100_0000;
        registers.f = value.into();
        let result: u8 = registers.f.into();

        assert_eq!(result, value);
    }

    #[test]
    fn setting_f_from_struct() {
        let mut registers = Registers::new();
        let mut value = FlagsRegister::new();
        value.zero = true;
        value.carry = true;
        registers.f = value;
        let result: u8 = registers.f.into();
        let value: u8 = value.into();

        assert_eq!(result, value);
    }
}
