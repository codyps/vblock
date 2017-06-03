fn to_cstr<P: ::openat::AsPath>(path: P) -> ::std::io::Result<P::Buffer> {
    path.to_path()
    .ok_or_else(|| {
        ::std::io::Error::new(::std::io::ErrorKind::InvalidInput,
                       "nul byte in file name")
    })
}

pub trait DirVblockExt {
    fn create_dir_open<P: ::openat::AsPath>(&self, path: P) -> ::std::io::Result<Self>
        where Self: Sized;
}

impl DirVblockExt for ::openat::Dir {
    /// Try to open a directory, creating it if it does not exist.
    /// 
    /// Allow for others to be attempting to create the directory at the same time as we are.
    /// 
    /// Use "ask for forgiveness" strategy: always try to open first before attempting to create.
    fn create_dir_open<P: ::openat::AsPath>(&self, path: P) -> ::std::io::Result<Self>
    {
        let path = to_cstr(path)?;
        let path = path.as_ref();
        match self.sub_dir(path) {
            Err(_) => {
                match self.create_dir(path, 0o777) {
                    Ok(_) => {
                        // XXX: can we do anything further here?
                        self.sub_dir(path)
                    },
                    Err(e) => {
                        if e.kind() == ::std::io::ErrorKind::AlreadyExists {
                            // XXX: can we do anything further here?
                            self.sub_dir(path)
                        } else {
                            // XXX: can we do anything further here?
                            Err(e)
                        }
                    }
                }
            }
            Ok(d1) => Ok(d1),
        }
    }
}

#[cfg(test)]
mod test {
    extern crate tempdir;
    macro_rules! check { ($e:expr) => (
        match $e {
            Ok(t) => t,
            Err(e) => panic!("{} failed with: {}", stringify!($e), e),
        }
    ) }

    use super::DirVblockExt;
    #[test]
    fn create_open() {
        let tdb = tempdir::TempDir::new(module_path!()).unwrap();
        let d = ::openat::Dir::open(tdb.path()).unwrap();
        let d2 = d.create_dir_open("x").unwrap();

        // can we see the dir with normal path methods?
        assert!(tdb.path().join("x").metadata().unwrap().is_dir());

        for e in d.list_dir(".").unwrap() {
            println!("{:?}", e);
        }

        // can the dir see the new dir it has created?
        let m = d.metadata("x").unwrap();
        println!("{:?}", m.simple_type());
        assert!(m.is_dir());

        // make sure that's not a fluke by trying to 
        assert!(d.metadata("y").is_err());

        // check that our 'tmp/x' doesn't have an unexpected subdir 'y'
        assert!(d2.metadata("y").is_err());

        // create that subdir 'tmp/x/y'
        d2.create_dir_open("y").unwrap();

        // check using std that 'tmp/x/y' exists
        assert!(tdb.path().join("x").join("y").metadata().unwrap().is_dir());

        // look at 'y' via 'tmp/x'
        assert!(d2.metadata("y").unwrap().is_dir());
    }

    #[test]
    fn create_open_concurrent_race() {
        for _ in 0..500 {
            let tdb = tempdir::TempDir::new(module_path!()).unwrap();
            let mut join = vec![];
            for _ in 0 ..8 {
                let b = tdb.path().to_owned();
                join.push(::std::thread::spawn(move || {
                    let mut d = ::openat::Dir::open(&b).unwrap();
                    check!(d.create_dir_open("a"));
                }));
            }

            join.drain(..).map(|join| join.join().unwrap()).count();
        }
    }
}
