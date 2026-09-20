//! Inputs ending exactly at a PROT_NONE page must never fault (no over-read).
#[cfg(unix)]
#[cfg_attr(miri, ignore)] // mprotect is not modelled by Miri; the Miri run covers the same paths via tests/vectors.rs
#[test]
fn no_read_past_end() {
    use std::ptr;
    let page = 4096usize;
    let pages = 4;
    unsafe {
        let p = libc::mmap(ptr::null_mut(), page * pages, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS, -1, 0) as *mut u8;
        assert_ne!(p as isize, -1);
        // last page unreadable
        assert_eq!(libc::mprotect(p.add(page * (pages - 1)) as *mut libc::c_void, page, libc::PROT_NONE), 0);
        let end = p.add(page * (pages - 1));
        for i in 0..page * (pages - 1) {
            *p.add(i) = (i * 31 + 7) as u8;
        }
        let mut lens: Vec<usize> = (0..=1300).collect();
        lens.extend([2048, 4096, 4097, 8192, 12288]);
        for &len in &lens {
            let start = end.sub(len);
            let bytes = std::slice::from_raw_parts(start, len);
            let want = lanehash::aes::spec::hash128_spec(bytes, 3);
            assert_eq!(lanehash::aes::hash128(bytes, 3), want);
            let mut st = lanehash::aes::Stream::new(3);
            st.update(bytes);
            assert_eq!(st.finish128(), want, "stream len={len}");
        }
        // also inputs starting right after a PROT_NONE page (under-read check)
        assert_eq!(libc::mprotect(p as *mut libc::c_void, page, libc::PROT_NONE), 0);
        for &len in &lens {
            if len > page * (pages - 2) {
                continue;
            }
            let start = p.add(page);
            let bytes = std::slice::from_raw_parts(start, len);
            let _ = lanehash::aes::hash128(bytes, 5);
        }
        libc::munmap(p as *mut libc::c_void, page * pages);
    }
}
