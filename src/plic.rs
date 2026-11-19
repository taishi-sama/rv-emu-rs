use crate::{mmdevice::MMdevice, traps::TrapType};

/// The maximum number of source slots, including the reserved source 0.
pub const MAX_SOURCES: u32 = 1024;
/// A single hart has one machine-mode and one supervisor-mode context, in that order.
pub const CONTEXTS: usize = 2;

const PRIORITY_END: u32 = 0x0000_1000;
const PENDING_END: u32 = 0x0000_2000;
const ENABLE_END: u32 = 0x0020_0000;
const CONTEXT_STRIDE: u32 = 0x1000;
const ENABLE_STRIDE: u32 = 0x80;
const CLAIM_COMPLETE_OFFSET: u32 = 0x4;
pub const MEMORY_REGION_SIZE: u32 = 0x0400_0000;

//Contexts are ordered by the `interrupts-extended` property in the device tree: the first entry
//is the hart's machine-mode external interrupt, the second is its supervisor-mode external one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlicContext {
    Machine,
    Supervisor,
}

impl PlicContext {
    pub const fn index(self) -> usize {
        match self {
            Self::Machine => 0,
            Self::Supervisor => 1,
        }
    }

    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Machine),
            1 => Some(Self::Supervisor),
            _ => None,
        }
    }
}

/// A runtime handle to one PLIC source. Handles are created by [`Plic::register_source`] or
/// [`Plic::allocate_source`], so source numbers do not have to be compile-time constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrqLine(u32);

