#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct RangeUsize {
    pub start: usize,
    pub end: usize,
}

impl RangeUsize {
    #[inline(always)]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[inline(always)]
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}
