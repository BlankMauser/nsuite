use ringbuf::{traits::*, HeapRb};

use crate::gamepad::{self, Buttons, NPAD_CONNECTED};
use std::cell::UnsafeCell;

pub const FRAMES_PER_PAIR: usize = 3;
pub const CONTROLLERS_PER_PAIR: usize = 2;
pub const MAX_PLAYERS: usize = 8;
pub const MAX_PAIRS: usize = MAX_PLAYERS / CONTROLLERS_PER_PAIR;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct ControllerInput {
    pub buttons: Buttons,
    pub connected: bool,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct InputFrame {
    pub controllers: [ControllerInput; CONTROLLERS_PER_PAIR],
}

pub struct ControllerPairFrames {
    pub pair_index: u8,
    frames: HeapRb<InputFrame>,
}

impl ControllerPairFrames {
    #[inline]
    pub fn new(pair_index: u8) -> Self {
        Self {
            pair_index,
            frames: HeapRb::new(FRAMES_PER_PAIR),
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.frames.occupied_len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    #[inline(always)]
    pub fn pop_oldest(&mut self) -> Option<InputFrame> {
        self.frames.try_pop()
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        while self.frames.try_pop().is_some() {}
    }

    #[inline(always)]
    pub fn advance_next_index(&mut self, frame: InputFrame) {
        if self.frames.is_full() {
            let _ = self.frames.try_pop();
        }

        let (left, right) = self.frames.vacant_slices_mut();
        unsafe {
            let slot = if !left.is_empty() {
                left.get_unchecked_mut(0)
            } else {
                right.get_unchecked_mut(0)
            };
            slot.write(frame);
            self.frames.advance_write_index(1);
        }
    }

    #[inline(always)]
    pub fn latest(&self) -> Option<&InputFrame> {
        self.frames.last()
    }

    #[inline(always)]
    pub fn latest_mut(&mut self) -> Option<&mut InputFrame> {
        self.frames.last_mut()
    }

    #[inline(always)]
    pub fn latest_raw(&self) -> *const InputFrame {
        self.latest()
            .map_or(std::ptr::null(), |frame| frame as *const InputFrame)
    }

    #[inline(always)]
    pub fn latest_raw_mut(&mut self) -> *mut InputFrame {
        self.latest_mut()
            .map_or(std::ptr::null_mut(), |frame| frame as *mut InputFrame)
    }

    #[inline(always)]
    pub const fn controller_id(&self, local_index: usize) -> u8 {
        ((self.pair_index as usize * CONTROLLERS_PER_PAIR) + local_index) as u8
    }

    #[inline(always)]
    pub fn first_controller_with_all(&self, mask: Buttons) -> Option<u8> {
        let frame = self.latest()?;
        for i in 0..CONTROLLERS_PER_PAIR {
            let c = frame.controllers[i];
            if c.connected && (c.buttons & mask) == mask {
                return Some(self.controller_id(i));
            }
        }
        None
    }

    #[inline(always)]
    pub fn first_controller_with_any(&self, mask: Buttons) -> Option<u8> {
        let frame = self.latest()?;
        for i in 0..CONTROLLERS_PER_PAIR {
            let c = frame.controllers[i];
            if c.connected && (c.buttons & mask) != 0 {
                return Some(self.controller_id(i));
            }
        }
        None
    }
}

pub struct InputFrameStore {
    pairs: Vec<ControllerPairFrames>,
    tracked_pair_count: u8,
}

impl InputFrameStore {
    #[inline]
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            tracked_pair_count: 0,
        }
    }

    #[inline(always)]
    pub fn tracked_pair_count(&self) -> u8 {
        self.tracked_pair_count
    }

    #[inline(always)]
    pub fn allocated_pair_count(&self) -> u8 {
        self.pairs.len() as u8
    }

    #[inline(always)]
    pub fn pairs(&self) -> &[ControllerPairFrames] {
        &self.pairs
    }

    #[inline(always)]
    pub fn pairs_mut(&mut self) -> &mut [ControllerPairFrames] {
        &mut self.pairs
    }

    #[inline(always)]
    pub fn active_pairs_mut(&mut self) -> &mut [ControllerPairFrames] {
        let active = self.tracked_pair_count as usize;
        &mut self.pairs[..active]
    }

    #[inline(always)]
    pub fn free_memory(&mut self) {
        self.pairs.clear();
        self.pairs.shrink_to_fit();
        self.tracked_pair_count = 0;
    }

    #[inline(always)]
    pub fn free(self) {}

    #[inline(always)]
    fn resize_pairs_for_required(&mut self, required_pairs: usize) {
        let clamped = required_pairs.min(MAX_PAIRS);
        if self.pairs.len() > clamped {
            self.pairs.truncate(clamped);
            self.pairs.shrink_to(clamped);
        } else {
            while self.pairs.len() < clamped {
                self.pairs.push(ControllerPairFrames::new(self.pairs.len() as u8));
            }
        }
    }

    #[inline(always)]
    pub unsafe fn capture_next(&mut self) -> u8 {
        let connected = gamepad::check_inputs() as usize;
        let required_pairs = connected.div_ceil(CONTROLLERS_PER_PAIR);
        self.resize_pairs_for_required(required_pairs);
        self.tracked_pair_count = required_pairs as u8;

        for pair in 0..required_pairs {
            let id0 = (pair * CONTROLLERS_PER_PAIR) as u32;
            let id1 = id0 + 1;
            let p0 = gamepad::probe_input(id0);
            let p1 = gamepad::probe_input(id1);

            let frame = InputFrame {
                controllers: [
                    ControllerInput {
                        buttons: p0.state.buttons,
                        connected: (p0.state.flags & NPAD_CONNECTED) != 0,
                    },
                    ControllerInput {
                        buttons: p1.state.buttons,
                        connected: (p1.state.flags & NPAD_CONNECTED) != 0,
                    },
                ],
            };
            self.pairs[pair].advance_next_index(frame);
        }

        self.tracked_pair_count
    }

    #[inline(always)]
    pub fn first_controller_with_all(&self, mask: Buttons) -> Option<u8> {
        for pair in &self.pairs[..self.tracked_pair_count as usize] {
            if let Some(id) = pair.first_controller_with_all(mask) {
                return Some(id);
            }
        }
        None
    }

    #[inline(always)]
    pub fn first_controller_with_any(&self, mask: Buttons) -> Option<u8> {
        for pair in &self.pairs[..self.tracked_pair_count as usize] {
            if let Some(id) = pair.first_controller_with_any(mask) {
                return Some(id);
            }
        }
        None
    }

    #[inline(always)]
    pub unsafe fn check_inputs(&mut self) -> CheckedInputs<'_> {
        self.capture_next();
        CheckedInputs {
            iter: self.active_pairs_mut().iter_mut(),
        }
    }
}

