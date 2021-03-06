use memory::Memory;
use util::LoHi;

bitflags! {
  struct Flags: u8 {
    const ZERO       = 0b10000000;
    const SUBTRACT   = 0b01000000;
    const HALF_CARRY = 0b00100000;
    const CARRY      = 0b00010000;
  }
}

// FFFF & FF0F
bitflags! {
  struct InterruptFlags: u8 {
    const JOYPAD   = 0b00010000;
    const SERIAL   = 0b00001000;
    const TIMER    = 0b00000100;
    const LCD_STAT = 0b00000010;
    const VBLANK   = 0b00000001;
  }
}

enum Interrupt {
  VBlank  = 0x0040,
  LCDStat = 0x0048,
  Timer   = 0x0050,
  Serial  = 0x0058,
  Joypad  = 0x0060
}

pub struct CPU {
  a: u8,
  f: Flags,
  b: u8,
  c: u8,
  d: u8,
  e: u8,
  h: u8,
  l: u8,
  stack_pointer: u16,
  program_counter: u16,
  transition_enable_interrupts: bool,
  interrupts: bool // IME
}

impl CPU {
  pub fn new() -> CPU {
    CPU {
      a: 0x01,
      f: Flags::from_bits_truncate(0xb0),
      b: 0x00,
      c: 0x13,
      d: 0x00,
      e: 0xd8,
      h: 0x01,
      l: 0x4d,
      stack_pointer: 0xfffe,
      program_counter: 0x100,
      transition_enable_interrupts: false,
      interrupts: true
    }
  }

