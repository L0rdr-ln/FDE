//! Memory for the pictures FDE's own programs hand to the compositor: an anonymous file that has no
//! name anywhere, so no other program can open, replace or read it (no file in /tmp to race for).

use std::os::fd::FromRawFd;

pub fn anonymous_file(name: &str) -> std::io::Result<std::fs::File> {
    let cname = std::ffi::CString::new(name).unwrap_or_default();
    let fd = unsafe { libc::memfd_create(cname.as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { std::fs::File::from_raw_fd(fd) })
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_file_has_no_name_and_holds_what_is_written() {
        use std::io::{Read, Seek, Write};
        let mut f = super::anonymous_file("test").unwrap();
        f.write_all(b"hello").unwrap();
        f.rewind().unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        assert_eq!(s, "hello");
        use std::os::fd::AsRawFd;
        let link = std::fs::read_link(format!("/proc/self/fd/{}", f.as_raw_fd())).unwrap();
        assert!(link.to_string_lossy().starts_with("/memfd:"), "{link:?}");
    }
}
