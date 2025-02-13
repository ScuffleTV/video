/// Returns true if the error is a fatal TCP error.
///
/// Not all errors are fatal, some can be ignored.
pub fn is_fatal_tcp_error(err: &std::io::Error) -> bool {
    matches!(
        err.raw_os_error(),
        Some(libc::EFAULT)
            | Some(libc::EINVAL)
            | Some(libc::ENFILE)
            | Some(libc::EMFILE)
            | Some(libc::ENOBUFS)
            | Some(libc::ENOMEM)
    )
}
