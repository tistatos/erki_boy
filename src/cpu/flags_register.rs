use std::fmt;

const ZERO_FLAG_BYTE: u8 = 7;
const SUBTRACT_FLAG_BYTE: u8 = 6;
const HALF_CARRY_FLAG_BYTE: u8 = 5;
const CARRY_FLAG_BYTE: u8 = 4;

#[derive(Copy, Clone, PartialEq)]
pub struct FlagsRegister {
    pub zero: bool,
    pub subtract: bool,
    pub half_carry: bool,
    pub carry: bool,
}

impl FlagsRegister {
    pub fn new() -> FlagsRegister {
        FlagsRegister {
            zero: false,
            subtract: false,
            half_carry: false,
            carry: false,
        }
    }
}

impl fmt::Display for FlagsRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut values = format!("");
        if self.zero { values += "Z"; } else { values += " ";}
        if self.subtract { values += "N"; } else { values += " ";}
        if self.half_carry { values += "H"; } else { values += " ";}
        if self.carry { values += "C"; } else { values += " ";}
        write!(f, "{}", values.trim())
    }
}
impl fmt::Debug for FlagsRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut values = format!("(");
        if self.zero { values += "Z"; } else { values += " ";}
        if self.subtract { values += "N"; } else { values += " ";}
        if self.half_carry { values += "H"; } else { values += " ";}
        if self.carry { values += "C"; } else { values += " ";}
        values += ")";

        write!(f, "{}", values)
    }
}

impl std::convert::From<FlagsRegister> for u8 {
    fn from(flag: FlagsRegister) -> u8 {
        (if flag.zero { 1 } else { 0 } << ZERO_FLAG_BYTE)
            | (if flag.subtract { 1 } else { 0 } << SUBTRACT_FLAG_BYTE)
            | (if flag.half_carry { 1 } else { 0 } << HALF_CARRY_FLAG_BYTE)
            | (if flag.carry { 1 } else { 0 } << CARRY_FLAG_BYTE)
    }
}

impl std::convert::From<u8> for FlagsRegister {
    fn from(byte: u8) -> FlagsRegister {
        let zero = ((byte >> ZERO_FLAG_BYTE) & 0x0b1) != 0;
        let subtract = ((byte >> SUBTRACT_FLAG_BYTE) & 0x0b1) != 0;
        let half_carry = ((byte >> HALF_CARRY_FLAG_BYTE) & 0x0b1) != 0;
        let carry = ((byte >> CARRY_FLAG_BYTE) & 0x0b1) != 0;

        FlagsRegister {
            zero,
            subtract,
            half_carry,
            carry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converting_to_u8() {
        let mut flags = FlagsRegister::new();
        flags.zero = true;
        flags.carry = true;
        let result: u8 = flags.into();
        assert_eq!(result, 0b1001_0000u8);
    }

    #[test]
    fn converting_from_u8() {
        let flags = 0b0101_0000u8;
        let result = FlagsRegister::from(flags);
        assert_eq!(result.subtract, true);
        assert_eq!(result.carry, true);
    }
}
