pub mod flags_register;
pub mod instruction;
pub mod registers;

use self::instruction::*;
use self::registers::Registers;
use crate::memory_bus::MemoryBus;
use crate::interrupts::{InterruptLocation};

#[derive(Debug, PartialEq)]
enum InterruptState {
    Enabled,
    Disabled,
    Enabling,
    Disabling,
}

pub struct CPU {
    is_halted: bool,
    interrupt_enabled: bool,
    pub pc: u16,
    pub sp: u16,
    pub registers: Registers,

    pub bus: MemoryBus,
}

impl CPU {
    pub fn new(boot_room: Option<Vec<u8>>, game_rom: Vec<u8>) -> CPU {
        CPU {
            is_halted: false,
            interrupt_enabled: true,
            bus: MemoryBus::new(boot_room, game_rom),
            pc: 0,
            sp: 0,
            registers: Registers::new()
        }
    }

    pub fn debug_output(&self) {
        let mut instruction_byte = self.bus.read_byte(self.pc);
        let mut output = format!("PC:0x{:04X}: ", self.pc);
        let prefixed = instruction_byte == 0xCB;
        if prefixed {
            output += "0xCB ";
            instruction_byte = self.read_next_byte();
        }
        if let Some(instruction) = Instruction::from_byte(instruction_byte, prefixed) {

            output += format!("{:?} (0x{:02X})", instruction, instruction_byte).as_str();
            let instruction_length = instruction.byte_length();
            if instruction_length == 2{
                output += format!(" {} ", self.read_next_byte()).as_str();

            }
            println!("{}\t {:?}, sp: 0x{:X}",
                    output, self.registers, self.sp);
        }
    }

    pub fn step(&mut self) -> u16 {
        let mut instruction_byte = self.bus.read_byte(self.pc);
        let prefixed = instruction_byte == 0xCB;
        if prefixed {
            instruction_byte = self.read_next_byte();
        }


        let (next_pc, mut cycles) = if let Some(instruction) = Instruction::from_byte(instruction_byte, prefixed)
        {
            self.execute(instruction)
        } else {
            let description = format!(
                "0x{}{:x}",
                if prefixed { "cb" } else { "" },
                instruction_byte
            );
            panic!("Unkown instruction found for: {}", description);
        };

        self.bus.step(cycles);
        if self.bus.interrupted() {
            self.is_halted = false;
        }
        if !self.is_halted {
            self.pc = next_pc; //By not increasing PC, we are essentially spinlocking here until the interrupt occurs
        }

        let mut interrupted = false;
        if self.interrupt_enabled {
            if self.bus.interrupts_enabled.vertical_blank_interrupt
                && self.bus.interrupt_flags.vertical_blank_interrupt {
                println!("VBlank interrupt");
                interrupted = true;
                self.bus.interrupt_flags.vertical_blank_interrupt = false;
                self.interrupt(InterruptLocation::VBlank);
            }
            if self.bus.interrupts_enabled.lcd_c_interrupt
                && self.bus.interrupt_flags.lcd_c_interrupt {
                println!("LCD interrupt");
                interrupted = true;
                self.bus.interrupt_flags.lcd_c_interrupt = false;
                self.interrupt(InterruptLocation::LCD);
            }
            if self.bus.interrupts_enabled.timer_interrupt
                && self.bus.interrupt_flags.timer_interrupt {
                println!("timer interrupt");
                interrupted = true;
                self.bus.interrupt_flags.timer_interrupt = false;
                self.interrupt(InterruptLocation::Timer);
            }
        }
        if interrupted {
            cycles += 12;
        }

        cycles
    }

    fn read_byte_at_hl(&self) -> u8 {
        self.bus.read_byte(self.registers.get_hl())
    }

    fn read_next_byte(&self) -> u8 {
        self.bus.read_byte(self.pc + 1)
    }

    fn read_next_word(&self) -> u16 {
        ((self.bus.read_byte(self.pc + 2) as u16) << 8) | (self.bus.read_byte(self.pc + 1) as u16)
    }

    fn write_byte_at_hl(&mut self, value: u8) {
        self.bus.write_byte(self.registers.get_hl(), value);
    }

    fn interrupt(&mut self, location: InterruptLocation) {
        self.interrupt_enabled = false;
        self.push(self.pc);
        self.pc = location as u16;
        self.bus.step(12);
    }

    fn jump(&mut self, should_jump: bool) -> (u16, u16) {
        if should_jump {
            let addr = self.read_next_word();
            (addr, 16)
        } else {
            (self.pc.wrapping_add(3), 12)
        }
    }

    fn restart(&mut self, address: RestartOffset) -> u16 {
        self.push(self.pc.wrapping_add(1));
        address.into()
    }

    fn jump_relative(&mut self, should_jump: bool) -> (u16, u16) {
        let next_pc = self.pc.wrapping_add(2);
        if should_jump {
            let relative_offset = self.read_next_byte() as i8;
            let pc = if relative_offset >= 0 {
                next_pc.wrapping_add(relative_offset as u16)
            } else {
                next_pc.wrapping_sub(relative_offset.abs() as u16)
            };
            return (pc, 12);
        } else {
            (next_pc, 8)
        }
    }


