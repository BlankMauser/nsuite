use std::sync::atomic::{AtomicUsize, Ordering};

#[inline(always)]
pub unsafe fn lookup_symbol_addr(symbol_nul: &'static [u8]) -> Option<usize> {
    let mut addr = 0usize;
    let rc = skyline::nn::ro::LookupSymbol(&mut addr, symbol_nul.as_ptr());
    if rc == 0 && addr != 0 {
        Some(addr)
    } else {
        None
    }
}

#[inline(always)]
pub unsafe fn cast_addr<F: Copy>(addr: usize) -> F {
    union Cast<F: Copy> {
        raw: usize,
        typed: F,
    }
    Cast::<F> { raw: addr }.typed
}

#[inline(always)]
pub unsafe fn lookup_symbol_fn<F: Copy>(symbol_nul: &'static [u8]) -> Option<F> {
    match lookup_symbol_addr(symbol_nul) {
        Some(addr) => Some(cast_addr::<F>(addr)),
        None => None,
    }
}

/// Caches a looked-up symbol address for fast repeated calls.
pub struct CachedSymbol {
    addr: AtomicUsize,
    symbol_nul: &'static [u8],
}

impl CachedSymbol {
    pub const fn new(symbol_nul: &'static [u8]) -> Self {
        Self {
            addr: AtomicUsize::new(0),
            symbol_nul,
        }
    }

    #[inline(always)]
    pub unsafe fn init(&self) -> bool {
        if let Some(addr) = lookup_symbol_addr(self.symbol_nul) {
            self.addr.store(addr, Ordering::Release);
            true
        } else {
            false
        }
    }

    #[inline(always)]
    pub fn is_initialized(&self) -> bool {
        self.addr.load(Ordering::Acquire) != 0
    }

    #[inline(always)]
    pub fn address(&self) -> usize {
        self.addr.load(Ordering::Acquire)
    }

    #[inline(always)]
    pub unsafe fn get<F: Copy>(&self) -> Option<F> {
        let addr = self.address();
        if addr == 0 {
            None
        } else {
            Some(cast_addr::<F>(addr))
        }
    }

    #[inline(always)]
    pub unsafe fn get_unchecked<F: Copy>(&self) -> F {
        cast_addr::<F>(self.address())
    }
}
