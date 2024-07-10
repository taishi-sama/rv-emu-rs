use std::fmt::Display;

const INTERRUPT_BIT: u32 = 0x80000000;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u32)]
pub enum TrapType {
    InstructionAddressMisaligned,
    InstructionAccessFault,
    IllegalInstruction,
    Breakpoint,
    LoadAddressMisaligned,
    LoadAccessFault,
    StoreAddressMisaligned,
    StoreAccessFault,
    EnvironmentCallFromUMode,
    EnvironmentCallFromSMode,
    EnvironmentCallFromMMode = 11,
    InstructionPageFault,
    LoadPageFault,
    StorePageFault = 15,
    UserSoftwareInterrupt = INTERRUPT_BIT,
    SupervisorSoftwareInterrupt,
    MachineSoftwareInterrupt = INTERRUPT_BIT + 3,
    UserTimerInterrupt,
    SupervisorTimerInterrupt,
    MachineTimerInterrupt = INTERRUPT_BIT + 7,
    UserExternalInterrupt,
    SupervisorExternalInterrupt,
    MachineExternalInterrupt = INTERRUPT_BIT + 11,
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Trap {
    pub tcause: TrapType,
    pub tval: u32,
}
impl Trap {
    pub fn get_trap_cause(&self) -> u32 {
        self.tcause as u32
    }
}
impl Display for Trap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Trap type: {}, 0x{:x}",
            match self.tcause {
                TrapType::InstructionAddressMisaligned => "InstructionAddressMisaligned",
                TrapType::InstructionAccessFault => "InstructionAccessFault",
                TrapType::IllegalInstruction => "IllegalInstruction",
                TrapType::Breakpoint => "Breakpoint",
                TrapType::LoadAddressMisaligned => "LoadAddressMisaligned",
                TrapType::LoadAccessFault => "LoadAccessFault",
                TrapType::StoreAddressMisaligned => "StoreAddressMisaligned",
                TrapType::StoreAccessFault => "StoreAccessFault",
                TrapType::EnvironmentCallFromUMode => "EnvironmentCallFromUMode",
                TrapType::EnvironmentCallFromSMode => "EnvironmentCallFromSMode",
                TrapType::EnvironmentCallFromMMode => "EnvironmentCallFromMMode",
                TrapType::InstructionPageFault => "InstructionPageFault",
                TrapType::LoadPageFault => "LoadPageFault",
                TrapType::StorePageFault => "StorePageFault",
                TrapType::UserSoftwareInterrupt => "UserSoftwareInterrupt",
                TrapType::SupervisorSoftwareInterrupt => "SupervisorSoftwareInterrupt",
                TrapType::MachineSoftwareInterrupt => "MachineSoftwareInterrupt",
                TrapType::UserTimerInterrupt => "UserTimerInterrupt",
                TrapType::SupervisorTimerInterrupt => "SupervisorTimerInterrupt",
                TrapType::MachineTimerInterrupt => "MachineTimerInterrupt",
                TrapType::UserExternalInterrupt => "UserExternalInterrupt",
                TrapType::SupervisorExternalInterrupt => "SupervisorExternalInterrupt",
                TrapType::MachineExternalInterrupt => "MachineExternalInterrupt",
            },
            self.tval
        )
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FatalError {}