impl IrqLine {
    pub fn source(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlicError {
    ReservedSource,
    SourceOutOfRange,
    SourceAlreadyAssigned,
}

//Devices implement this trait and push their current line levels into the one PLIC. Keeping the
//PLIC concrete is deliberate: there is a single controller, and devices must never claim or
//complete an interrupt themselves.
pub trait InterruptSource {
    fn poll_irqs(&mut self, plic: &mut Plic);
}

pub struct Plic {
    sources: u32,
    contexts: usize,
    words: usize,
    //One priority per source; entry 0 is the reserved source and always reads zero.
    priorities: Box<[u32]>,
    //One pending bit per source. These bits are global; the in-service bits are per context.
    pending: Box<[u32]>,
    //contexts * words: one enable bitmap per context.
    enables: Box<[u32]>,
    //One threshold per context.
    thresholds: Box<[u32]>,
    //contexts * words: sources a context has claimed but not completed.
    in_service: Box<[u32]>,
    //Sources handed to a device. This is software bookkeeping for runtime registration; the
    //hardware register map still contains all `sources` entries.
    assigned: Box<[u32]>,
}

impl Plic {
    pub fn new(sources: u32, contexts: usize) -> Self {
        assert!(sources > 1, "the PLIC needs at least one real source");
        assert!(
            sources <= MAX_SOURCES,
            "the PLIC supports at most {MAX_SOURCES} sources"
        );
        assert!(
            contexts > 0 && contexts <= CONTEXTS,
            "only machine and supervisor contexts are supported"
        );
        let words = (sources as usize).div_ceil(32);
        Self {
            sources,
            contexts,
            words,
            priorities: vec![0; sources as usize].into_boxed_slice(),
            pending: vec![0; words].into_boxed_slice(),
            enables: vec![0; contexts * words].into_boxed_slice(),
            thresholds: vec![0; contexts].into_boxed_slice(),
            in_service: vec![0; contexts * words].into_boxed_slice(),
            assigned: vec![0; words].into_boxed_slice(),
        }
    }

    /// Assigns an explicit source number, normally one taken from the device tree.
    pub fn register_source(&mut self, source: u32) -> Result<IrqLine, PlicError> {
        if source == 0 {
            return Err(PlicError::ReservedSource);
        }
        if source >= self.sources {
            return Err(PlicError::SourceOutOfRange);
        }
        if self.is_assigned(source) {
            return Err(PlicError::SourceAlreadyAssigned);
        }
        self.reset_source(source);
        self.set_assigned(source, true);
        Ok(IrqLine(source))
    }

    /// Assigns the lowest free source number, for devices that do not need a fixed number.
    pub fn allocate_source(&mut self) -> Option<IrqLine> {
        for source in 1..self.sources {
            if !self.is_assigned(source) {
                self.reset_source(source);
                self.set_assigned(source, true);
                return Some(IrqLine(source));
            }
        }
        None
    }

    /// Releases a source and clears all of its pending, enable and in-service state.
    pub fn unregister_source(&mut self, line: IrqLine) -> bool {
        if !self.is_line_assigned(line) {
            return false;
        }
        self.reset_source(line.0);
        self.set_assigned(line.0, false);
        true
    }

    /// Updates a level-triggered line. A low level clears the pending bit; a high level sets it.
    #[inline]
    pub fn set_level(&mut self, line: IrqLine, level: bool) {
        if !self.is_line_assigned(line) {
            return;
        }
        let source = line.0 as usize;
        let word = source / 32;
        let bit = 1u32 << (source % 32);
        if level {
            self.pending[word] |= bit;
        } else {
            self.pending[word] &= !bit;
        }
    }

    /// Sets a sticky pending bit for an edge-triggered source.
    pub fn raise(&mut self, line: IrqLine) {
        if !self.is_line_assigned(line) {
            return;
        }
        let source = line.0 as usize;
        self.pending[source / 32] |= 1u32 << (source % 32);
    }

    /// The CPU's external-interrupt line for one context.
    #[inline]
    pub fn pending_for(&self, context: PlicContext) -> bool {
        let Some(context_index) = self.context_index(context) else {
            return false;
        };
        let threshold = self.thresholds[context_index];
        for word in 0..self.words {
            //Only sources that are pending, enabled for this context, not already in service,
            //and implemented can possibly fire. In the common idle case this is zero, so the
            //interrupt poll costs one word load instead of a scan over every source.
            let pending = self.pending[word];
            if pending == 0 {
                continue;
            }
            let mut candidates = pending
                & self.enables[context_index * self.words + word]
                & !self.in_service[context_index * self.words + word]
                & self.valid_mask(word);
            while candidates != 0 {
                let source = word * 32 + candidates.trailing_zeros() as usize;
                if source > 0 && self.priorities[source] > threshold {
                    return true;
                }
                candidates &= candidates - 1;
            }
        }
        false
    }

    /// Returns the highest-priority eligible source for a context, or 0 if none is eligible.
    ///
    /// A successful claim clears the pending bit and marks the source in service for that
    /// context. Sources with equal priority are resolved in favour of the lower source number.
    pub fn claim(&mut self, context: PlicContext) -> u32 {
        let Some(context_index) = self.context_index(context) else {
            return 0;
        };
        let threshold = self.thresholds[context_index];
        let mut best = 0;
        let mut best_priority = threshold;
        for word in 0..self.words {
            let pending = self.pending[word];
            if pending == 0 {
                continue;
            }
            let mut candidates = pending
                & self.enables[context_index * self.words + word]
                & !self.in_service[context_index * self.words + word]
                & self.valid_mask(word);
            while candidates != 0 {
                let source = word * 32 + candidates.trailing_zeros() as usize;
                let priority = self.priorities[source];
                //Strict greater-than and ascending bit order implement the lower-ID tie-break.
                if source > 0 && priority > best_priority {
                    best = source as u32;
                    best_priority = priority;
                }
                candidates &= candidates - 1;
            }
        }
        if best != 0 {
            self.clear_pending(best as usize);
            self.set_in_service(context_index, best as usize);
        }
        best
    }

    /// Completes a previously claimed source.
    ///
    /// Pending is not re-armed from a remembered level. A level-triggered device re-asserts its
    /// line through `set_level` on the next device poll, so completion observes the current
    /// line instead of a stale sample taken before the handler acknowledged the device.
    pub fn complete(&mut self, context: PlicContext, source: u32) {
        let Some(context_index) = self.context_index(context) else {
            return;
        };
        if source == 0 || source >= self.sources || !self.is_assigned(source) {
            return;
        }
        self.clear_in_service(context_index, source as usize);
    }

    fn context_index(&self, context: PlicContext) -> Option<usize> {
        let index = context.index();
        (index < self.contexts).then_some(index)
    }

    fn valid_mask(&self, word: usize) -> u32 {
        let remaining = self.sources as usize - word * 32;
        if remaining >= 32 {
            u32::MAX
        } else {
            (1u32 << remaining) - 1
        }
    }

    fn is_assigned(&self, source: u32) -> bool {
        (self.assigned[(source / 32) as usize] & (1u32 << (source % 32))) != 0
    }

    fn is_line_assigned(&self, line: IrqLine) -> bool {
        line.0 > 0 && line.0 < self.sources && self.is_assigned(line.0)
    }

    fn set_assigned(&mut self, source: u32, assigned: bool) {
        let word = (source / 32) as usize;
        let bit = 1u32 << (source % 32);
        if assigned {
            self.assigned[word] |= bit;
        } else {
            self.assigned[word] &= !bit;
        }
    }

    fn reset_source(&mut self, source: u32) {
        let source_index = source as usize;
        self.clear_pending(source_index);
        for context in 0..self.contexts {
            self.clear_enable(context, source_index);
            self.clear_in_service(context, source_index);
        }
    }

    fn clear_pending(&mut self, source: usize) {
        self.pending[source / 32] &= !(1u32 << (source % 32));
    }

    fn set_in_service(&mut self, context: usize, source: usize) {
        self.in_service[context * self.words + source / 32] |= 1u32 << (source % 32);
    }

    fn clear_in_service(&mut self, context: usize, source: usize) {
        self.in_service[context * self.words + source / 32] &= !(1u32 << (source % 32));
    }

    fn clear_enable(&mut self, context: usize, source: usize) {
        self.enables[context * self.words + source / 32] &= !(1u32 << (source % 32));
    }

    fn read_priority(&self, offset: u32) -> u32 {
        self.priorities
            .get((offset / 4) as usize)
            .copied()
            .unwrap_or(0)
    }

    fn read_pending_register(&self, offset: u32) -> u32 {
        self.pending
            .get(((offset - PRIORITY_END) / 4) as usize)
            .copied()
            .unwrap_or(0)
    }

    fn read_enable(&self, offset: u32) -> u32 {
        let relative = offset - PENDING_END;
        let context = (relative / ENABLE_STRIDE) as usize;
        let word = ((relative % ENABLE_STRIDE) / 4) as usize;
        if context < self.contexts && word < self.words {
            self.enables[context * self.words + word]
        } else {
            0
        }
    }

    fn read_context(&mut self, offset: u32) -> u32 {
        let relative = offset - ENABLE_END;
        let context_index = (relative / CONTEXT_STRIDE) as usize;
        if context_index >= self.contexts {
            return 0;
        }
        let Some(context) = PlicContext::from_index(context_index) else {
            return 0;
        };
        match relative % CONTEXT_STRIDE {
            0 => self.thresholds[context_index],
            CLAIM_COMPLETE_OFFSET => self.claim(context),
            _ => 0,
        }
    }

    fn write_priority(&mut self, offset: u32, value: u32) {
        let source = (offset / 4) as usize;
        if source > 0 && source < self.priorities.len() {
            self.priorities[source] = value;
        }
    }

    fn write_enable(&mut self, offset: u32, value: u32) {
        let relative = offset - PENDING_END;
        let context = (relative / ENABLE_STRIDE) as usize;
        let word = ((relative % ENABLE_STRIDE) / 4) as usize;
        if context >= self.contexts || word >= self.words {
            return;
        }
        let mut mask = self.valid_mask(word);
        if word == 0 {
            //Source 0 is reserved and its enable bit is hardwired to zero.
            mask &= !1;
        }
        let index = context * self.words + word;
        self.enables[index] = (self.enables[index] & !mask) | (value & mask);
    }

    fn write_context(&mut self, offset: u32, value: u32) {
        let relative = offset - ENABLE_END;
        let context_index = (relative / CONTEXT_STRIDE) as usize;
        if context_index >= self.contexts {
            return;
        }
        let Some(context) = PlicContext::from_index(context_index) else {
            return;
        };
        match relative % CONTEXT_STRIDE {
            0 => self.thresholds[context_index] = value,
            CLAIM_COMPLETE_OFFSET => self.complete(context, value),
            _ => {}
        }
    }
}

impl MMdevice for Plic {
    const MEMORY_REGION_SIZE: u32 = MEMORY_REGION_SIZE;

    fn read_word(&mut self, offset: u32) -> Result<u32, TrapType> {
        if offset & 0b11 != 0 {
            return Err(TrapType::LoadAddressMisaligned);
        }
        Ok(match offset {
            0x0000_0000..=0x0000_0fff => self.read_priority(offset),
            0x0000_1000..=0x0000_1fff => self.read_pending_register(offset),
            0x0000_2000..=0x001f_ffff => self.read_enable(offset),
            0x0020_0000..=0x03ff_ffff => self.read_context(offset),
            _ => 0,
        })
    }

    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), TrapType> {
        if offset & 0b11 != 0 {
            return Err(TrapType::StoreAddressMisaligned);
        }
        match offset {
            0x0000_0000..=0x0000_0fff => self.write_priority(offset, value),
            //The pending register is read-only.
            0x0000_1000..=0x0000_1fff => {}
            0x0000_2000..=0x001f_ffff => self.write_enable(offset, value),
            0x0020_0000..=0x03ff_ffff => self.write_context(offset, value),
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enable(plic: &mut Plic, context: PlicContext, source: u32) {
        let offset = 0x0000_2000 + context.index() as u32 * ENABLE_STRIDE + (source / 32) * 4;
        let bit = 1u32 << (source % 32);
        let current = plic.read_word(offset).unwrap();
        plic.write_word(offset, current | bit).unwrap();
    }

    fn priority(plic: &mut Plic, source: u32, priority: u32) {
        plic.write_word(source * 4, priority).unwrap();
    }

    fn threshold(plic: &mut Plic, context: PlicContext, threshold: u32) {
        plic.write_word(
            ENABLE_END + context.index() as u32 * CONTEXT_STRIDE,
            threshold,
        )
        .unwrap();
    }

    fn claim(plic: &mut Plic, context: PlicContext) -> u32 {
        plic.read_word(ENABLE_END + context.index() as u32 * CONTEXT_STRIDE + 4)
            .unwrap()
    }

    fn complete(plic: &mut Plic, context: PlicContext, source: u32) {
        plic.write_word(
            ENABLE_END + context.index() as u32 * CONTEXT_STRIDE + 4,
            source,
        )
        .unwrap();
    }

    #[test]
    fn sources_can_be_registered_and_allocated() {
        let mut plic = Plic::new(8, 2);
        let uart = plic.register_source(4).unwrap();
        assert_eq!(uart.source(), 4);
        assert_eq!(
            plic.register_source(4),
            Err(PlicError::SourceAlreadyAssigned)
        );
        assert_eq!(plic.register_source(0), Err(PlicError::ReservedSource));
        assert_eq!(plic.register_source(8), Err(PlicError::SourceOutOfRange));
        assert_eq!(plic.allocate_source().unwrap().source(), 1);
        assert!(plic.unregister_source(uart));
        assert_eq!(plic.register_source(4).unwrap().source(), 4);
    }

    #[test]
    fn claim_honours_priority_enable_and_threshold() {
        let mut plic = Plic::new(8, 2);
        let one = plic.register_source(1).unwrap();
        let two = plic.register_source(2).unwrap();
        let three = plic.register_source(3).unwrap();
        priority(&mut plic, 1, 1);
        priority(&mut plic, 2, 3);
        priority(&mut plic, 3, 2);
        for line in [one, two, three] {
            enable(&mut plic, PlicContext::Supervisor, line.source());
            plic.raise(line);
        }
        assert!(plic.pending_for(PlicContext::Supervisor));
        assert_eq!(claim(&mut plic, PlicContext::Supervisor), 2);
        complete(&mut plic, PlicContext::Supervisor, 2);
        assert_eq!(claim(&mut plic, PlicContext::Supervisor), 3);
        complete(&mut plic, PlicContext::Supervisor, 3);
        assert_eq!(claim(&mut plic, PlicContext::Supervisor), 1);
        complete(&mut plic, PlicContext::Supervisor, 1);
        assert_eq!(claim(&mut plic, PlicContext::Supervisor), 0);

        plic.raise(one);
        plic.raise(two);
        plic.raise(three);
        threshold(&mut plic, PlicContext::Supervisor, 2);
        assert_eq!(claim(&mut plic, PlicContext::Supervisor), 2);
        complete(&mut plic, PlicContext::Supervisor, 2);
        assert_eq!(claim(&mut plic, PlicContext::Supervisor), 0);
    }

    #[test]
    fn equal_priorities_choose_the_lower_source() {
        let mut plic = Plic::new(8, 2);
        let four = plic.register_source(4).unwrap();
        let five = plic.register_source(5).unwrap();
        priority(&mut plic, 4, 1);
        priority(&mut plic, 5, 1);
        enable(&mut plic, PlicContext::Supervisor, 4);
        enable(&mut plic, PlicContext::Supervisor, 5);
        plic.raise(four);
        plic.raise(five);
        assert_eq!(claim(&mut plic, PlicContext::Supervisor), 4);
    }

    #[test]
    fn source_32_uses_the_second_enable_and_pending_word() {
        let mut plic = Plic::new(33, 2);
        let line = plic.register_source(32).unwrap();
        priority(&mut plic, 32, 1);
        enable(&mut plic, PlicContext::Supervisor, 32);
        plic.raise(line);
        assert!(plic.pending_for(PlicContext::Supervisor));
        assert_eq!(claim(&mut plic, PlicContext::Supervisor), 32);
    }

    #[test]
    fn in_service_blocks_redelivery_until_complete() {
        let mut plic = Plic::new(8, 2);
        let line = plic.register_source(6).unwrap();
        priority(&mut plic, 6, 1);
        enable(&mut plic, PlicContext::Supervisor, 6);
        plic.set_level(line, true);
        assert_eq!(claim(&mut plic, PlicContext::Supervisor), 6);
        //The line is still high, but the source is in service, so it is not deliverable.
        assert!(!plic.pending_for(PlicContext::Supervisor));
        complete(&mut plic, PlicContext::Supervisor, 6);
        //Completion does not trust a stale level. The next device poll re-asserts the still-high
        //line, which is what makes the source deliverable again.
        assert!(!plic.pending_for(PlicContext::Supervisor));
        plic.set_level(line, true);
        assert!(plic.pending_for(PlicContext::Supervisor));
        assert_eq!(claim(&mut plic, PlicContext::Supervisor), 6);
    }

    #[test]
    fn level_low_clears_pending_and_source_zero_is_reserved() {
        let mut plic = Plic::new(8, 2);
        let line = plic.register_source(7).unwrap();
        priority(&mut plic, 7, 1);
        enable(&mut plic, PlicContext::Supervisor, 7);
        plic.set_level(line, true);
        assert!(plic.pending_for(PlicContext::Supervisor));
        plic.set_level(line, false);
        assert!(!plic.pending_for(PlicContext::Supervisor));

        assert_eq!(plic.read_word(0).unwrap(), 0);
        plic.write_word(0, 7).unwrap();
        assert_eq!(plic.read_word(0).unwrap(), 0);
        plic.write_word(0x0000_2000, u32::MAX).unwrap();
        assert_eq!(plic.read_word(0x0000_2000).unwrap() & 1, 0);
    }
}
