/*
use std::io::Write;

/// Provides a mechanism to split blocks given 
struct Spliter {
    average_size_log2: u8,

    // XXX: consider allowing this to be supplied externally for perf and flexibility
    scan_buffer: Vec<u8>,

    /// the next byte to be filled it, the last valid byte is right before this one.
    scan_next : usize,
}

impl Spliter {
    pub fn new(average_size_log2: u8, scan_len: usize) -> Splitter {
        assert!(average_size_log2 < 64);

        Spliter {
            average_size_log2: average_size_log2,
            scan_buffer: vec![0u8; scan_len],
            scan_next: 0,
        }
    }
}

impl Write for Splitter {
    fn write(&mut self, buf: &[u8]) -> ::std::io::Result<usize> {
        /* Iterate over the concatenation of scan_buffer's valid bytes and all of buf looking for
         * the termination condition to occur. If we find the termination condition, return early
         */

        Ok(buf.len())
    }

    fn flush(&mut self) -> ::std::io::Result<()>
    {
        Ok(())
    }
}
*/

#[test]
fn it_works() {
}

