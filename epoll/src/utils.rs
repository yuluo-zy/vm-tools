use std::ffi::c_void;
use std::os::fd::RawFd;
use libc::read;
use log::warn;

pub fn register_event_helper(){}

pub fn unregister_event_helper(){}


/// 这段代码用于从一个文件描述符 (fd) 读取数据。
//
// `read` 是一个 POSIX 系统调用，它从文件描述符读取数据到内存缓冲区。在这段代码中，被读取的数据存储在 `value` 变量中。
//
// `read_fd` 函数的参数 `fd` 是一个类型为 `RawFd` 的文件描述符，它是一个整数，用于标识一个特定的打开文件。
//
// 该函数首先定义了一个 `u64` 类型的变量 `value`，并初始化为 0。然后它使用 `read` 系统调用从 `fd` 读取数据，并将结果保存到 `value` 中。这里使用 `unsafe` 是因为 `read` 系统调用可能会失败，如果失败，它将返回 -1。
//
// 接着，函数检查 `read` 的返回值。如果返回值是 -1，那么表示读取失败，它将输出一个警告消息 "Failed to read fd"。
//
// 最后，函数返回从文件描述符读取的值。
//
// 注意：`read` 系统调用通常用于从文件或者 socket 读取数据，但是在这个情况下，它被用于从一个 eventfd 读取数据。eventfd 是一种特殊的文件描述符，它可以用于事件通知。当有事件发生时，可以通过读取 eventfd 来获取事件的信息。
pub fn read_fd(fd: RawFd) -> u64 {
    let mut value: u64 = 0;

    // SAFETY: this is called by notifier handler and notifier handler
    // is executed with fd is is valid. The value is defined above thus
    // valid too.
    let ret = unsafe {
        read(
            fd,
            &mut value as *mut u64 as *mut c_void,
            std::mem::size_of::<u64>(),
        )
    };

    if ret == -1 {
        warn!("Failed to read fd");
    }

    value
}

use thiserror::Error;

#[derive(Error, Debug)]
pub enum UtilError {
    #[error("Notifier Operation non allowed.")]
    BadNotifierOperation,
    #[error("Found no parked fd {0}.")]
    NoParkedFd(i32),
    #[error("The fd {0} is not registered in epoll.")]
    NoRegisterFd(i32),
    #[error("Found bad syscall, error is {0} .")]
    BadSyscall(std::io::Error),
}