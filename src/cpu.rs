use memory::Memory;

// TODO: use bitflags?
struct Flag {
  zero: bool,
  subtract: bool,
  half_carry: bool,
  carry: bool
}

pub struct CPU {
  a: u8,
  f: u8,
  b: u8,
  c: u8,
  d: u8,
  e: u8,
  h: u8,
  l: u8,
  stack_pointer: u16,
  program_counter: u16,
  flags: Flag,
  cycles: u64
}

impl CPU {
  pub fn new() -> CPU {
    CPU {
      a: 0x01,
      f: 0xb0,
      b: 0x00,
      c: 0x13,
      d: 0x00,
      e: 0xd8,
      h: 0x01,
      l: 0x4d,
      stack_pointer: 0xfffe,
      program_counter: 0x100,
      flags: Flag {
        zero: false,
        subtract: false,
        half_carry: false,
        carry: false
      },
      cycles: 0
    }
  }

  pub fn step(&mut self, memory: &mut Memory) {
    // Fetch
    let opcode: u8 = memory.read(self.program_counter);
    println!("{:x}", opcode);
    // Increment
    self.program_counter += 1;
    // Execute
    match opcode {
      0x00 => {
        // NOP
        self.cycles += 4;
      },
      0x4d => {
        // LD L into C
        self.c = self.l;
        self.cycles += 4;
      },
      0xaf => {
        // XOR A with n
        let operand = memory.read(self.program_counter);
        self.program_counter += 1;
        self.a ^= operand;
        self.flags.zero = self.a == 0;
        self.flags.subtract = false;
        self.flags.half_carry = false;
        self.flags.carry = false;
        self.cycles += 4;
      },
      0xc3 => {
        // JMP nn
        let target = self.read_two_immediate(memory);
        println!("{:x}", target);
        self.program_counter = target;
        self.cycles += 12; // @Correctness; conflicting information on this
      },
      0xf1 => {
        // Pop into AF
        let double = self.pop_two(memory);
        self.a = double.hi();
        self.f = double.lo();
        self.cycles += 12
      }
      0xff => {
        // RST 38
        self.write_pc_to_stack(memory);
        self.program_counter = 0x0038;
        self.cycles += 32;
      }
      _ => unimplemented!()
    }
  }

  fn write_pc_to_stack(&mut self, memory: &mut Memory) {
    self.decrement_sp();
    memory.write(self.stack_pointer, self.program_counter.hi());
    self.decrement_sp();
    memory.write(self.stack_pointer, self.program_counter.lo());
  }

  fn pop_two(&mut self, memory: &Memory) -> u16 {
    let mut x: u16 = memory.read(self.stack_pointer) as u16;
    self.increment_sp();
    x = (memory.read(self.stack_pointer) as u16) << 8 | x;
    self.increment_sp();
    x
  }

  fn read_two_immediate(&mut self, memory: &Memory) -> u16 {
    // @Robustness pc + 1
    let mut x: u16 = memory.read(self.program_counter) as u16;
    (memory.read(self.program_counter + 1) as u16) << 8 | x
  }

  fn decrement_sp(&mut self) {
    debug_assert!(self.stack_pointer != 0xFF80);
    self.stack_pointer -= 1;
  }

  fn increment_sp(&mut self) {
    debug_assert!(self.stack_pointer != 0xFFFE);
    self.stack_pointer += 1;
  }
}

trait LoHi {
    type Output;
    
    fn lo(&self) -> Self::Output;
    fn hi(&self) -> Self::Output;
}

impl LoHi for u16 {
    type Output = u8;
    
    fn lo(&self) -> Self::Output { *self as u8 }
    fn hi(&self) -> Self::Output { (*self >> 8) as u8 }
}