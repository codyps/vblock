/*
//extern crate fs_at;
extern crate sodalite;

pub use std::io::Result;

/// A location that is either occupied or unoccupied.
///
/// Occupation status may change rather quickly, 
pub struct Entry<'a> {
    p: &'a Store,
    k: Key,
}

impl Entry {
    pub fn
}

pub struct Store {
    base: fs_at::Dir,
}

impl Store {
    pub fn with_dir(d: fs_at::Dir) -> Self {
        Store {
            base: d
        }
    }

    pub fn store(&self, data: &[u8]) -> Result<usize>
    {
    }

    pub fn entry(&self, key: &str) -> Result<Entry>
    {

    }
}
*/

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