  pub fn step(&mut self, memory: &mut Memory) -> i64 {    
    // Interrupts
    {
      let mut active_interrupt: Option<Interrupt> = None;

      let mut ifs = InterruptFlags::from_bits_truncate(memory.read_byte(0xff0f));
      let ies = InterruptFlags::from_bits_truncate(memory.read_byte(0xffff));

      if ifs.contains(VBLANK) && ies.contains(VBLANK) {
        active_interrupt = Some(Interrupt::VBlank);
        ifs.remove(VBLANK);
        memory.write_byte(0xff0f, ifs.bits);
      } else if ifs.contains(LCD_STAT) && ies.contains(LCD_STAT) {
        active_interrupt = Some(Interrupt::LCDStat);
        ifs.remove(LCD_STAT);
        memory.write_byte(0xff0f, ifs.bits);
      } else if ifs.contains(TIMER) && ies.contains(TIMER) {
        active_interrupt = Some(Interrupt::Timer);
        ifs.remove(TIMER);
        memory.write_byte(0xff0f, ifs.bits);
      } else if ifs.contains(SERIAL) && ies.contains(SERIAL) {
        active_interrupt = Some(Interrupt::Serial);
        ifs.remove(SERIAL);
        memory.write_byte(0xff0f, ifs.bits);
      } else if ifs.contains(JOYPAD) && ies.contains(JOYPAD) {
        active_interrupt = Some(Interrupt::Joypad);
        ifs.remove(JOYPAD);
        memory.write_byte(0xff0f, ifs.bits);
      }

      if self.interrupts {
        if let Some(interrupt) = active_interrupt {
          let pc = self.program_counter;
          self.push_short(memory, pc);
          self.program_counter = interrupt as u16;
          self.interrupts = false;
          return 80;
        }
      }

      if self.transition_enable_interrupts {
        self.transition_enable_interrupts = false;
        self.interrupts = true;
      }
    }
    // Fetch
    let opcode: u8 = memory.read_byte(self.program_counter);
    println!("{:02x} at address {:04x}", opcode, self.program_counter);
    assert!(self.program_counter <= 0x7FFF);
    // Increment
    self.program_counter = self.program_counter.wrapping_add(1);
    // Execute
    match opcode {
      0x00 => {
        // NOP
        4
      },
      0x01 => {
        // LD BC, d16
        self.c = self.read_byte_immediate(memory);
        self.b = self.read_byte_immediate(memory);
        12
      },
      0x02 => {
        // LD (BC),A
        let value = memory.read_byte((self.b as u16) << 8 | self.c as u16);
        self.a = value;
        8
      },
      0x03 => {
        // INC BC
        inc_double_r8(&mut self.b, &mut self.c)
      },
      0x04 => {
        // INC B
        inc_r8(&mut self.b, &mut self.f)
      },
      0x05 => {
        // DEC B
        dec_r8(&mut self.b, &mut self.f)
      },
      0x06 => {
        // LD n into B
        let value = self.read_byte_immediate(memory);
        self.b = value;
        8
      },
      0x07 => {
        // RLCA
        let old_carry = (self.f.bits & CARRY.bits) >> 4; // 0 or 1
        self.f.remove(ZERO);
        self.f.remove(SUBTRACT);
        self.f.remove(HALF_CARRY);
        self.f.set(CARRY, (self.a & 0x80) == 0x80);
        self.a <<= 1;
        self.a |= old_carry;
        4
      },
      0x09 => {
        // ADD HL,BC
        let orig = self.hl();
        let val = self.hl().wrapping_add(self.bc());
        self.h = val.hi();
        self.l = val.lo();
        let res = self.hl();
        self.f.remove(SUBTRACT);
        self.f.set(HALF_CARRY, (orig ^ val ^ res) & 0x100 == 0x100);
        self.f.set(CARRY, res < orig);
        8
      },
      0x0b => {
        // DEC BC
        dec_double_r8(&mut self.b, &mut self.c)
      },
      0x0c => {
        // INC C
        inc_r8(&mut self.c, &mut self.f)
      },
      0x0d => {
        // DEC C
        dec_r8(&mut self.c, &mut self.f)
      },
      0x0e => {
        // LD n into C
        let value = self.read_byte_immediate(memory);
        self.c = value;
        8
      },
      0x11 => {
        // LD DE,d16
        self.e = self.read_byte_immediate(memory);
        self.d = self.read_byte_immediate(memory);
        12
      },
      0x13 => {
        // INC DE
        inc_double_r8(&mut self.d, &mut self.e)
      },
      0x14 => {
        // INC D
        inc_r8(&mut self.d, &mut self.f)
      },
      0x15 => {
        // DEC D
        dec_r8(&mut self.d, &mut self.f)
      },
      0x18 => {
        // JR
        let rel_target = self.read_signed_byte_immediate(memory);
        self.relative_jump(rel_target);
        12
      },
      0x1a => {
        // LD A,(DE)
        self.a = memory.read_byte(self.de());
        8
      },
      0x1b => {
        // DEC DE
        dec_double_r8(&mut self.d, &mut self.e)
      },
      0x1c => {
        // INC E
        inc_r8(&mut self.e, &mut self.f)
      },
      0x1d => {
        // DEC E
        dec_r8(&mut self.e, &mut self.f)
      },
      0x20 => {
        // JR NZ
        let rel_target = self.read_signed_byte_immediate(memory);
        if !self.f.contains(ZERO) {
          self.relative_jump(rel_target);
          12
        } else {
          8
        }
      },
      0x21 => {
        // LD nn into HL
        let value = self.read_short_immediate(memory);
        self.h = value.hi();
        self.l = value.lo();
        12
      },
      0x22 => {
        // LD (HL+),A
        memory.write_byte(self.hl(), self.a);
        let val = self.hl().wrapping_add(1);
        self.h = val.hi();
        self.l = val.lo();
        8
      },
      0x23 => {
        // INC HL
        inc_double_r8(&mut self.h, &mut self.l)
      },
      0x24 => {
        // INC H
        inc_r8(&mut self.h, &mut self.f)
      },
      0x25 => {
        // DEC H
        dec_r8(&mut self.h, &mut self.f)
      },
      0x28 => {
        // JR Z,r8
        let rel_target = self.read_signed_byte_immediate(memory);
        if self.f.contains(ZERO) {
          self.relative_jump(rel_target);
          12
        } else {
          8
        }
      },
      0x2a => {
        // LD A,(HL+)
        self.a = memory.read_byte(self.hl());
        let val = self.hl().wrapping_add(1);
        self.h = val.hi();
        self.l = val.lo();
        8
      },
      0x2b => {
        // DEC HL
        dec_double_r8(&mut self.h, &mut self.l)
      },
      0x2c => {
        // INC L
        inc_r8(&mut self.l, &mut self.f)
      },
      0x2d => {
        // DEC L
        dec_r8(&mut self.l, &mut self.f)
      },
      0x2f => {
        // CPL A
        self.a = !self.a;
        self.f.insert(SUBTRACT);
        self.f.insert(HALF_CARRY);
        4
      },
      0x31 => {
        // LD SP,d16
        let value = self.read_short_immediate(memory);
        self.stack_pointer = value;
        12
      },
      0x32 => {
        // LD (HL-),A
        let result = (self.a as u16).wrapping_sub(1);
        self.h = result.hi();
        self.l = result.lo();
        8
      },
      0x33 => {
        // INC SP
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        8
      },
      0x34 => {
        // INC (HL)
        let orig = self.read_byte_immediate(memory);
        let value = orig.wrapping_add(1);
        let destination = self.hl();
        memory.write_byte(destination, value);
        self.f.set(ZERO, value == 0);
        self.f.remove(SUBTRACT);
        self.f.set(HALF_CARRY, (orig ^ 1 ^ value & 0x10) == 0x10);
        12
      },
      0x35 => {
        // DEC (HL)
        let orig = self.read_byte_immediate(memory);
        let value = orig.wrapping_sub(1);
        let destination = self.hl();
        memory.write_byte(destination, value);
        self.f.set(ZERO, value == 0);
        self.f.insert(SUBTRACT);
        self.f.set(HALF_CARRY, (orig ^ !1 ^ value & 0x10) == 0x10);
        12
      },
      0x36 => {
        // LD (HL),d8
        let value = self.read_byte_immediate(memory);
        let destination = self.hl();
        memory.write_byte(destination, value);
        12
      },
      0x38 => {
        // JR C,r8
        let rel_target = self.read_signed_byte_immediate(memory);
        if self.f.contains(CARRY) {
          self.relative_jump(rel_target);
          12
        } else {
          8
        }
      },
      0x3a => {
        // LD A,(HL-)
        self.a = memory.read_byte(self.hl());
        let val = self.hl().wrapping_sub(1);
        self.h = val.hi();
        self.l = val.lo();
        8
      },
      0x3b => {
        // DEC SP
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
        8
      },
      0x3c => {
        // INC A
        inc_r8(&mut self.a, &mut self.f)
      },
      0x3d => {
        // DEC A
        dec_r8(&mut self.a, &mut self.f)
      },
      0x3e => {
        // LD # into A
        let result = self.read_byte_immediate(memory);
        self.a = result;
        8
      },
      0x40 => {
        // LD B,B
        // self.b = self.b;
        4
      },
      0x41 => {
        // LD B,C
        self.b = self.c;
        4
      },
      0x42 => {
        // LD B,D
        self.b = self.d;
        4
      },
      0x43 => {
        // LD B,E
        self.b = self.e;
        4
      },
      0x44 => {
        // LD B,H
        self.b = self.h;
        4
      },
      0x45 => {
        // LD B,L
        self.b = self.l;
        4
      },
      0x47 => {
        // LD B,A
        self.b = self.a;
        4
      },
      0x48 => {
        // LD C,B
        self.c = self.b;
        4
      },
      0x49 => {
        // LD C,C
        // self.c = self.c;
        4
      },
      0x4a => {
        // LD C,D
        self.c = self.d;
        4
      },
      0x4b => {
        // LD C,E
        self.c = self.e;
        4
      },
      0x4c => {
        // LD C,H
        self.c = self.h;
        4
      },
      0x4d => {
        // LD C,L
        self.c = self.l;
        4
      },
      0x4f => {
        // LD C,A
        self.c = self.a;
        4
      },
      0x50 => {
        // LD D,B
        self.d = self.b;
        4
      },
      0x51 => {
        // LD D,C
        self.d = self.c;
        4
      },
      0x52 => {
        // LD D,D
        // self.d = self.d;
        4
      },
      0x53 => {
        // LD D,E
        self.d = self.e;
        4
      },
      0x54 => {
        // LD D,H
        self.d = self.h;
        4
      },
      0x55 => {
        // LD D,L
        self.d = self.l;
        4
      },
      0x56 => {
        // LD D,(HL)
        self.d = memory.read_byte(self.hl());
        8
      },
      0x57 => {
        // LD D,A
        self.d = self.a;
        4
      },
      0x58 => {
        // LD E,B
        self.e = self.b;
        4
      },
      0x59 => {
        // LD E,C
        self.e = self.c;
        4
      },
      0x5a => {
        // LD E,D
        self.e = self.d;
        4
      },
      0x5b => {
        // LD E,E
        // self.e = self.e;
        4
      },
      0x5c => {
        // LD E,H
        self.e = self.h;
        4
      },
      0x5d => {
        // LD E,L
        self.e = self.l;
        4
      },
      0x5e => {
        // LD E,(HL)
        self.e = memory.read_byte(self.hl());
        8
      },
      0x5f => {
        // LD E,A
        self.e = self.a;
        4
      },
      0x60 => {
        // LD H,B
        self.h = self.b;
        4
      },
      0x61 => {
        // LD H,C
        self.h = self.c;
        4
      },
      0x62 => {
        // LD H,D
        self.h = self.d;
        4
      },
      0x63 => {
        // LD H,E
        self.h = self.e;
        4
      },
      0x64 => {
        // LD H,H
        // self.h = self.h;
        4
      },
      0x65 => {
        // LD H,L
        self.h = self.l;
        4
      },
      0x67 => {
        // LD H,A
        self.h = self.a;
        4
      },
      0x68 => {
        // LD L,B
        self.l = self.b;
        4
      },
      0x69 => {
        // LD L,C
        self.l = self.c;
        4
      },
      0x6a => {
        // LD L,D
        self.l = self.d;
        4
      },
      0x6b => {
        // LD L,E
        self.l = self.e;
        4
      },
      0x6c => {
        // LD L,H
        self.l = self.h;
        4
      },
      0x6d => {
        // LD L,L
        // self.l = self.l;
        4
      },
      0x6f => {
        // LD L,A
        self.l = self.a;
        4
      },
      0x77 => {
        // LD (HL),A
        memory.write_byte(self.hl(), self.a);
        8
      },
      0x78 => {
        // LD A,B
        self.a = self.b;
        4
      },
      0x79 => {
        // LD A,C
        self.a = self.c;
        4
      0x7a => {
        // LD A,D
        self.a = self.d;
        4
      },
      0x7b => {
        // LD A,E
        self.a = self.e;
        4
      },
      0x7c => {
        // LD A,H
        self.a = self.h;
        4
      },
      0x7d => {
        // LD A,L
        self.a = self.l;
        4
      },
      0x7e => {
        // LD A,(HL)
        self.a = memory.read_byte(self.hl());
        8
      },
      0x7f => {
        // LD A,A
        // self.a = self.a;
        4
      },
      0x83 => {
        // ADD A,E
        let original = self.a;
        self.a = self.a.wrapping_add(self.e);
        self.f.set(ZERO, self.a == 0);
        self.f.remove(SUBTRACT);
        self.f.set(HALF_CARRY, (original ^ self.e ^ self.a) & 0x10 == 0x10);
        self.f.set(CARRY, self.a < original);
        4
      },
      0x8e => {
        // ADC (HL)
        let original = self.a;
        let value = memory.read_byte(self.hl()).wrapping_add((self.f.bits & CARRY.bits) >> 4);
        self.a = self.a.wrapping_add(value);
        self.f.set(ZERO, self.a == 0);
        self.f.remove(SUBTRACT);
        self.f.set(HALF_CARRY, (original ^ value ^ self.a) & 0x10 == 0x10);
        self.f.set(CARRY, self.a < original);
        8
      },
      0x99 => {
        // SBC A,C
        let original = self.a;
        let value = self.c.wrapping_add((self.f.bits & CARRY.bits) >> 4);
        self.a = self.a.wrapping_add(value);
        self.f.set(ZERO, self.a == 0);
        self.f.insert(SUBTRACT);
        self.f.set(HALF_CARRY, (original ^ value ^ self.a) & 0x10 == 0x10);
        self.f.set(CARRY, self.a < original);
        4
      },
      0xa2 => {
        // AND C
        self.a &= self.c;
        self.f.set(ZERO, self.a == 0);
        self.f.remove(SUBTRACT);
        self.f.insert(HALF_CARRY);
        self.f.remove(CARRY);
        4
      },
      0xa3 => {
        // AND E
        self.a &= self.e;
        self.f.set(ZERO, self.a == 0);
        self.f.remove(SUBTRACT);
        self.f.insert(HALF_CARRY);
        self.f.remove(CARRY);
        4
      },
      0xa7 => {
        // AND A
        // self.a &= self.a;
        self.f.set(ZERO, self.a == 0);
        self.f.remove(SUBTRACT);
        self.f.insert(HALF_CARRY);
        self.f.remove(CARRY);
        4
      },
      0xa9 => {
        // XOR C
        self.a ^= self.c;
        self.f.set(ZERO, self.a == 0);
        self.f.remove(SUBTRACT);
        self.f.remove(HALF_CARRY);
        self.f.remove(CARRY);
        4
      },
      0xaf => {
        // XOR A with A
        //self.a ^= self.a;
        self.a = 0;
        self.f.insert(ZERO);
        self.f.remove(SUBTRACT);
        self.f.remove(HALF_CARRY);
        self.f.remove(CARRY);
        4
      },
      0xb1 => {
        // OR B
        self.a |= self.b;
        self.f.set(ZERO, self.a == 0);
        self.f.remove(SUBTRACT);
        self.f.remove(HALF_CARRY);
        self.f.remove(CARRY);
        4
      },
      0xbd => {
        // CP L
        self.f.set(ZERO, self.a == self.l);
        self.f.insert(SUBTRACT);
        self.f.set(HALF_CARRY, (self.a ^ self.l ^ self.a.wrapping_sub(self.l)) & 0x10 == 0x10);
        self.f.set(CARRY, self.a > self.l);
        4
      },
      0xc0 => {
        // RET NZ
        let dest = self.pop_short(memory);
        if !self.f.contains(ZERO) {
          self.program_counter = dest;
          20
        } else {
          self.push_short(memory, dest);
          8
        }
      },
      0xc1 => {
        // POP BC
        self.c = self.pop_byte(memory);
        self.b = self.pop_byte(memory);
        12
      },
      0xc3 => {
        // JMP nn
        let target = memory.read_short(self.program_counter);
        self.program_counter = target;
        16
      },
      0xc4 => {
        // CALL NZ,a16
        let target = self.read_short_immediate(memory);
        if !self.f.contains(ZERO) {
          let pc = self.program_counter;
          self.push_short(memory, pc);
          self.program_counter = target;
          24
        } else {
          12
        }
      },
      0xc5 => {
        // PUSH BC
        let b = self.b;
        self.push_byte(memory, b);
        let c = self.c;
        self.push_byte(memory, c);
        16
      },
      0xcd => {
        // CALL a16
        let target = self.read_short_immediate(memory);
        let pc = self.program_counter;
        self.push_short(memory, pc);
        self.program_counter = target;
        24
      },
      0xc9 => {
        // RET
        let dest = self.pop_short(memory);
        self.program_counter = dest;
        16
      },
      0xcb => {
        // CB
        let next_opcode = self.read_byte_immediate(memory);
        // TODO: is this 4 plus right? starting to think not
        4 + self.cb(next_opcode)
      },
      0xd5 => {
        // PUSH DE
        let d = self.d;
        self.push_byte(memory, d);
        let e = self.e;
        self.push_byte(memory, e);
        16
      },
      0xe0 => {
        // LDH n,A
        let offset = self.read_byte_immediate(memory);
        memory.write_byte(0xFF00 + offset as u16, self.a);
        12
      },
      0xe1 => {
        // POP HL
        self.l = self.pop_byte(memory);
        self.h = self.pop_byte(memory);
        12
      },
      0xe2 => {
        // LD (C),A
        memory.write_byte(0xFF00 + self.c as u16, self.a);
        8
      },
      0xe5 => {
        // PUSH HL
        let h = self.h;
        self.push_byte(memory, h);
        let l = self.l;
        self.push_byte(memory, l);
        16
      },
      0xe6 => {
        // AND d8
        let val = self.read_byte_immediate(memory);
        self.a &= val;
        self.f.set(ZERO, self.a == 0);
        self.f.remove(SUBTRACT);
        self.f.insert(HALF_CARRY);
        self.f.remove(CARRY);
        8
      },
      0xe7 => {
        // RST 20H
        let pc = self.program_counter;
        self.push_short(memory, pc);
        self.program_counter = 0x0020;
        16
      },
      0xe9 => {
        // JP HL
        self.program_counter = self.hl();
        4
      },
      0xea => {
        // LD a16,A
        let dest = self.read_short_immediate(memory);
        memory.write_byte(dest, self.a);
        16
      },
      0xee => {
        // XOR d8
        let val = self.read_byte_immediate(memory);
        self.a ^= val;
        self.f.set(ZERO, self.a == 0);
        self.f.remove(SUBTRACT);
        self.f.remove(HALF_CARRY);
        self.f.remove(CARRY);
        8
      },
      0xf0 => {
        // LDH A,n
        let offset = self.read_byte_immediate(memory);
        self.a = memory.read_byte(0xFF00 + offset as u16);
        12
      },
      0xf1 => {
        // POP AF
        self.f = Flags::from_bits_truncate(self.pop_byte(memory));
        self.a = self.pop_byte(memory);
        12
      },
      0xf3 => {
        // DI
        self.interrupts = false;
        4
      },
      0xf5 => {
        // PSH AF
        let af = self.af();
        self.push_short(memory, af);
        16
      },
      0xfa => {
        // LD A,(a16)
        let addr = self.read_short_immediate(memory);
        self.a = memory.read_byte(addr);
        16
      },
      0xfb => {
        // EI
        self.transition_enable_interrupts = true;
        4
      },
      0xfe => {
        // CP n
        let value = self.read_byte_immediate(memory);
        self.f.set(ZERO, self.a == value);
        self.f.insert(SUBTRACT);
        self.f.set(HALF_CARRY, (self.a ^ value ^ self.a.wrapping_sub(value)) & 0x10 == 0x10);
        self.f.set(CARRY, self.a > value);
        8
      },
      0xff => {
        // RST 38
        let pc = self.program_counter;
        self.push_short(memory, pc);
        self.program_counter = 0x0038;
        16
      },
      _ => {
        unimplemented!()
      }
    }
  }

  fn cb(&mut self, opcode: u8) -> i64 {
    println!("cb {:02x}", opcode);
    match opcode {
      0x37 => {
        // SWAP A
        self.f.set(ZERO, self.a == 0);
        self.f.remove(SUBTRACT);
        self.f.remove(HALF_CARRY);
        self.f.remove(CARRY);
        8
      },
      _ => {
        unimplemented!()
      }
    }
  }

  fn relative_jump(&mut self, rel_target: i8) {
    if rel_target < 0 {
      self.program_counter -= -rel_target as u16;
    } else {
      self.program_counter += rel_target as u16;
    }
  }

  fn push_short(&mut self, memory: &mut Memory, value: u16) {
    println!("pushing {:x} onto stack", value);
    self.push_byte(memory, value.hi());
    self.push_byte(memory, value.lo());
  }

  fn push_byte(&mut self, memory: &mut Memory, value: u8) {
    self.stack_pointer = self.stack_pointer.wrapping_sub(1);
    memory.write_byte(self.stack_pointer, value);
  }

  fn pop_short(&mut self, memory: &Memory) -> u16 {
    let lo = self.pop_byte(memory) as u16;
    let t = (self.pop_byte(memory) as u16) << 8 | lo;
    println!("popping {:x} off stack", t);
    t
  }

  fn pop_byte(&mut self, memory: &Memory) -> u8 {
    let x = memory.read_byte(self.stack_pointer);
    self.stack_pointer = self.stack_pointer.wrapping_add(1);
    x
  }

  fn read_short_immediate(&mut self, memory: &Memory) -> u16 {
    let value = memory.read_short(self.program_counter);
    self.program_counter += 2;
    value
  }

  fn read_byte_immediate(&mut self, memory: &Memory) -> u8 {
    let value = memory.read_byte(self.program_counter);
    self.program_counter += 1;
    value
  }

  fn read_signed_byte_immediate(&mut self, memory: &Memory) -> i8 {
    let value = memory.read_signed_byte(self.program_counter);
    self.program_counter += 1;
    value
  }

  fn hl(&self) -> u16 {
    (self.h as u16) << 8 | self.l as u16
  }

  fn af(&self) -> u16 {
    (self.a as u16) << 8 | self.f.bits as u16
  }

  fn bc(&self) -> u16 {
    (self.b as u16) << 8 | self.c as u16
  }

  fn de(&self) -> u16 {
    (self.d as u16) << 8 | self.e as u16
  }
}

fn inc_r8(register: &mut u8, flags: &mut Flags) -> i64 {
  // INC 8-bit register
  let orig = *register;
  *register = (*register).wrapping_add(1);
  (*flags).set(ZERO, *register == 0);
  (*flags).remove(SUBTRACT);
  (*flags).set(HALF_CARRY, (orig ^ 1 ^ *register & 0x10) == 0x10);
  4
}

fn dec_r8(register: &mut u8, flags: &mut Flags) -> i64 {
  // DEC 8-bit register
  let orig = *register;
  *register = (*register).wrapping_sub(1);
  (*flags).set(ZERO, (*register) == 0);
  (*flags).insert(SUBTRACT);
  (*flags).set(HALF_CARRY, (orig ^ !1 ^ *register & 0x10) == 0x10);
  4
}

fn inc_double_r8(hi_reg: &mut u8, lo_reg: &mut u8) -> i64 {
  // INC 16-bit register (formed by two 8-bit registers)
  let combined = (*hi_reg as u16) << 8 | *lo_reg as u16;
  let val = combined.wrapping_add(1);
  *hi_reg = val.hi();
  *lo_reg = val.lo();
  8
}

fn dec_double_r8(hi_reg: &mut u8, lo_reg: &mut u8) -> i64 {
  // DEC 16-bit register (formed by two 8-bit registers)
  let combined = (*hi_reg as u16) << 8 | *lo_reg as u16;
  let val = combined.wrapping_sub(1);
  *hi_reg = val.hi();
  *lo_reg = val.lo();
  8
}