    fn push(&mut self, value: u16) {
        self.sp = self.sp.wrapping_sub(1);
        self.bus.write_byte(self.sp, ((value & 0xFF00) >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.bus.write_byte(self.sp, (value & 0xFF) as u8);
    }

    fn pop(&mut self) -> u16 {
        let lsb = self.bus.read_byte(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        let msb = self.bus.read_byte(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        (msb << 8) | lsb
    }

    fn call(&mut self, should_jump: bool) -> (u16, u16) {
        let next_pc = self.pc.wrapping_add(3);
        if should_jump {
            self.push(next_pc);
            (self.read_next_word(), 24)
        } else {
            (next_pc, 12)
        }
    }

    fn return_(&mut self, should_jump: bool) -> u16 {
        if should_jump {
            self.pop()
        } else {
            self.pc.wrapping_add(1)
        }
    }

    fn execute(&mut self, instruction: Instruction) -> (u16, u16) {
        match instruction {
            Instruction::NOP => (self.pc.wrapping_add(1), 4),
            Instruction::HALT => {
                self.is_halted = true;
                (self.pc.wrapping_add(1), 4)
            }
            Instruction::STOP => {
                if !self.is_halted {
                    println!("STOP called at 0x{:X}", self.pc+1);
                }
                self.is_halted = true; //FIXME: perhaps this should have its own state?
                (self.pc.wrapping_add(1), 4)
            }
            Instruction::EI => {
                self.interrupt_enabled = true;
                (self.pc.wrapping_add(1), 4)
            }
            Instruction::DI => {
                self.interrupt_enabled = false;
                (self.pc.wrapping_add(1), 4)
            }
            Instruction::RETI => {
                let pc = self.pop();
                self.interrupt_enabled = true;
                (pc, 16)
            }
            Instruction::RST(offset) => (self.restart(offset), 16),
            Instruction::POP(target) => {
                let result = self.pop();
                match target {
                    //FIXME should all flags be set on pop AF?
                    StackTarget::AF => self.registers.set_af(result),
                    StackTarget::BC => self.registers.set_bc(result),
                    StackTarget::DE => self.registers.set_de(result),
                    StackTarget::HL => self.registers.set_hl(result),
                }
                (self.pc.wrapping_add(1), 12)
            }
            Instruction::PUSH(target) => {
                let value = match target {
                    StackTarget::AF => self.registers.get_af(),
                    StackTarget::BC => self.registers.get_bc(),
                    StackTarget::DE => self.registers.get_de(),
                    StackTarget::HL => self.registers.get_hl(),
                };
                self.push(value);
                (self.pc.wrapping_add(1), 16)
            }

            Instruction::CALL(test) => {
                let jump_condition = match test {
                    JumpTest::NotZero => !self.registers.f.zero,
                    JumpTest::Zero => self.registers.f.zero,
                    JumpTest::Carry => self.registers.f.carry,
                    JumpTest::NotCarry => !self.registers.f.carry,
                    JumpTest::Always => true,
                };
                self.call(jump_condition)
            }

            Instruction::RET(test) => {
                let jump_condition = match test {
                    JumpTest::NotZero => !self.registers.f.zero,
                    JumpTest::Zero => self.registers.f.zero,
                    JumpTest::Carry => self.registers.f.carry,
                    JumpTest::NotCarry => !self.registers.f.carry,
                    JumpTest::Always => true,
                };
                let next_pc = self.return_(jump_condition);

                let cycles = if jump_condition && test == JumpTest::Always {
                    16
                } else if jump_condition {
                    20
                } else {
                    8
                };
                (next_pc, cycles)
            }

            Instruction::LD(load_type) => match load_type {
                LoadType::SPFromHL => {
                    self.sp = self.registers.get_hl();
                    (self.pc.wrapping_add(1), 8)
                }
                LoadType::HLFromSPN => {
                    let n = self.read_next_byte() as i8 as i16 as u16;
                    let value = self.sp.wrapping_add(n);
                    self.registers.set_hl(value);
                    self.registers.f.zero = false;
                    self.registers.f.subtract = false;
                    self.registers.f.half_carry =
                        (self.sp & 0xF) + (n & 0xF) > 0xF;
                    self.registers.f.carry =
                        (self.sp & 0xFF) + (n & 0xFF) > 0xFF;
                    (self.pc.wrapping_add(2), 12)
                }
                LoadType::IndirectFromSP => {
                    let address = self.read_next_word();
                    let sp = self.sp;
                    self.bus.write_byte(address, (sp & 0xFF) as u8);
                    self.bus
                        .write_byte(address.wrapping_add(1), ((sp & 0xFF00) >> 8) as u8);
                    (self.pc.wrapping_add(3), 20)
                }
                LoadType::Byte(target, source) => {
                    let source_value = match source {
                        LoadByteSource::A => self.registers.a,
                        LoadByteSource::B => self.registers.b,
                        LoadByteSource::C => self.registers.c,
                        LoadByteSource::D => self.registers.d,
                        LoadByteSource::E => self.registers.e,
                        LoadByteSource::H => self.registers.h,
                        LoadByteSource::L => self.registers.l,
                        LoadByteSource::D8 => self.read_next_byte(),
                        LoadByteSource::HLI => self.read_byte_at_hl(),
                    };
                    match target {
                        LoadByteTarget::A => self.registers.a = source_value,
                        LoadByteTarget::B => self.registers.b = source_value,
                        LoadByteTarget::C => self.registers.c = source_value,
                        LoadByteTarget::D => self.registers.d = source_value,
                        LoadByteTarget::E => self.registers.e = source_value,
                        LoadByteTarget::H => self.registers.h = source_value,
                        LoadByteTarget::L => self.registers.l = source_value,
                        LoadByteTarget::HLI => self.write_byte_at_hl(source_value),
                    };

                    match source {
                        LoadByteSource::D8 => (self.pc.wrapping_add(2), 8),
                        LoadByteSource::HLI => (self.pc.wrapping_add(1), 8),
                        _ => (self.pc.wrapping_add(1), 4),
                    }
                }
                LoadType::Word(target) => {
                    match target {
                        LoadWordTarget::BC => {
                            self.registers.set_bc(self.read_next_word());
                        }
                        LoadWordTarget::DE => {
                            self.registers.set_de(self.read_next_word());
                        }
                        LoadWordTarget::HL => {
                            self.registers.set_hl(self.read_next_word());
                        }
                        LoadWordTarget::SP => {
                            self.sp = self.read_next_word();
                        }
                    }
                    (self.pc.wrapping_add(3), 12)
                }
                LoadType::IndirectFromA(target) => {
                    match target {
                        Indirect::BC => self
                            .bus
                            .write_byte(self.registers.get_bc(), self.registers.a),
                        Indirect::DE => self
                            .bus
                            .write_byte(self.registers.get_de(), self.registers.a),
                        Indirect::HLPlus => {
                            let hl = self.registers.get_hl();
                            self.registers.set_hl(hl.wrapping_add(1));
                            self.bus.write_byte(hl, self.registers.a);
                        }
                        Indirect::HLMinus => {
                            let hl = self.registers.get_hl();
                            self.registers.set_hl(hl.wrapping_sub(1));
                            self.bus.write_byte(hl, self.registers.a);
                        }
                        Indirect::Word => {
                            self.bus.write_byte(
                                self.read_next_word(), self.registers.a)
                        }
                        Indirect::LastByte => {
                            self.bus
                                .write_byte(0xFF00 + self.registers.c as u16, self.registers.a);
                        }
                    }
                    match target {
                        Indirect::Word => (self.pc.wrapping_add(3), 16),
                        _ => (self.pc.wrapping_add(1), 8),
                    }
                }
                LoadType::AFromIndirect(target) => {
                    match target {
                        Indirect::BC => {
                            self.registers.a = self.bus.read_byte(self.registers.get_bc())
                        }
                        Indirect::DE => {
                            self.registers.a = self.bus.read_byte(self.registers.get_de())
                        }
                        Indirect::HLPlus => {
                            let hl = self.registers.get_hl();
                            self.registers.set_hl(hl.wrapping_add(1));
                            self.registers.a = self.bus.read_byte(hl);
                        }
                        Indirect::HLMinus => {
                            let hl = self.registers.get_hl();
                            self.registers.set_hl(hl.wrapping_sub(1));
                            self.registers.a = self.bus.read_byte(hl);
                        }
                        Indirect::Word => {
                            self.registers.a =
                                self.bus.read_byte(self.read_next_word())
                        }
                        Indirect::LastByte => {
                            self.registers.a = self.bus.read_byte(0xFF00 + self.registers.c as u16);
                        }
                    }
                    match target {
                        Indirect::Word => (self.pc.wrapping_add(3), 16),
                        _ => (self.pc.wrapping_add(1), 8),
                    }
                }
                LoadType::ByteAddressFromA => {
                    let address_offset = self.read_next_byte();
                    let address = 0xFF00 + address_offset as u16;
                    self.bus.write_byte(address, self.registers.a);
                    (self.pc.wrapping_add(2), 12)
                }
                LoadType::AFromByteAddress => {
                    let address_offset = self.read_next_byte();
                    let address = 0xFF00 + address_offset as u16;
                    self.registers.a = self.bus.read_byte(address);
                    (self.pc.wrapping_add(2), 12)
                }
            },

            Instruction::JP(test) => {
                let jump_condition = match test {
                    JumpTest::NotZero => !self.registers.f.zero,
                    JumpTest::NotCarry => !self.registers.f.carry,
                    JumpTest::Zero => self.registers.f.zero,
                    JumpTest::Carry => self.registers.f.carry,
                    JumpTest::Always => true,
                };
                self.jump(jump_condition)
            }
            Instruction::JR(test) => {
                let jump_condition = match test {
                    JumpTest::NotZero => !self.registers.f.zero,
                    JumpTest::NotCarry => !self.registers.f.carry,
                    JumpTest::Zero => self.registers.f.zero,
                    JumpTest::Carry => self.registers.f.carry,
                    JumpTest::Always => true,
                };
                self.jump_relative(jump_condition)
            }

            Instruction::JPHL => (self.registers.get_hl(), 4),

            Instruction::ADC(register) => {
                match register {
                    ArithmeticTarget::A => {
                        self.registers.a = self.add_with_carry(self.registers.a);
                    }
                    ArithmeticTarget::B => {
                        self.registers.a = self.add_with_carry(self.registers.b);
                    }
                    ArithmeticTarget::C => {
                        self.registers.a = self.add_with_carry(self.registers.c);
                    }
                    ArithmeticTarget::D => {
                        self.registers.a = self.add_with_carry(self.registers.d);
                    }
                    ArithmeticTarget::E => {
                        self.registers.a = self.add_with_carry(self.registers.e);
                    }
                    ArithmeticTarget::H => {
                        self.registers.a = self.add_with_carry(self.registers.h);
                    }
                    ArithmeticTarget::L => {
                        self.registers.a = self.add_with_carry(self.registers.l);
                    }
                    ArithmeticTarget::D8 => {
                        self.registers.a = self.add_with_carry(self.read_next_byte());
                    }
                    ArithmeticTarget::HLI => {
                        self.registers.a =
                            self.add_with_carry(self.read_byte_at_hl());
                    }
                }
                if register == ArithmeticTarget::D8 {
                    (self.pc.wrapping_add(2), 8)
                } else if register == ArithmeticTarget::HLI {
                    (self.pc.wrapping_add(1), 8)
                } else {
                    (self.pc.wrapping_add(1), 4)
                }
            }
            Instruction::ADD(register) => {
                match register {
                    ArithmeticTarget::A => {
                        self.registers.a = self.add_without_carry(self.registers.a);
                    }
                    ArithmeticTarget::B => {
                        self.registers.a = self.add_without_carry(self.registers.b);
                    }
                    ArithmeticTarget::C => {
                        self.registers.a = self.add_without_carry(self.registers.c);
                    }
                    ArithmeticTarget::D => {
                        self.registers.a = self.add_without_carry(self.registers.d);
                    }
                    ArithmeticTarget::E => {
                        self.registers.a = self.add_without_carry(self.registers.e);
                    }
                    ArithmeticTarget::H => {
                        self.registers.a = self.add_without_carry(self.registers.h);
                    }
                    ArithmeticTarget::L => {
                        self.registers.a = self.add_without_carry(self.registers.l);
                    }
                    ArithmeticTarget::D8 => {
                        self.registers.a = self.add_without_carry(self.read_next_byte());
                    }
                    ArithmeticTarget::HLI => {
                        self.registers.a = self.add_without_carry(self.read_byte_at_hl());
                    }
                }
                if register == ArithmeticTarget::D8 {
                    (self.pc.wrapping_add(2), 8)
                } else if register == ArithmeticTarget::HLI {
                    (self.pc.wrapping_add(1), 8)
                } else {
                    (self.pc.wrapping_add(1), 4)
                }
            }
            Instruction::ADDHL(register) => {
                let value = match register {
                    ArithmeticHLTarget::BC => self.registers.get_bc(),
                    ArithmeticHLTarget::DE => self.registers.get_de(),
                    ArithmeticHLTarget::HL => self.registers.get_hl(),
                    ArithmeticHLTarget::SP => self.sp,
                };
                let result = self.add_hl(value);
                self.registers.set_hl(result);

                (self.pc.wrapping_add(1), 8)
            }
            Instruction::ADDSP => {
                let value = self.read_next_byte() as i8 as i16 as u16;
                let result = self.sp.wrapping_add(value);

                self.registers.f.zero = false;
                self.registers.f.subtract = false;

                self.registers.f.half_carry =
                    (self.sp & 0xF) + (value & 0xF) > 0xF;
                self.registers.f.carry =
                    (self.sp & 0xFF) + (value & 0xFF) > 0xFF;

                self.sp = result;
                (self.pc.wrapping_add(2), 16)
            }
            Instruction::SUB(register) => {
                match register {
                    ArithmeticTarget::A => {
                        self.registers.a = self.sub_without_carry(self.registers.a);
                    }
                    ArithmeticTarget::B => {
                        self.registers.a = self.sub_without_carry(self.registers.b);
                    }
                    ArithmeticTarget::C => {
                        self.registers.a = self.sub_without_carry(self.registers.c);
                    }
                    ArithmeticTarget::D => {
                        self.registers.a = self.sub_without_carry(self.registers.d);
                    }
                    ArithmeticTarget::E => {
                        self.registers.a = self.sub_without_carry(self.registers.e);
                    }
                    ArithmeticTarget::H => {
                        self.registers.a = self.sub_without_carry(self.registers.h);
                    }
                    ArithmeticTarget::L => {
                        self.registers.a = self.sub_without_carry(self.registers.l);
                    }
                    ArithmeticTarget::D8 => {
                        self.registers.a = self.sub_without_carry(self.read_next_byte());
                    }
                    ArithmeticTarget::HLI => {
                        self.registers.a = self.sub_without_carry(self.read_byte_at_hl());
                    }
                }
                if register == ArithmeticTarget::D8 {
                    (self.pc.wrapping_add(2), 8)
                } else if register == ArithmeticTarget::HLI {
                    (self.pc.wrapping_add(1), 8)
                } else {
                    (self.pc.wrapping_add(1), 4)
                }
            }
            Instruction::SBC(register) => {
                match register {
                    ArithmeticTarget::A => {
                        self.registers.a = self.sub_with_carry(self.registers.a);
                    }
                    ArithmeticTarget::B => {
                        self.registers.a = self.sub_with_carry(self.registers.b);
                    }
                    ArithmeticTarget::C => {
                        self.registers.a = self.sub_with_carry(self.registers.c);
                    }
                    ArithmeticTarget::D => {
                        self.registers.a = self.sub_with_carry(self.registers.d);
                    }
                    ArithmeticTarget::E => {
                        self.registers.a = self.sub_with_carry(self.registers.e);
                    }
                    ArithmeticTarget::H => {
                        self.registers.a = self.sub_with_carry(self.registers.h);
                    }
                    ArithmeticTarget::L => {
                        self.registers.a = self.sub_with_carry(self.registers.l);
                    }
                    ArithmeticTarget::D8 => {
                        self.registers.a = self.sub_with_carry(self.read_next_byte());
                    }
                    ArithmeticTarget::HLI => {
                        self.registers.a =
                            self.sub_with_carry(self.read_byte_at_hl());
                    }
                }
                if register == ArithmeticTarget::D8 {
                    (self.pc.wrapping_add(2), 8)
                } else if register == ArithmeticTarget::HLI {
                    (self.pc.wrapping_add(1), 8)
                } else {
                    (self.pc.wrapping_add(1), 4)
                }
            }
            Instruction::AND(register) => {
                match register {
                    ArithmeticTarget::A => {
                        self.registers.a = self.and(self.registers.a);
                    }
                    ArithmeticTarget::B => {
                        self.registers.a = self.and(self.registers.b);
                    }
                    ArithmeticTarget::C => {
                        self.registers.a = self.and(self.registers.c);
                    }
                    ArithmeticTarget::D => {
                        self.registers.a = self.and(self.registers.d);
                    }
                    ArithmeticTarget::E => {
                        self.registers.a = self.and(self.registers.e);
                    }
                    ArithmeticTarget::H => {
                        self.registers.a = self.and(self.registers.h);
                    }
                    ArithmeticTarget::L => {
                        self.registers.a = self.and(self.registers.l);
                    }
                    ArithmeticTarget::D8 => {
                        self.registers.a = self.and(self.read_next_byte());
                    }
                    ArithmeticTarget::HLI => {
                        let result = self.and(self.read_byte_at_hl());
                        self.registers.a = result;
                    }
                }
                if register == ArithmeticTarget::D8 {
                    (self.pc.wrapping_add(2), 8)
                } else if register == ArithmeticTarget::HLI {
                    (self.pc.wrapping_add(1), 8)
                } else {
                    (self.pc.wrapping_add(1), 4)
                }
            }
            Instruction::OR(register) => {
                match register {
                    ArithmeticTarget::A => {
                        self.registers.a = self.or(self.registers.a);
                    }
                    ArithmeticTarget::B => {
                        self.registers.a = self.or(self.registers.b);
                    }
                    ArithmeticTarget::C => {
                        self.registers.a = self.or(self.registers.c);
                    }
                    ArithmeticTarget::D => {
                        self.registers.a = self.or(self.registers.d);
                    }
                    ArithmeticTarget::E => {
                        self.registers.a = self.or(self.registers.e);
                    }
                    ArithmeticTarget::H => {
                        self.registers.a = self.or(self.registers.h);
                    }
                    ArithmeticTarget::L => {
                        self.registers.a = self.or(self.registers.l);
                    }
                    ArithmeticTarget::D8 => {
                        self.registers.a = self.or(self.read_next_byte());
                    }
                    ArithmeticTarget::HLI => {
                        let result = self.or(self.read_byte_at_hl());
                        self.registers.a = result;
                    }
                }
                if register == ArithmeticTarget::D8 {
                    (self.pc.wrapping_add(2), 8)
                } else if register == ArithmeticTarget::HLI {
                    (self.pc.wrapping_add(1), 8)
                } else {
                    (self.pc.wrapping_add(1), 4)
                }
            }
            Instruction::XOR(register) => {
                match register {
                    ArithmeticTarget::A => {
                        self.registers.a = self.xor(self.registers.a);
                    }
                    ArithmeticTarget::B => {
                        self.registers.a = self.xor(self.registers.b);
                    }
                    ArithmeticTarget::C => {
                        self.registers.a = self.xor(self.registers.c);
                    }
                    ArithmeticTarget::D => {
                        self.registers.a = self.xor(self.registers.d);
                    }
                    ArithmeticTarget::E => {
                        self.registers.a = self.xor(self.registers.e);
                    }
                    ArithmeticTarget::H => {
                        self.registers.a = self.xor(self.registers.h);
                    }
                    ArithmeticTarget::L => {
                        self.registers.a = self.xor(self.registers.l);
                    }
                    ArithmeticTarget::D8 => {
                        self.registers.a = self.xor(self.read_next_byte());
                    }
                    ArithmeticTarget::HLI => {
                        let result = self.xor(self.read_byte_at_hl());
                        self.registers.a = result;
                    }
                }
                if register == ArithmeticTarget::D8 {
                    (self.pc.wrapping_add(2), 8)
                } else if register == ArithmeticTarget::HLI {
                    (self.pc.wrapping_add(1), 8)
                } else {
                    (self.pc.wrapping_add(1), 4)
                }
            }
            Instruction::CP(register) => {
                match register {
                    ArithmeticTarget::A => {
                        self.compare(self.registers.a);
                    }
                    ArithmeticTarget::B => {
                        self.compare(self.registers.b);
                    }
                    ArithmeticTarget::C => {
                        self.compare(self.registers.c);
                    }
                    ArithmeticTarget::D => {
                        self.compare(self.registers.d);
                    }
                    ArithmeticTarget::E => {
                        self.compare(self.registers.e);
                    }
                    ArithmeticTarget::H => {
                        self.compare(self.registers.h);
                    }
                    ArithmeticTarget::L => {
                        self.compare(self.registers.l);
                    }
                    ArithmeticTarget::D8 => {
                        self.compare(self.read_next_byte());
                    }
                    ArithmeticTarget::HLI => {
                        self.compare(self.read_byte_at_hl());
                    }
                }
                if register == ArithmeticTarget::D8 {
                    (self.pc.wrapping_add(2), 8)
                } else if register == ArithmeticTarget::HLI {
                    (self.pc.wrapping_add(1), 8)
                } else {
                    (self.pc.wrapping_add(1), 4)
                }
            }
            Instruction::INC(register) => match register {
                IncDecTarget::A => {
                    self.registers.a = self.increment_8bit(self.registers.a);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::B => {
                    self.registers.b = self.increment_8bit(self.registers.b);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::C => {
                    self.registers.c = self.increment_8bit(self.registers.c);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::D => {
                    self.registers.d = self.increment_8bit(self.registers.d);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::E => {
                    self.registers.e = self.increment_8bit(self.registers.e);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::H => {
                    self.registers.h = self.increment_8bit(self.registers.h);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::L => {
                    self.registers.l = self.increment_8bit(self.registers.l);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::BC => {
                    let new_value = self.increment_16bit(self.registers.get_bc());
                    self.registers.set_bc(new_value);
                    (self.pc.wrapping_add(1), 8)
                }
                IncDecTarget::DE => {
                    let new_value = self.increment_16bit(self.registers.get_de());
                    self.registers.set_de(new_value);
                    (self.pc.wrapping_add(1), 8)
                }
                IncDecTarget::HL => {
                    let new_value = self.increment_16bit(self.registers.get_hl());
                    self.registers.set_hl(new_value);
                    (self.pc.wrapping_add(1), 8)
                }
                IncDecTarget::HLI => {
                    let new_value = self.increment_8bit(self.read_byte_at_hl());
                    self.write_byte_at_hl(new_value);
                    (self.pc.wrapping_add(1), 12)
                }
                IncDecTarget::SP => {
                    self.sp = self.increment_16bit(self.sp);
                    (self.pc.wrapping_add(1), 8)
                }
            },
            Instruction::DEC(register) => match register {
                IncDecTarget::A => {
                    self.registers.a = self.decrement_8bit(self.registers.a);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::B => {
                    self.registers.b = self.decrement_8bit(self.registers.b);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::C => {
                    self.registers.c = self.decrement_8bit(self.registers.c);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::D => {
                    self.registers.d = self.decrement_8bit(self.registers.d);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::E => {
                    self.registers.e = self.decrement_8bit(self.registers.e);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::H => {
                    self.registers.h = self.decrement_8bit(self.registers.h);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::L => {
                    self.registers.l = self.decrement_8bit(self.registers.l);
                    (self.pc.wrapping_add(1), 4)
                }
                IncDecTarget::BC => {
                    let new_value = self.decrement_16bit(self.registers.get_bc());
                    self.registers.set_bc(new_value);
                    (self.pc.wrapping_add(1), 8)
                }
                IncDecTarget::DE => {
                    let new_value = self.decrement_16bit(self.registers.get_de());
                    self.registers.set_de(new_value);
                    (self.pc.wrapping_add(1), 8)
                }
                IncDecTarget::HL => {
                    let new_value = self.decrement_16bit(self.registers.get_hl());
                    self.registers.set_hl(new_value);
                    (self.pc.wrapping_add(1), 8)
                }
                IncDecTarget::HLI => {
                    let new_value = self.decrement_8bit(self.read_byte_at_hl());
                    self.write_byte_at_hl(new_value);
                    (self.pc.wrapping_add(1), 12)
                }
                IncDecTarget::SP => {
                    self.sp = self.decrement_16bit(self.sp);
                    (self.pc.wrapping_add(1), 8)
                }
            },
            Instruction::DAA => {
                let value = self.decimal_adjust(self.registers.a);
                self.registers.f.zero = value == 0;
                self.registers.f.half_carry = false;
                self.registers.a = value;
                (self.pc.wrapping_add(1), 4)
            }
            Instruction::CCF => {
                self.registers.f.subtract = false;
                self.registers.f.half_carry = false;
                self.registers.f.carry = !self.registers.f.carry;
                (self.pc.wrapping_add(1), 4)
            }
            Instruction::SCF => {
                self.registers.f.subtract = false;
                self.registers.f.half_carry = false;
                self.registers.f.carry = true;
                (self.pc.wrapping_add(1), 4)
            }
            Instruction::RRA => {
                self.registers.a = self.rotate_right_through_carry_retain_zero(self.registers.a);
                (self.pc.wrapping_add(1), 4)
            }
            Instruction::RLA => {
                self.registers.a = self.rotate_left_through_carry_retain_zero(self.registers.a);
                (self.pc.wrapping_add(1), 4)
            }
            Instruction::RRCA => {
                self.registers.a = self.rotate_right_retain_zero(self.registers.a);
                (self.pc.wrapping_add(1), 4)
            }
            Instruction::RLCA => {
                self.registers.a = self.rotate_left_retain_zero(self.registers.a);
                (self.pc.wrapping_add(1), 4)
            }
            Instruction::CPL => {
                self.registers.a = self.complement(self.registers.a);
                (self.pc.wrapping_add(1), 4)
            }
            Instruction::BIT(register, bit_position) => {
                match register {
                    PrefixTarget::A => self.bit_test(self.registers.a, bit_position),
                    PrefixTarget::B => self.bit_test(self.registers.b, bit_position),
                    PrefixTarget::C => self.bit_test(self.registers.c, bit_position),
                    PrefixTarget::D => self.bit_test(self.registers.d, bit_position),
                    PrefixTarget::E => self.bit_test(self.registers.e, bit_position),
                    PrefixTarget::H => self.bit_test(self.registers.h, bit_position),
                    PrefixTarget::L => self.bit_test(self.registers.l, bit_position),
                    PrefixTarget::HLI => self.bit_test(self.read_byte_at_hl(), bit_position),
                }
                match register {
                    PrefixTarget::HLI => (self.pc.wrapping_add(2), 16),
                    _ => (self.pc.wrapping_add(2), 8),
                }
            }
            Instruction::RES(register, bit_position) => {
                match register {
                    PrefixTarget::A => {
                        self.registers.a = self.bit_reset(self.registers.a, bit_position)
                    }
                    PrefixTarget::B => {
                        self.registers.b = self.bit_reset(self.registers.b, bit_position)
                    }
                    PrefixTarget::C => {
                        self.registers.c = self.bit_reset(self.registers.c, bit_position)
                    }
                    PrefixTarget::D => {
                        self.registers.d = self.bit_reset(self.registers.d, bit_position)
                    }
                    PrefixTarget::E => {
                        self.registers.e = self.bit_reset(self.registers.e, bit_position)
                    }
                    PrefixTarget::H => {
                        self.registers.h = self.bit_reset(self.registers.h, bit_position)
                    }
                    PrefixTarget::L => {
                        self.registers.l = self.bit_reset(self.registers.l, bit_position)
                    }
                    PrefixTarget::HLI => {
                        let result = self.bit_reset(self.read_byte_at_hl(), bit_position);
                        self.write_byte_at_hl(result);
                    }
                }
                match register {
                    PrefixTarget::HLI => (self.pc.wrapping_add(2), 16),
                    _ => (self.pc.wrapping_add(2), 8),
                }
            }
            Instruction::SET(register, bit_position) => {
                match register {
                    PrefixTarget::A => {
                        self.registers.a = self.bit_set(self.registers.a, bit_position)
                    }
                    PrefixTarget::B => {
                        self.registers.b = self.bit_set(self.registers.b, bit_position)
                    }
                    PrefixTarget::C => {
                        self.registers.c = self.bit_set(self.registers.c, bit_position)
                    }
                    PrefixTarget::D => {
                        self.registers.d = self.bit_set(self.registers.d, bit_position)
                    }
                    PrefixTarget::E => {
                        self.registers.e = self.bit_set(self.registers.e, bit_position)
                    }
                    PrefixTarget::H => {
                        self.registers.h = self.bit_set(self.registers.h, bit_position)
                    }
                    PrefixTarget::L => {
                        self.registers.l = self.bit_set(self.registers.l, bit_position)
                    }
                    PrefixTarget::HLI => {
                        let result = self.bit_set(self.read_byte_at_hl(), bit_position);
                        self.write_byte_at_hl(result);
                    }
                }
                match register {
                    PrefixTarget::HLI => (self.pc.wrapping_add(2), 16),
                    _ => (self.pc.wrapping_add(2), 8),
                }
            }
            Instruction::SRL(register) => {
                match register {
                    PrefixTarget::A => {
                        self.registers.a = self.shift_right_logical(self.registers.a)
                    }
                    PrefixTarget::B => {
                        self.registers.b = self.shift_right_logical(self.registers.b)
                    }
                    PrefixTarget::C => {
                        self.registers.c = self.shift_right_logical(self.registers.c)
                    }
                    PrefixTarget::D => {
                        self.registers.d = self.shift_right_logical(self.registers.d)
                    }
                    PrefixTarget::E => {
                        self.registers.e = self.shift_right_logical(self.registers.e)
                    }
                    PrefixTarget::H => {
                        self.registers.h = self.shift_right_logical(self.registers.h)
                    }
                    PrefixTarget::L => {
                        self.registers.l = self.shift_right_logical(self.registers.l)
                    }
                    PrefixTarget::HLI => {
                        let result = self.shift_right_logical(self.read_byte_at_hl());
                        self.write_byte_at_hl(result);
                    }
                }
                match register {
                    PrefixTarget::HLI => (self.pc.wrapping_add(2), 16),
                    _ => (self.pc.wrapping_add(2), 8),
                }
            }
            Instruction::RR(register) => {
                match register {
                    PrefixTarget::A => {
                        self.registers.a =
                            self.rotate_right_through_carry_set_zero(self.registers.a)
                    }
                    PrefixTarget::B => {
                        self.registers.b =
                            self.rotate_right_through_carry_set_zero(self.registers.b)
                    }
                    PrefixTarget::C => {
                        self.registers.c =
                            self.rotate_right_through_carry_set_zero(self.registers.c)
                    }
                    PrefixTarget::D => {
                        self.registers.d =
                            self.rotate_right_through_carry_set_zero(self.registers.d)
                    }
                    PrefixTarget::E => {
                        self.registers.e =
                            self.rotate_right_through_carry_set_zero(self.registers.e)
                    }
                    PrefixTarget::H => {
                        self.registers.h =
                            self.rotate_right_through_carry_set_zero(self.registers.h)
                    }
                    PrefixTarget::L => {
                        self.registers.l =
                            self.rotate_right_through_carry_set_zero(self.registers.l)
                    }
                    PrefixTarget::HLI => {
                        let result =
                            self.rotate_right_through_carry_set_zero(self.read_byte_at_hl());
                        self.write_byte_at_hl(result);
                    }
                }
                match register {
                    PrefixTarget::HLI => (self.pc.wrapping_add(2), 16),
                    _ => (self.pc.wrapping_add(2), 8),
                }
            }
            Instruction::RL(register) => {
                match register {
                    PrefixTarget::A => {
                        self.registers.a =
                            self.rotate_left_through_carry_set_zero(self.registers.a)
                    }
                    PrefixTarget::B => {
                        self.registers.b =
                            self.rotate_left_through_carry_set_zero(self.registers.b)
                    }
                    PrefixTarget::C => {
                        self.registers.c =
                            self.rotate_left_through_carry_set_zero(self.registers.c)
                    }
                    PrefixTarget::D => {
                        self.registers.d =
                            self.rotate_left_through_carry_set_zero(self.registers.d)
                    }
                    PrefixTarget::E => {
                        self.registers.e =
                            self.rotate_left_through_carry_set_zero(self.registers.e)
                    }
                    PrefixTarget::H => {
                        self.registers.h =
                            self.rotate_left_through_carry_set_zero(self.registers.h)
                    }
                    PrefixTarget::L => {
                        self.registers.l =
                            self.rotate_left_through_carry_set_zero(self.registers.l)
                    }
                    PrefixTarget::HLI => {
                        let result =
                            self.rotate_left_through_carry_set_zero(self.read_byte_at_hl());
                        self.write_byte_at_hl(result);
                    }
                }
                match register {
                    PrefixTarget::HLI => (self.pc.wrapping_add(2), 16),
                    _ => (self.pc.wrapping_add(2), 8),
                }
            }
            Instruction::RRC(register) => {
                match register {
                    PrefixTarget::A => {
                        self.registers.a = self.rotate_right_set_zero(self.registers.a)
                    }
                    PrefixTarget::B => {
                        self.registers.b = self.rotate_right_set_zero(self.registers.b)
                    }
                    PrefixTarget::C => {
                        self.registers.c = self.rotate_right_set_zero(self.registers.c)
                    }
                    PrefixTarget::D => {
                        self.registers.d = self.rotate_right_set_zero(self.registers.d)
                    }
                    PrefixTarget::E => {
                        self.registers.e = self.rotate_right_set_zero(self.registers.e)
                    }
                    PrefixTarget::H => {
                        self.registers.h = self.rotate_right_set_zero(self.registers.h)
                    }
                    PrefixTarget::L => {
                        self.registers.l = self.rotate_right_set_zero(self.registers.l)
                    }
                    PrefixTarget::HLI => {
                        let result = self.rotate_right_set_zero(self.read_byte_at_hl());
                        self.write_byte_at_hl(result);
                    }
                }
                match register {
                    PrefixTarget::HLI => (self.pc.wrapping_add(2), 16),
                    _ => (self.pc.wrapping_add(2), 8),
                }
            }
            Instruction::RLC(register) => {
                match register {
                    PrefixTarget::A => {
                        self.registers.a = self.rotate_left_set_zero(self.registers.a)
                    }
                    PrefixTarget::B => {
                        self.registers.b = self.rotate_left_set_zero(self.registers.b)
                    }
                    PrefixTarget::C => {
                        self.registers.c = self.rotate_left_set_zero(self.registers.c)
                    }
                    PrefixTarget::D => {
                        self.registers.d = self.rotate_left_set_zero(self.registers.d)
                    }
                    PrefixTarget::E => {
                        self.registers.e = self.rotate_left_set_zero(self.registers.e)
                    }
                    PrefixTarget::H => {
                        self.registers.h = self.rotate_left_set_zero(self.registers.h)
                    }
                    PrefixTarget::L => {
                        self.registers.l = self.rotate_left_set_zero(self.registers.l)
                    }
                    PrefixTarget::HLI => {
                        let result = self.rotate_left_set_zero(self.read_byte_at_hl());
                        self.write_byte_at_hl(result);
                    }
                }
                match register {
                    PrefixTarget::HLI => (self.pc.wrapping_add(2), 16),
                    _ => (self.pc.wrapping_add(2), 8),
                }
            }
            Instruction::SRA(register) => {
                match register {
                    PrefixTarget::A => {
                        self.registers.a = self.shift_right_arithmetic(self.registers.a)
                    }
                    PrefixTarget::B => {
                        self.registers.b = self.shift_right_arithmetic(self.registers.b)
                    }
                    PrefixTarget::C => {
                        self.registers.c = self.shift_right_arithmetic(self.registers.c)
                    }
                    PrefixTarget::D => {
                        self.registers.d = self.shift_right_arithmetic(self.registers.d)
                    }
                    PrefixTarget::E => {
                        self.registers.e = self.shift_right_arithmetic(self.registers.e)
                    }
                    PrefixTarget::H => {
                        self.registers.h = self.shift_right_arithmetic(self.registers.h)
                    }
                    PrefixTarget::L => {
                        self.registers.l = self.shift_right_arithmetic(self.registers.l)
                    }
                    PrefixTarget::HLI => {
                        let result = self.shift_right_arithmetic(self.read_byte_at_hl());
                        self.write_byte_at_hl(result);
                    }
                }
                match register {
                    PrefixTarget::HLI => (self.pc.wrapping_add(2), 16),
                    _ => (self.pc.wrapping_add(2), 8),
                }
            }
            Instruction::SLA(register) => {
                match register {
                    PrefixTarget::A => {
                        self.registers.a = self.shift_left_arithmetic(self.registers.a)
                    }
                    PrefixTarget::B => {
                        self.registers.b = self.shift_left_arithmetic(self.registers.b)
                    }
                    PrefixTarget::C => {
                        self.registers.c = self.shift_left_arithmetic(self.registers.c)
                    }
                    PrefixTarget::D => {
                        self.registers.d = self.shift_left_arithmetic(self.registers.d)
                    }
                    PrefixTarget::E => {
                        self.registers.e = self.shift_left_arithmetic(self.registers.e)
                    }
                    PrefixTarget::H => {
                        self.registers.h = self.shift_left_arithmetic(self.registers.h)
                    }
                    PrefixTarget::L => {
                        self.registers.l = self.shift_left_arithmetic(self.registers.l)
                    }
                    PrefixTarget::HLI => {
                        let result = self.shift_left_arithmetic(self.read_byte_at_hl());
                        self.write_byte_at_hl(result);
                    }
                }
                match register {
                    PrefixTarget::HLI => (self.pc.wrapping_add(2), 16),
                    _ => (self.pc.wrapping_add(2), 8),
                }
            }
            Instruction::SWAP(register) => {
                match register {
                    PrefixTarget::A => self.registers.a = self.swap_nibble(self.registers.a),
                    PrefixTarget::B => self.registers.b = self.swap_nibble(self.registers.b),
                    PrefixTarget::C => self.registers.c = self.swap_nibble(self.registers.c),
                    PrefixTarget::D => self.registers.d = self.swap_nibble(self.registers.d),
                    PrefixTarget::E => self.registers.e = self.swap_nibble(self.registers.e),
                    PrefixTarget::H => self.registers.h = self.swap_nibble(self.registers.h),
                    PrefixTarget::L => self.registers.l = self.swap_nibble(self.registers.l),
                    PrefixTarget::HLI => {
                        let result = self.swap_nibble(self.read_byte_at_hl());
                        self.write_byte_at_hl(result);
                    }
                }
                match register {
                    PrefixTarget::HLI => (self.pc.wrapping_add(2), 16),
                    _ => (self.pc.wrapping_add(2), 8),
                }
            }
        }
    }

    fn add_hl(&mut self, value: u16) -> u16 {
        let hl = self.registers.get_hl();
        let (result, did_overflow) = hl.overflowing_add(value);
        self.registers.f.carry = did_overflow;
        self.registers.f.subtract = false;
        const CARRY_MASK: u16 = 0b111_1111_1111;

        self.registers.f.half_carry = (value & CARRY_MASK) + (hl & CARRY_MASK) > CARRY_MASK;

        result
    }

    fn add_without_carry(&mut self, value: u8) -> u8 {
        self.add(value, false)
    }
    fn add_with_carry(&mut self, value: u8) -> u8 {
        self.add(value, true)
    }

    fn add(&mut self, value: u8, add_carry: bool) -> u8 {
        let additional_carry = if add_carry && self.registers.f.carry {
            1
        } else {
            0
        };
        let (add, did_overflow) = self.registers.a.overflowing_add(value);
        let (add2, did_overflow2) = add.overflowing_add(additional_carry);
        self.registers.f.zero = add2 == 0;
        self.registers.f.subtract = false;
        self.registers.f.carry = did_overflow || did_overflow2;
        self.registers.f.half_carry =
            (self.registers.a & 0xF) + (value & 0xF) + additional_carry > 0xF;
        add2
    }

    fn bit_test(&mut self, value: u8, bit_position: BitPosition) {
        let bit_position: u8 = bit_position.into();
        let result = (value >> bit_position) & 0b1;
        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = true;
    }

    fn decimal_adjust(&mut self, value: u8) -> u8 {

        let mut carry = false;

        let result = if !self.registers.f.subtract {
            let mut result = value;
            if self.registers.f.carry || value > 0x99 {
                result = result.wrapping_add(0x60);
                carry = true;
            }
            if self.registers.f.half_carry || (value & 0x0F) > 0x09 {
                result = result.wrapping_add(0x06);
            }
            result
        } else if self.registers.f.carry {
            carry = true;
            let add = if self.registers.f.half_carry { 0x9A } else { 0xA0 };
            value.wrapping_add(add)
        }
        else if self.registers.f.half_carry {
            value.wrapping_add(0xFA)
        }
        else {
            value
        };

        self.registers.f.carry = carry;
        result
    }

    fn bit_reset(&mut self, value: u8, bit_position: BitPosition) -> u8 {
        let bit_position: u8 = bit_position.into();
        value & !(1 << bit_position)
    }

    fn bit_set(&mut self, value: u8, bit_position: BitPosition) -> u8 {
        let bit_position: u8 = bit_position.into();
        value | (1 << bit_position)
    }

    fn sub_with_carry(&mut self, value: u8) -> u8 {
        self.sub(value, true)
    }

    fn sub_without_carry(&mut self, value: u8) -> u8 {
        self.sub(value, false)
    }

    fn sub(&mut self, value: u8, sub_carry: bool) -> u8 {
        let additional_carry = if sub_carry && self.registers.f.carry {
            1
        } else {
            0
        };
        let (sub, did_overflow) = self.registers.a.overflowing_sub(value);
        let (sub2, did_overflow2) = sub.overflowing_sub(additional_carry);
        self.registers.f.zero = sub2 == 0;
        self.registers.f.subtract = true;
        self.registers.f.carry = did_overflow || did_overflow2;
        self.registers.f.half_carry = (self.registers.a & 0xF) < (value & 0xF) + additional_carry;
        sub2
    }

    fn and(&mut self, value: u8) -> u8 {
        let result = self.registers.a & value;
        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = true;
        self.registers.f.carry = false;
        result
    }

    fn or(&mut self, value: u8) -> u8 {
        let result = self.registers.a | value;
        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = false;
        result
    }

    fn xor(&mut self, value: u8) -> u8 {
        let result = self.registers.a ^ value;
        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = false;
        result
    }

    fn compare(&mut self, value: u8) {
        self.registers.f.zero = self.registers.a == value;
        self.registers.f.subtract = true;
        self.registers.f.half_carry = (self.registers.a & 0xF) < (value & 0xF);
        self.registers.f.carry = self.registers.a < value;
    }

    fn complement(&mut self, value: u8) -> u8 {
        let new_value = !value;

        self.registers.f.subtract = true;
        self.registers.f.half_carry = true;

        new_value
    }

    fn increment_8bit(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = value & 0xF == 0xF;
        result
    }

    fn increment_16bit(&mut self, value: u16) -> u16 {
        value.wrapping_add(1)
    }

    fn decrement_8bit(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        self.registers.f.zero = result == 0;
        self.registers.f.subtract = true;
        self.registers.f.half_carry = value & 0xF == 0x0;
        result
    }

    fn decrement_16bit(&mut self, value: u16) -> u16 {
        value.wrapping_sub(1)
    }

    fn rotate_right_through_carry_retain_zero(&mut self, value: u8) -> u8 {
        self.rotate_right_through_carry(value, false)
    }

    fn rotate_right_through_carry_set_zero(&mut self, value: u8) -> u8 {
        self.rotate_right_through_carry(value, true)
    }

    fn rotate_left_through_carry_set_zero(&mut self, value: u8) -> u8 {
        self.rotate_left_through_carry(value, true)
    }

    fn rotate_left_through_carry_retain_zero(&mut self, value: u8) -> u8 {
        self.rotate_left_through_carry(value, false)
    }

    fn rotate_right_through_carry(&mut self, value: u8, set_zero: bool) -> u8 {
        let carry_bit = if self.registers.f.carry { 1 } else { 0 } << 7;
        let new_value = carry_bit | (value >> 1);

        self.registers.f.zero = set_zero && new_value == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = value & 0b1 == 0b1;

        new_value
    }

    fn rotate_left_through_carry(&mut self, value: u8, set_zero: bool) -> u8 {
        let carry_bit = if self.registers.f.carry { 1 } else { 0 };
        let new_value = (value << 1) | carry_bit;

        self.registers.f.zero = set_zero && new_value == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (value & 0x80) == 0x80;

        new_value
    }

    fn rotate_left_retain_zero(&mut self, value: u8) -> u8 {
        self.rotate_left(value, false)
    }

    fn rotate_left_set_zero(&mut self, value: u8) -> u8 {
        self.rotate_left(value, true)
    }

    fn rotate_right_retain_zero(&mut self, value: u8) -> u8 {
        self.rotate_right(value, false)
    }

    fn rotate_right_set_zero(&mut self, value: u8) -> u8 {
        self.rotate_right(value, true)
    }

    fn rotate_right(&mut self, value: u8, set_zero: bool) -> u8 {
        let new_value = value.rotate_right(1);

        self.registers.f.zero = set_zero && new_value == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = value & 0b1 == 0b1;

        new_value
    }

    fn rotate_left(&mut self, value: u8, set_zero: bool) -> u8 {
        let carry = (value & 0x80) >> 7;
        let new_value = value.rotate_left(1) | carry;

        self.registers.f.zero = set_zero && new_value == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = carry == 0x01;
        new_value
    }

    fn shift_right_logical(&mut self, value: u8) -> u8 {
        let new_value = value >> 1;

        self.registers.f.zero = new_value == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = value & 0b1 == 0b1;

        new_value
    }

    fn shift_left_arithmetic(&mut self, value: u8) -> u8 {
        let new_value = value << 1;

        self.registers.f.zero = new_value == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = value & 0x80 == 0x80;

        new_value
    }

    fn shift_right_arithmetic(&mut self, value: u8) -> u8 {
        let msb = value & 0x80;
        let new_value = msb | value >> 1;

        self.registers.f.zero = new_value == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = value & 0b1 == 0b1;

        new_value
    }

    fn swap_nibble(&mut self, value: u8) -> u8 {
        let new_value = ((value & 0xf) << 4) | ((value & 0xf0) >> 4);
        self.registers.f.zero = new_value == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = false;

        new_value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod instructions {
        use super::*;

        //Special instructions
        #[test]
        fn nop() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.bus.write_byte(0, 0x00);
            cpu.step();
            assert_eq!(cpu.pc, 1);
        }

        #[test]
        fn enable_interrupt() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.interrupt_enabled = false;
            cpu.bus.write_byte(0, 0xFB);
            cpu.bus.write_byte(1, 0x00);
            cpu.step();
            assert_eq!(cpu.interrupt_enabled, true);
            cpu.step();
            assert_eq!(cpu.interrupt_enabled, true);
            assert_eq!(cpu.pc, 2);
        }

        #[test]
        fn disable_interrupt() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.bus.write_byte(0, 0xF3);
            cpu.bus.write_byte(1, 0x00);
            cpu.step();
            assert_eq!(cpu.interrupt_enabled, false);
            cpu.step();
            assert_eq!(cpu.interrupt_enabled,  false);
            assert_eq!(cpu.pc, 2);
        }

        #[test]
        fn restart() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.pc = 100;
            cpu.sp = 0x10;
            cpu.bus.write_byte(100, 0xDF);
            cpu.step();
            assert_eq!(cpu.pc, 0x18);
            assert_eq!(cpu.sp, 0x0E);
            assert_eq!(cpu.bus.read_byte(cpu.sp), 0x65);
            assert_eq!(cpu.bus.read_byte(cpu.sp + 2), 0x00);
        }

        #[test]
        fn return_enable_interrupt() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.pc = 100;
            cpu.sp = 0x10;
            cpu.bus.write_byte(100, 0xD9);
            cpu.bus.write_byte(0x10, 0x01);
            cpu.bus.write_byte(0x11, 0x05);
            cpu.step();
            assert_eq!(cpu.interrupt_enabled, true);
            assert_eq!(cpu.sp, 0x12);
            assert_eq!(cpu.pc, 0x0501);
        }

        #[test]
        fn decimal_adjust() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 0b0000_0101 + 0b0000_0101; // 5 + 5 inBCD
            cpu.execute(Instruction::DAA);
            assert_eq!(cpu.registers.a, 0b0001_0000);
            cpu.registers.a = 0b0001_0110 + 0b0001_0110; // 16+16 in BCD
            cpu.execute(Instruction::DAA);
            assert_eq!(cpu.registers.a, 0b0011_0010);
        }

        //LD on 16 bit registers
        #[test]
        fn load_word_into_16bit_register() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.bus.write_byte(0, 0x01); //LD BC d16
            cpu.bus.write_byte(1, 0x11);
            cpu.bus.write_byte(2, 0x01);
            cpu.step();
            assert_eq!(cpu.registers.get_bc(), 0x0111);
        }

        #[test]
        fn load_16bit_value_to_address_at_bc_from_a() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 5;
            cpu.registers.set_bc(0x04);
            cpu.bus.write_byte(0, 0x02); //LD BC A
            cpu.step();
            assert_eq!(cpu.bus.read_byte(0x04), 5);
        }

        #[test]
        fn load_16bit_value_to_address_at_hl_from_a() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 5;
            cpu.registers.set_hl(0x04);
            cpu.bus.write_byte(0, 0x22); // LD HL+ A
            cpu.step();
            assert_eq!(cpu.bus.read_byte(0x04), 5);
            assert_eq!(cpu.registers.get_hl(), 5);
        }

        #[test]
        fn load_16bit_value_to_a_from_address_from_a() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.set_bc(0x04);
            cpu.bus.write_byte(0, 0x0A); // LD A BC
            cpu.bus.write_byte(4, 0x0A);
            cpu.step();
            assert_eq!(cpu.registers.a, 10);
        }

        //LD 8 bit
        #[test]
        fn load_8bit_value_to_b() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.bus.write_byte(0, 0x06);
            cpu.bus.write_byte(1, 0x19);
            cpu.step();
            assert_eq!(cpu.pc, 2);
            assert_eq!(cpu.registers.b, 25);
        }

        #[test]
        fn load_value_from_b_to_c() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 15;
            cpu.bus.write_byte(0, 0x48);
            cpu.step();
            assert_eq!(cpu.pc, 1);
            assert_eq!(cpu.registers.b, 15);
            assert_eq!(cpu.registers.c, 15);
        }

        #[test]
        fn load_value_from_address_in_hl_to_e() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.bus.write_byte(0, 0x5E);
            cpu.bus.write_byte(3, 0x48);
            cpu.registers.set_hl(3);
            cpu.step();
            assert_eq!(cpu.registers.e, 0x48);
        }
        #[test]
        fn load_value_to_address_in_hl_from_e() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.e = 5;
            cpu.bus.write_byte(0, 0x73);
            cpu.registers.set_hl(3);
            cpu.step();
            assert_eq!(cpu.bus.read_byte(3), 5);
        }

        //Load byte address
        #[test]
        fn load_byte_address_from_a() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 101;
            cpu.bus.write_byte(0, 0xE0);
            cpu.bus.write_byte(1, 0x8D);
            cpu.step();
            assert_eq!(cpu.bus.read_byte(0xFF8D), 101);
        }

        #[test]
        fn load_a_from_byte_address() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.bus.write_byte(0, 0xF0);
            cpu.bus.write_byte(1, 0x8D);
            cpu.bus.write_byte(0xFF8D, 123);
            cpu.step();
            assert_eq!(cpu.registers.a, 123);
        }

        //Load last byte
        #[test]
        fn load_a_from_address_last_byte_in_c() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.bus.write_byte(0, 0xF2);
            cpu.bus.write_byte(0xFF85, 123);
            cpu.registers.c = 0x85;
            cpu.step();
            assert_eq!(cpu.registers.a, 123);
        }

        #[test]
        fn load_address_with_last_byte_in_c_from_a() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 101;
            cpu.bus.write_byte(0, 0xE2);
            cpu.registers.c = 0x85;
            cpu.step();
            assert_eq!(cpu.bus.read_byte(0xFF85), 101);
        }

        #[test]
        fn load_hl_with_sp_and_byte() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.sp = 0x10;
            cpu.bus.write_byte(0, 0xF8);
            cpu.bus.write_byte(1, 0xE2);
            cpu.step();
            assert_eq!(cpu.registers.get_hl(), 0xF2);
        }

