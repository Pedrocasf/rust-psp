#![allow(
    stable_features,
    internal_features,
    clippy::bad_bit_mask,
    clippy::missing_safety_doc
)]
#![feature(
    asm_experimental_arch,
    alloc_error_handler,
    global_asm,
    const_loop,
    const_if_match,
    core_intrinsics,
    c_variadic,
    lang_items,
    rustc_attrs
)]
// For unwinding support
#![feature(std_internals, panic_info_message, panic_internals, c_unwind)]
#![cfg_attr(not(feature = "stub-only"), feature(panic_unwind))]
#![cfg_attr(feature = "std", feature(psp_std))]
// For the `const_generics` feature.
#![allow(incomplete_features)]
#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate paste;
#[cfg(not(feature = "stub-only"))]
extern crate alloc;
#[cfg(not(feature = "stub-only"))]
extern crate panic_unwind;

#[macro_use]
#[doc(hidden)]
#[cfg(not(feature = "stub-only"))]
pub mod debug;

#[macro_use]
mod vfpu;
mod eabi;
pub mod math;
pub mod sys;
#[cfg(not(feature = "stub-only"))]
pub mod test_runner;
#[cfg(not(feature = "stub-only"))]
pub mod vram_alloc;

#[cfg(not(feature = "stub-only"))]
mod alloc_impl;
#[cfg(not(feature = "stub-only"))]
pub mod panic;

#[cfg(not(feature = "stub-only"))]
mod screenshot;
#[cfg(not(feature = "stub-only"))]
pub use screenshot::*;

#[cfg(not(feature = "stub-only"))]
mod benchmark;
#[cfg(not(feature = "stub-only"))]
pub use benchmark::*;

#[cfg(not(feature = "stub-only"))]
mod constants;
#[cfg(not(feature = "stub-only"))]
pub use constants::*;

#[doc(hidden)]
pub use unstringify::unstringify;

#[cfg(not(feature = "std"))]
#[cfg(feature = "stub-only")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop()
    }
}

#[cfg(not(test))]
#[cfg_attr(not(bootstrap), rustc_std_internal_symbol)]
extern "C" fn __rust_foreign_exception() -> ! {
    loop {
        core::hint::spin_loop()
    }
}

#[cfg(feature = "std")]
pub use std::panic::catch_unwind;

#[cfg(all(not(feature = "std"), not(feature = "stub-only")))]
pub use panic::catch_unwind;

#[cfg(feature = "embedded-graphics")]
pub mod embedded_graphics;

#[repr(align(16))]
#[derive(Copy, Clone)]
pub struct Align16<T>(pub T);

#[cfg(all(target_os = "psp", not(feature = "stub-only")))]
core::arch::global_asm!(
    r#"
        .section .lib.ent.top, "a", @progbits
        .align 2
        .word 0
    .global __lib_ent_top
    __lib_ent_top:
        .section .lib.ent.btm, "a", @progbits
        .align 2
    .global __lib_ent_bottom
    __lib_ent_bottom:
        .word 0

        .section .lib.stub.top, "a", @progbits
        .align 2
        .word 0
    .global __lib_stub_top
    __lib_stub_top:
        .section .lib.stub.btm, "a", @progbits
        .align 2
    .global __lib_stub_bottom
    __lib_stub_bottom:
        .word 0
    "#
);

#[cfg(feature = "std")]
extern "C" {
    #[link_name = "main"]
    #[doc(hidden)]
    pub fn c_main(argc: isize, argv: *const *const u8) -> isize;
}

// Because code generated by `module!()` lives in user's crate, cfg attribute cannot be used there.
// Hence this macro.
#[cfg(feature = "std")]
#[doc(hidden)]
#[macro_export]
macro_rules! _start {
    ($_:expr, $argc:expr, $argv:expr) => {
        unsafe { $crate::c_main($argc as _, $argv as _) as _ }
    };
}
#[cfg(not(feature = "std"))]
#[doc(hidden)]
#[macro_export]
macro_rules! _start {
    ($psp_main:expr, $argc:expr, $argv:expr) => {{
        unsafe fn init_cwd(arg0: *mut u8) {
            let mut len = 0;
            while *arg0.add(len) != 0 {
                len += 1;
            }

            // Truncate until last '/'
            while len > 0 && *arg0.add(len - 1) != b'/' {
                len -= 1;
            }

            if len > 0 {
                let tmp = *arg0.add(len);
                *arg0.add(len) = 0;
                $crate::sys::sceIoChdir(arg0 as *const u8);
                *arg0.add(len) = tmp;
            }
        }

        if $argc > 0 {
            unsafe { init_cwd($argv as *mut u8) };
        }

        // TODO: Maybe print any error to debug screen?
        let _ = $crate::catch_unwind($psp_main);

        0
    }};
}

