#[macro_export]
macro_rules! nn_lookup_symbol_slot {
    (
        $(#[$meta:meta])*
        $vis:vis static $slot:ident : $ty:ty = $symbol:expr;
        init fn $init_fn:ident;
        get fn $get_fn:ident;
    ) => {
        $(#[$meta])*
        $vis static $slot: core::sync::atomic::AtomicUsize =
            core::sync::atomic::AtomicUsize::new(0);

        #[inline(always)]
        $vis unsafe fn $init_fn() -> bool {
            if let Some(addr) = $crate::symbol::lookup_symbol_addr($symbol) {
                $slot.store(addr, core::sync::atomic::Ordering::Release);
                true
            } else {
                false
            }
        }

        #[inline(always)]
        $vis unsafe fn $get_fn() -> Option<$ty> {
            let addr = $slot.load(core::sync::atomic::Ordering::Acquire);
            if addr == 0 {
                None
            } else {
                union Cast<F: Copy> {
                    raw: usize,
                    typed: F,
                }
                Some(Cast::<$ty> { raw: addr }.typed)
            }
        }
    };
}

#[macro_export]
macro_rules! nn_lookup_symbol_fn {
    (
        $(#[$meta:meta])*
        $vis:vis static $slot:ident : $ty:ty = $symbol:expr;
        init fn $init_fn:ident;
        get fn $get_fn:ident;
        get_unchecked fn $get_unchecked_fn:ident;
    ) => {
        $(#[$meta])*
        $vis static $slot: core::sync::atomic::AtomicUsize =
            core::sync::atomic::AtomicUsize::new(0);

        #[inline(always)]
        $vis unsafe fn $init_fn() -> bool {
            if let Some(addr) = $crate::symbol::lookup_symbol_addr($symbol) {
                $slot.store(addr, core::sync::atomic::Ordering::Release);
                true
            } else {
                false
            }
        }

        #[inline(always)]
        $vis unsafe fn $get_fn() -> Option<$ty> {
            let addr = $slot.load(core::sync::atomic::Ordering::Acquire);
            if addr == 0 {
                None
            } else {
                Some($crate::symbol::cast_addr::<$ty>(addr))
            }
        }

        #[inline(always)]
        $vis unsafe fn $get_unchecked_fn() -> $ty {
            $crate::symbol::cast_addr::<$ty>($slot.load(core::sync::atomic::Ordering::Acquire))
        }
    };
}

#[macro_export]
macro_rules! logN {
    () => {{
        let line = ::std::format!("[nsuite][{}]", module_path!());
        $crate::logger::log_line(&line);
    }};
    (target: $target:expr, $($arg:tt)*) => {{
        let msg = ::std::format!($($arg)*);
        let line = ::std::format!("[nsuite][{}] {}", $target, msg);
        $crate::logger::log_line(&line);
    }};
    ($($arg:tt)*) => {{
        let msg = ::std::format!($($arg)*);
        let line = ::std::format!("[nsuite][{}] {}", module_path!(), msg);
        $crate::logger::log_line(&line);
    }};
}

// Compatibility aliases for older callsites.
#[macro_export]
macro_rules! nlog {
    () => {{
        $crate::logN!();
    }};
    ($($arg:tt)*) => {{
        $crate::logN!($($arg)*);
    }};
}

#[macro_export]
macro_rules! nlogln {
    ($($arg:tt)*) => {{
        $crate::logN!($($arg)*);
    }};
}