        // CALL
        #[test]
        fn call() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.sp = 0x10;
            cpu.bus.write_byte(0, 0xCC); //Jump if zero
            cpu.bus.write_byte(3, 0xC4); //jump if not zero
            cpu.bus.write_byte(4, 0x14);
            cpu.bus.write_byte(5, 0x00);
            cpu.step();
            assert_eq!(cpu.pc, 3);
            cpu.step();
            assert_eq!(cpu.pc, 20);
            assert_eq!(cpu.sp, 0x0E);
        }

        //RET
        #[test]
        fn ret() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.sp = 0x10;
            cpu.bus.write_byte(0, 0xC4); //jump if not zero
            cpu.bus.write_byte(1, 0x14);
            cpu.bus.write_byte(2, 0x00);
            cpu.bus.write_byte(20, 0x00);
            cpu.bus.write_byte(21, 0xC9);
            cpu.step();
            assert_eq!(cpu.pc, 20);
            assert_eq!(cpu.sp, 0x0E);
            cpu.step();
            assert_eq!(cpu.pc, 21);
            cpu.step();
            assert_eq!(cpu.pc, 3);
            assert_eq!(cpu.sp, 0x10);
        }

        //PUSH & POP
        #[test]
        fn push_and_pop() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.sp = 0x10;
            cpu.bus.write_byte(0, 0xC5);
            cpu.bus.write_byte(1, 0xD1);
            cpu.registers.b = 0x4;
            cpu.registers.c = 0x89;
            cpu.step();

            assert_eq!(cpu.bus.read_byte(0x0F), 0x04);
            assert_eq!(cpu.bus.read_byte(0x0E), 0x89);
            assert_eq!(cpu.sp, 0x0E);
            assert_eq!(cpu.pc, 1);
            cpu.step();
            assert_eq!(cpu.pc, 2);
            assert_eq!(cpu.registers.d, 0x4);
            assert_eq!(cpu.registers.e, 0x89);
            assert_eq!(cpu.sp, 0x10);
        }

        //JP
        #[test]
        fn jump() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.bus.write_byte(0, 0x00);
            cpu.bus.write_byte(1, 0xC3); //JP always
            cpu.bus.write_byte(2, 0x01);
            cpu.bus.write_byte(3, 0x02);
            cpu.step();
            assert_eq!(cpu.pc, 1);
            cpu.step();
            assert_eq!(cpu.pc, 513);

            cpu.pc = 0;
            cpu.registers.f.zero = true;
            cpu.bus.write_byte(1, 0xCA); //JP Zero
            cpu.bus.write_byte(2, 0x01);
            cpu.bus.write_byte(3, 0x02);
            cpu.step();
            assert_eq!(cpu.pc, 1);
            cpu.step();
            assert_eq!(cpu.pc, 513);
            cpu.pc = 0;
            cpu.registers.f.zero = false;
            cpu.step();
            cpu.step();
            assert_eq!(cpu.pc, 4);
        }

        #[test]
        fn jump_hl() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.set_hl(412);
            cpu.bus.write_byte(0, 0xE9);
            cpu.step();
            assert_eq!(cpu.pc, 412);
        }

        #[test]
        fn jump_relative() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.bus.write_byte(0, 0x18); //JR always
            cpu.bus.write_byte(1, 0x09);
            cpu.step();
            assert_eq!(cpu.pc, 11);
            cpu.bus.write_byte(11, 0x18); //JR always
            cpu.bus.write_byte(12, 255 - 5);
            cpu.step();
            assert_eq!(cpu.pc, 7);
        }

        // ADD tests
        #[test]
        fn add_instruction() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 2;
            cpu.registers.c = 4;
            cpu.execute(Instruction::ADD(ArithmeticTarget::C));
            assert_eq!(cpu.registers.a, 6);
        }

        #[test]
        fn add_byte_instruction() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 2;
            cpu.bus.write_byte(0, 0xC6);
            cpu.bus.write_byte(1, 0x01);
            cpu.step();
            assert_eq!(cpu.registers.a, 3);
        }

        #[test]
        fn add_caused_overflow() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 254;
            cpu.registers.c = 3;
            cpu.execute(Instruction::ADD(ArithmeticTarget::C));
            assert_eq!(cpu.registers.f.carry, true);
            assert_eq!(cpu.registers.a, 1);
        }

        #[test]
        fn add_caused_half_carry() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 15;
            cpu.registers.c = 4;
            cpu.execute(Instruction::ADD(ArithmeticTarget::C));
            assert_eq!(cpu.registers.f.half_carry, true);
            assert_eq!(cpu.registers.a, 19);
        }

        #[test]
        fn add_was_zero() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 0;
            cpu.registers.c = 0;
            cpu.execute(Instruction::ADD(ArithmeticTarget::C));
            assert_eq!(cpu.registers.f.zero, true);
            assert_eq!(cpu.registers.a, 0);
            cpu.registers.a = 255;
            cpu.registers.c = 1;
            cpu.execute(Instruction::ADD(ArithmeticTarget::C));
            assert_eq!(cpu.registers.f.zero, true);
            assert_eq!(cpu.registers.f.carry, true);
            assert_eq!(cpu.registers.a, 0);
        }

        #[test]
        fn add_sp() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.sp = 0x10;
            cpu.bus.write_byte(0, 0xE8);
            cpu.bus.write_byte(1, 0x10);
            cpu.step();
            assert_eq!(cpu.sp, 0x20);
        }

        //ADDHL
        #[test]
        fn addhl() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.set_hl(300);
            cpu.registers.set_bc(400);
            cpu.execute(Instruction::ADDHL(ArithmeticHLTarget::BC));
            assert_eq!(cpu.registers.get_hl(), 700);
        }

        #[test]
        fn addhl_caused_half_carry() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.set_hl(2023);
            cpu.registers.set_bc(101);
            cpu.execute(Instruction::ADDHL(ArithmeticHLTarget::BC));
            assert_eq!(cpu.registers.get_hl(), 2124);
            assert_eq!(cpu.registers.f.half_carry, true);
        }
        #[test]
        fn addhl_caused_overflow() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.set_hl(65500);
            cpu.registers.set_bc(100);
            cpu.execute(Instruction::ADDHL(ArithmeticHLTarget::BC));
            assert_eq!(cpu.registers.get_hl(), 64);
            assert_eq!(cpu.registers.f.carry, true);
        }

        //ADDC

        #[test]
        fn addc() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 0b10;
            cpu.registers.b = 0b100;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::ADC(ArithmeticTarget::B));
            assert_eq!(cpu.registers.a, 7);
            assert_eq!(cpu.registers.f.carry, false);
            cpu.registers.a = 2;
            cpu.registers.b = 4;
            cpu.registers.f.carry = false;
            cpu.execute(Instruction::ADC(ArithmeticTarget::B));
            assert_eq!(cpu.registers.a, 6);
        }
        #[test]
        fn addc_caused_half_carry() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 12;
            cpu.registers.c = 4;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::ADC(ArithmeticTarget::C));
            assert_eq!(cpu.registers.a, 17);
            assert_eq!(cpu.registers.f.half_carry, true);
        }
        #[test]
        fn addc_caused_overflow() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 251;
            cpu.registers.c = 4;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::ADC(ArithmeticTarget::C));
            assert_eq!(cpu.registers.a, 0);
            assert_eq!(cpu.registers.f.carry, true);
        }

        //SUB
        #[test]
        fn sub() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 4;
            cpu.registers.c = 2;
            cpu.execute(Instruction::SUB(ArithmeticTarget::C));
            assert_eq!(cpu.registers.a, 2);
            assert_eq!(cpu.registers.f.subtract, true);
        }
        #[test]
        fn sub_caused_half_carry() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 17;
            cpu.registers.c = 4;
            cpu.execute(Instruction::SUB(ArithmeticTarget::C));
            assert_eq!(cpu.registers.a, 13);
            assert_eq!(cpu.registers.f.half_carry, true);
        }
        #[test]
        fn sub_caused_overflow() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 2;
            cpu.registers.c = 4;
            cpu.execute(Instruction::SUB(ArithmeticTarget::C));
            assert_eq!(cpu.registers.a, 254);
            assert_eq!(cpu.registers.f.carry, true);
        }

        //SBC

        #[test]
        fn sbc() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 4;
            cpu.registers.c = 2;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::SBC(ArithmeticTarget::C));
            assert_eq!(cpu.registers.a, 1);
            assert_eq!(cpu.registers.f.subtract, true);
        }

        #[test]
        fn sbc_caused_half_carry() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 20;
            cpu.registers.c = 4;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::SBC(ArithmeticTarget::C));
            assert_eq!(cpu.registers.a, 15);
            assert_eq!(cpu.registers.f.half_carry, true);
        }

        #[test]
        fn sbc_caused_overflow() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 2;
            cpu.registers.c = 2;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::SBC(ArithmeticTarget::C));
            assert_eq!(cpu.registers.a, 255);
            assert_eq!(cpu.registers.f.carry, true);
        }

        //AND

        #[test]
        fn and() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 3;
            cpu.registers.c = 2;
            cpu.execute(Instruction::AND(ArithmeticTarget::C));
            assert_eq!(cpu.registers.a, 2);
        }

        //OR

        #[test]
        fn or() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 3;
            cpu.registers.c = 4;
            cpu.execute(Instruction::OR(ArithmeticTarget::C));
            assert_eq!(cpu.registers.a, 7);
        }

        //XOR

        #[test]
        fn xor() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 7;
            cpu.registers.c = 4;
            cpu.execute(Instruction::XOR(ArithmeticTarget::C));
            assert_eq!(cpu.registers.a, 3);
        }

        //CP
        #[test]
        fn cp() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 7;
            cpu.registers.c = 8;
            cpu.execute(Instruction::CP(ArithmeticTarget::C));
            assert_eq!(cpu.registers.f.carry, true);
            assert_eq!(cpu.registers.f.subtract, true);
            assert_eq!(cpu.registers.f.half_carry, true);
            assert_eq!(cpu.registers.f.zero, false);
        }

        //INC
        #[test]
        fn increment_8bit_register() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 7;
            cpu.execute(Instruction::INC(IncDecTarget::B));
            assert_eq!(cpu.registers.b, 8);
            assert_eq!(cpu.registers.f.zero, false);
        }

        #[test]
        fn increment_8bit_register_overflow() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 255;
            cpu.execute(Instruction::INC(IncDecTarget::B));
            assert_eq!(cpu.registers.b, 0);
            assert_eq!(cpu.registers.f.zero, true);
            assert_eq!(cpu.registers.f.half_carry, true);
        }

        #[test]
        fn increment_16bit_register() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.set_bc(1020);
            cpu.execute(Instruction::INC(IncDecTarget::BC));
            assert_eq!(cpu.registers.get_bc(), 1021);
        }

        #[test]
        fn increment_16bit_register_overflow() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.set_bc(0xFFFF);
            cpu.execute(Instruction::INC(IncDecTarget::BC));
            assert_eq!(cpu.registers.get_bc(), 0);
        }

        #[test]
        fn increment_16bit_register_byte_overflow() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.set_bc(0xFF);
            cpu.execute(Instruction::INC(IncDecTarget::BC));
            assert_eq!(cpu.registers.get_bc(), 0x0100);
        }

        //DEC
        #[test]
        fn decrement_8bit_register() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 7;
            cpu.execute(Instruction::DEC(IncDecTarget::B));
            assert_eq!(cpu.registers.b, 6);
            assert_eq!(cpu.registers.f.zero, false);
        }

        #[test]
        fn decrement_8bit_register_underflow() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 0;
            cpu.execute(Instruction::DEC(IncDecTarget::B));
            assert_eq!(cpu.registers.b, 255);
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.f.half_carry, false);
        }

        #[test]
        fn decrement_16bit_register() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.set_bc(1020);
            cpu.execute(Instruction::DEC(IncDecTarget::BC));
            assert_eq!(cpu.registers.get_bc(), 1019);
        }

        #[test]
        fn decrement_16bit_register_underflow() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.set_bc(0x0);
            cpu.execute(Instruction::DEC(IncDecTarget::BC));
            assert_eq!(cpu.registers.get_bc(), 0xFFFF);
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.f.carry, false);
        }

        #[test]
        fn decrement_16bit_register_byte_underflow() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.set_bc(0x100);
            cpu.execute(Instruction::DEC(IncDecTarget::BC));
            assert_eq!(cpu.registers.get_bc(), 0xFF);
        }

        #[test]
        fn ccf() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::CCF);
            assert_eq!(cpu.registers.f.carry, false);
        }

        #[test]
        fn scf() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::SCF);
            assert_eq!(cpu.registers.f.carry, true);
            assert_eq!(cpu.registers.f.subtract, false);
            assert_eq!(cpu.registers.f.half_carry, false);
        }

        #[test]
        fn rra() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 0b00000101;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::RRA);
            assert_eq!(cpu.registers.a, 0b10000010);
            assert_eq!(cpu.registers.f.carry, true);
        }

        #[test]
        fn rla() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 0b00000101;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::RLA);
            assert_eq!(cpu.registers.a, 0b00001011);
            assert_eq!(cpu.registers.f.carry, false);
        }

        #[test]
        fn rrca() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 0b00000101;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::RRCA);
            assert_eq!(cpu.registers.a, 0b10000010);
            assert_eq!(cpu.registers.f.carry, true);
        }

        #[test]
        fn rlca() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 0b00000101;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::RLCA);
            assert_eq!(cpu.registers.a, 0b00001010);
            assert_eq!(cpu.registers.f.carry, false);
        }

        #[test]
        fn cpl() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.a = 0b01100101;
            cpu.execute(Instruction::CPL);
            assert_eq!(cpu.registers.a, 0b10011010);
            assert_eq!(cpu.registers.f.subtract, true);
            assert_eq!(cpu.registers.f.half_carry, true);
        }

        #[test]
        fn bit() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 0b10011000;
            cpu.registers.f.zero = true; //result of bit test will be stored here
            cpu.execute(Instruction::BIT(PrefixTarget::B, BitPosition::B4));
            assert_eq!(cpu.registers.f.zero, false);
            cpu.execute(Instruction::BIT(PrefixTarget::B, BitPosition::B2));
            assert_eq!(cpu.registers.f.zero, true);
        }

        #[test]
        fn reset() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 0b10011000;
            cpu.execute(Instruction::RES(PrefixTarget::B, BitPosition::B4));
            cpu.execute(Instruction::BIT(PrefixTarget::B, BitPosition::B4));
            assert_eq!(cpu.registers.f.zero, true);
        }

        use super::*;
        #[test]
        fn set() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 0b10010000;
            cpu.execute(Instruction::BIT(PrefixTarget::B, BitPosition::B3));
            assert_eq!(cpu.registers.f.zero, true);
            cpu.execute(Instruction::SET(PrefixTarget::B, BitPosition::B3));
            cpu.execute(Instruction::BIT(PrefixTarget::B, BitPosition::B3));
            assert_eq!(cpu.registers.f.zero, false);
        }

        #[test]
        fn srl() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 0b10010000;
            cpu.execute(Instruction::SRL(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.f.carry, false);
            assert_eq!(cpu.registers.b, 0b01001000);
            cpu.registers.b = 0b00000001;
            cpu.execute(Instruction::SRL(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, true);
            assert_eq!(cpu.registers.b, 0);
            assert_eq!(cpu.registers.f.carry, true);
        }

        #[test]
        fn rr() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 0b10010000;
            cpu.execute(Instruction::RR(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.f.carry, false);
            assert_eq!(cpu.registers.b, 0b01001000);
            cpu.registers.b = 0b00000001;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::RR(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.b, 0b10000000);
            assert_eq!(cpu.registers.f.carry, true);
        }

        #[test]
        fn rl() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 0b10010000;
            cpu.execute(Instruction::RL(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.f.carry, true);
            assert_eq!(cpu.registers.b, 0b00100000);
            cpu.registers.b = 0b10000001;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::RL(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.b, 0b00000011);
            assert_eq!(cpu.registers.f.carry, true);
        }

        #[test]
        fn rrc() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 0b10010000;
            cpu.execute(Instruction::RRC(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.f.carry, false);
            assert_eq!(cpu.registers.b, 0b01001000);
            cpu.registers.b = 0b10000001;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::RRC(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.b, 0b11000000);
            assert_eq!(cpu.registers.f.carry, true);
        }

        #[test]
        fn rlc() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 0b10010000;
            cpu.execute(Instruction::RLC(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.f.carry, true);
            assert_eq!(cpu.registers.b, 0b00100001);
            cpu.registers.b = 0b10000001;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::RLC(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.b, 0b00000011);
            assert_eq!(cpu.registers.f.carry, true);
        }

        #[test]
        fn sra() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 0b10010000;
            cpu.execute(Instruction::SRA(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.f.carry, false);
            assert_eq!(cpu.registers.b, 0b11001000);
            cpu.registers.b = 0b10000001;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::SRA(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.b, 0b11000000);
            assert_eq!(cpu.registers.f.carry, true);
        }

        fn sla() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 0b10010000;
            cpu.execute(Instruction::SLA(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.f.carry, true);
            assert_eq!(cpu.registers.b, 0b00100000);
            cpu.registers.b = 0b00000001;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::SLA(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.b, 0b00000010);
            assert_eq!(cpu.registers.f.carry, false);
        }

        #[test]
        fn swap() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.registers.b = 0b10010110;
            cpu.execute(Instruction::SWAP(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.f.carry, false);
            assert_eq!(cpu.registers.b, 0b01101001);
            cpu.registers.b = 0b00000001;
            cpu.registers.f.carry = true;
            cpu.execute(Instruction::SLA(PrefixTarget::B));
            assert_eq!(cpu.registers.f.zero, false);
            assert_eq!(cpu.registers.b, 0b00000010);
            assert_eq!(cpu.registers.f.carry, false);
        }
    }

    mod program_counter {
        use super::*;
        #[test]
        fn pc_increase_with_step() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.bus.write_byte(0, 0x00);
            cpu.bus.write_byte(1, 0x3C);
            cpu.bus.write_byte(2, 0x13);
            cpu.step();
            assert_eq!(cpu.pc, 1);
            cpu.step();
            assert_eq!(cpu.pc, 2);
            assert_eq!(cpu.registers.a, 1);
            cpu.step();
            assert_eq!(cpu.pc, 3);
            assert_eq!(cpu.registers.get_de(), 1);
        }
    }

    mod prefix_instruction {
        use super::*;
        #[test]
        fn run_prefixed_command() {
            let mut cpu = CPU::new(None, vec![0; 0x10000]);
            cpu.bus.write_byte(0, 0xCB);
            cpu.bus.write_byte(1, 0x37);
            cpu.registers.a = 0xEF;
            cpu.step();
            assert_eq!(cpu.pc, 2);
            assert_eq!(cpu.registers.a, 0xFE);
        }
    }
}
