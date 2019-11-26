#[cfg_attr(target_arch = "riscv32", path = "platform_riscv32.rs")]
#[cfg_attr(target_arch = "arm", path = "platform_arm.rs")]
#[cfg_attr(
    not(any(target_arch = "arm", target_arch = "riscv32")),
    path = "platform_mock.rs"
)]
mod platform;

use crate::callback::CallbackSubscription;
use crate::callback::SubscribableCallback;
use crate::shared_memory::SharedMemory;

pub mod raw {
    use super::platform;

    pub use platform::*;

    /// # Safety
    ///
    /// Yielding in the main function should be safe. Nevertheless, yielding manually is not required as this is already achieved by the `async` runtime.
    ///
    /// When yielding in callbacks, two problems can arise:
    /// - The guarantees of `FnMut` are violated. In this case, make sure your callback has `Fn` behavior.
    /// - Callbacks can get executed in a nested manner and overflow the stack quickly.
    ///
    /// This function is exported as `libtock::syscalls::raw::yieldk`. Do not reference this name directly. Its only purpose is to establish a back-channel from `async-support`, a patched version of `core` to `libtock-rs` via linking. This workaround has been chosen to keep the `core` crate free of dependencies on platform-specific syscall implementations and is expected to get removed as soon as possible.
    #[export_name = "libtock::syscalls::raw::yieldk"]
    pub unsafe fn yieldk() {
        platform::yieldk()
    }
}

pub fn subscribe<CB: SubscribableCallback>(
    driver_number: usize,
    subscribe_number: usize,
    callback: &mut CB,
) -> Result<CallbackSubscription, isize> {
    extern "C" fn c_callback<CB: SubscribableCallback>(
        arg1: usize,
        arg2: usize,
        arg3: usize,
        data: usize,
    ) {
        let callback = unsafe { &mut *(data as *mut CB) };
        callback.call_rust(arg1, arg2, arg3);
    }

    let return_code = {
        subscribe_fn(
            driver_number,
            subscribe_number,
            c_callback::<CB>,
            callback as *mut CB as usize,
        )
    };

    if return_code == 0 {
        Ok(CallbackSubscription::new(driver_number, subscribe_number))
    } else {
        Err(return_code)
    }
}

pub fn subscribe_fn(
    driver_number: usize,
    subscribe_number: usize,
    callback: extern "C" fn(usize, usize, usize, usize),
    userdata: usize,
) -> isize {
    unsafe {
        raw::subscribe(
            driver_number,
            subscribe_number,
            callback as *const _,
            userdata,
        )
    }
}

pub fn command(driver_number: usize, command_number: usize, arg1: usize, arg2: usize) -> isize {
    unsafe { raw::command(driver_number, command_number, arg1, arg2) }
}

// command1_insecure, is a variant of command() that only sets the first
// argument in the system call interface. It has the benefit of generating
// simpler assembly than command(), but it leaves the second argument's register
// as-is which leaks it to the kernel driver being called. Prefer to use
// command() instead of command1_insecure(), unless the benefit of generating
// simpler assembly outweighs the drawbacks of potentially leaking arbitrary
// information to the driver you are calling.
//
// At the moment, the only suitable use case for command1_insecure is the low
// level debug interface.

pub fn command1_insecure(driver_number: usize, command_number: usize, arg: usize) -> isize {
    unsafe { raw::command1(driver_number, command_number, arg) }
}

pub fn allow(
    driver_number: usize,
    allow_number: usize,
    buffer_to_share: &mut [u8],
) -> Result<SharedMemory, isize> {
    let len = buffer_to_share.len();
    let return_code = unsafe {
        raw::allow(
            driver_number,
            allow_number,
            buffer_to_share.as_mut_ptr(),
            len,
        )
    };
    if return_code == 0 {
        Ok(SharedMemory::new(
            driver_number,
            allow_number,
            buffer_to_share,
        ))
    } else {
        Err(return_code)
    }
}
