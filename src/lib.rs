extern crate openat;
//extern crate sodalite;

mod fs;
use fs::DirVblockExt;

pub use std::io::Result;

pub struct Store {
    base: openat::Dir,
}

impl Store {
    pub fn with_dir(d: openat::Dir) -> Self {
        Store {
            base: d
        }
    }

    /// TODO: consider multi-(name,data) API
    /// TODO: consider data being sourced incrimentally
    ///
    /// Note: `key` and `name` should only need to be valid `Path` fragments (`OsString`s). The
    /// restriction to `str` here could be lifted if needed.
    pub fn put(&self, key: &str, name: &str, data: &[u8]) -> Result<()>
    {
        /*
        let t = self.base.tempdir("vblock")?;
        t.create_dir_open(

        /// TODO: consider allowing configurable levels for key-splitting.
        let d: [Dir;3];
        d[0] = self.base.create_dir_open(key[0])?;

        for i in 1..d.len() {
            d[i] = d[i-1].create_dir_open(key[i])?;
        }

        let d_f = d[d.len()-1].create_dir_open(key[d.len()..])?;
        */
        unimplemented!()
    }

    /// TODO: consider data being read inrementally
    pub fn get(&self, key: &str, name: &str) -> Result<Vec<u8>> {
        unimplemented!()
    }
}

#[test]
fn it_works() {
}