impl Default for InputFrameStore {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CheckedInputs<'a> {
    iter: std::slice::IterMut<'a, ControllerPairFrames>,
}

impl<'a> Iterator for CheckedInputs<'a> {
    type Item = &'a mut ControllerPairFrames;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

#[inline(always)]
pub fn for_each_matching_in<'a, I, F>(pairs: I, mask: Buttons, mut on_match: F)
where
    I: IntoIterator<Item = &'a mut ControllerPairFrames>,
    F: FnMut(u8, Buttons),
{
    for pair in pairs {
        let Some(frame) = pair.latest() else {
            continue;
        };
        for i in 0..CONTROLLERS_PER_PAIR {
            let c = frame.controllers[i];
            if c.connected && (c.buttons & mask) == mask {
                on_match(pair.controller_id(i), c.buttons);
            }
        }
    }
}

struct GlobalInputFrameStore(UnsafeCell<Option<InputFrameStore>>);

unsafe impl Sync for GlobalInputFrameStore {}

static GLOBAL_INPUT_FRAME_STORE: GlobalInputFrameStore = GlobalInputFrameStore(UnsafeCell::new(None));

#[allow(non_snake_case)]
#[inline(always)]
pub unsafe fn CheckInputs() -> CheckedInputs<'static> {
    let slot = &mut *GLOBAL_INPUT_FRAME_STORE.0.get();
    let store = slot.get_or_insert_with(InputFrameStore::new);
    store.check_inputs()
}

#[allow(non_snake_case)]
#[inline(always)]
pub unsafe fn CurrentInputs() -> CheckedInputs<'static> {
    let slot = &mut *GLOBAL_INPUT_FRAME_STORE.0.get();
    let store = slot.get_or_insert_with(InputFrameStore::new);
    CheckedInputs {
        iter: store.active_pairs_mut().iter_mut(),
    }
}

#[allow(non_snake_case)]
#[inline(always)]
pub unsafe fn FreeInputFrameMemory() {
    if let Some(store) = (&mut *GLOBAL_INPUT_FRAME_STORE.0.get()).as_mut() {
        store.free_memory();
    }
}

#[allow(non_snake_case)]
#[inline(always)]
pub unsafe fn CheckInputsWithMask<F>(mask: Buttons, on_match: F)
where
    F: FnMut(u8, Buttons),
{
    for_each_matching_in(CurrentInputs(), mask, on_match);
}

#[allow(non_snake_case)]
#[inline(always)]
pub unsafe fn FirstControllerWithAll(mask: Buttons) -> Option<u8> {
    let slot = &mut *GLOBAL_INPUT_FRAME_STORE.0.get();
    slot.as_ref()?.first_controller_with_all(mask)
}

#[allow(non_snake_case)]
#[inline(always)]
pub unsafe fn FirstControllerWithAny(mask: Buttons) -> Option<u8> {
    let slot = &mut *GLOBAL_INPUT_FRAME_STORE.0.get();
    slot.as_ref()?.first_controller_with_any(mask)
}