/// Declare a PSP module.
///
/// You must also define a `fn psp_main() { ... }` function in conjunction with
/// this macro.
#[macro_export]
macro_rules! module {
    ($name:expr, $version_major:expr, $version_minor: expr) => {
        #[doc(hidden)]
        mod __psp_module {
            #[no_mangle]
            #[link_section = ".rodata.sceModuleInfo"]
            #[used]
            static MODULE_INFO: $crate::Align16<$crate::sys::SceModuleInfo> =
                $crate::Align16($crate::sys::SceModuleInfo {
                    mod_attribute: 0,
                    mod_version: [$version_major, $version_minor],
                    mod_name: $crate::sys::SceModuleInfo::name($name),
                    terminal: 0,
                    gp_value: unsafe { &_gp },
                    stub_top: unsafe { &__lib_stub_top },
                    stub_end: unsafe { &__lib_stub_bottom },
                    ent_top: unsafe { &__lib_ent_top },
                    ent_end: unsafe { &__lib_ent_bottom },
                });

            extern "C" {
                static _gp: u8;
                static __lib_ent_bottom: u8;
                static __lib_ent_top: u8;
                static __lib_stub_bottom: u8;
                static __lib_stub_top: u8;
            }

            #[no_mangle]
            #[link_section = ".lib.ent"]
            #[used]
            static LIB_ENT: $crate::sys::SceLibraryEntry = $crate::sys::SceLibraryEntry {
                // TODO: Fix this?
                name: core::ptr::null(),
                version: ($version_major, $version_minor),
                attribute: $crate::sys::SceLibAttr::SCE_LIB_IS_SYSLIB,
                entry_len: 4,
                var_count: 1,
                func_count: 1,
                entry_table: &LIB_ENT_TABLE,
            };

            #[no_mangle]
            #[link_section = ".rodata.sceResident"]
            #[used]
            static LIB_ENT_TABLE: $crate::sys::SceLibraryEntryTable =
                $crate::sys::SceLibraryEntryTable {
                    module_start_nid: 0xd632acdb, // module_start
                    module_info_nid: 0xf01d73a7,  // SceModuleInfo
                    module_start: module_start,
                    module_info: &MODULE_INFO.0,
                };

            use core::ffi::c_void;

            #[no_mangle]
            extern "C" fn module_start(argc_bytes: usize, argv: *mut c_void) -> isize {
                extern "C" fn main_thread(argc: usize, argv: *mut c_void) -> i32 {
                    $crate::_start!(super::psp_main, argc, argv)
                }

                unsafe {
                    let id = $crate::sys::sceKernelCreateThread(
                        b"main_thread\0".as_ptr(),
                        main_thread,
                        // default priority of 32.
                        32,
                        // 256kb stack
                        256 * 1024,
                        $crate::sys::ThreadAttributes::USER | $crate::sys::ThreadAttributes::VFPU,
                        core::ptr::null_mut(),
                    );

                    $crate::sys::sceKernelStartThread(id, argc_bytes, argv);
                }

                0
            }
        }
    };
}

/// Enable the home button.
///
/// This API does not have destructor support yet. You can manually setup an
/// exit callback if you need this, see the source code of this function.
pub fn enable_home_button() {
    use core::{ffi::c_void, ptr};
    use sys::ThreadAttributes;

    unsafe {
        unsafe extern "C" fn exit_thread(_args: usize, _argp: *mut c_void) -> i32 {
            unsafe extern "C" fn exit_callback(_arg1: i32, _arg2: i32, _arg: *mut c_void) -> i32 {
                sys::sceKernelExitGame();
                0
            }

            let id = sys::sceKernelCreateCallback(
                &b"exit_callback\0"[0],
                exit_callback,
                ptr::null_mut(),
            );

            sys::sceKernelRegisterExitCallback(id);
            sys::sceKernelSleepThreadCB();

            0
        }

        // Enable the home button.
        let id = sys::sceKernelCreateThread(
            &b"exit_thread\0"[0],
            exit_thread,
            32,
            0x1000,
            ThreadAttributes::empty(),
            ptr::null_mut(),
        );

        sys::sceKernelStartThread(id, 0, ptr::null_mut());
    }
}
