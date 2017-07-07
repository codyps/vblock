extern crate tempdir;
extern crate openat;
extern crate vblock;
extern crate rand;
extern crate quickcheck;

use openat::Dir;
use std::ffi::{CStr, CString};
use ::std::os::unix::ffi::OsStrExt;
use std::io::Read;

/*
macro_rules! check { ($e:expr) => (
        match $e {
            Ok(t) => t,
            Err(e) => panic!("{} failed with: {}", stringify!($e), e),
        }
) }
*/


#[derive(Debug, Clone)]
struct PrintDirRec<'a> {
    parent_path: &'a CStr,
    d: &'a Dir    
}

impl<'a> PrintDirRec<'a> {
    fn new(d: &'a Dir, parent_path: &'a CStr) -> Self {
        PrintDirRec { d: d, parent_path: parent_path }
    }
}

impl<'a> ::std::fmt::Display for PrintDirRec<'a> {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        let i = self.d.list_dir(".").map_err(|_| ::std::fmt::Error)?;
        for e in i {
            match e {
                Ok(e) => {
                    let st = e.simple_type();
                    let fna = e.file_name();
                    let mut npp : Vec<u8> = Vec::new();
                    npp.extend(self.parent_path.to_bytes());
                    npp.push(b'/');
                    npp.extend(fna.as_bytes());
                    let npp = CString::new(npp).map_err(|_| ::std::fmt::Error)?;
                    write!(fmt, "{:?} {:?}\n", npp, st)?;
                    match st {
                        Some(openat::SimpleType::Dir) => {
                            let nd = self.d.sub_dir(fna).map_err(|_| ::std::fmt::Error)?;
                            write!(fmt, "{}", PrintDirRec::new(&nd, npp.as_ref()))?;
                        },
                        _ => {}
                    }
                },
                Err(_) => {
                    return Err(::std::fmt::Error)
                },
            }
        }
        Ok(())
    }
}

#[test]
fn object_put() {
    let tdb = tempdir::TempDir::new(module_path!()).expect("failed to open tempdir");
    let s = vblock::Store::with_path(tdb.path()).expect("failed to open store");
    let oid = vblock::Oid::from_hex("0123456789").expect("failed to construct Oid");
    s.put_object(&oid, "this-name", b"data").expect("failed to insert object");
    let mut f = s.dir().open_file(CString::new(b"0/1/2/3456789/this-name".as_ref()).unwrap().as_ref()).expect("could not open data file");
    let mut d = vec![];
    f.read_to_end(&mut d).expect("reading data failed");
    assert_eq!(d, b"data");
}

#[test]
fn object_round_trip() {
    let tdb = tempdir::TempDir::new(module_path!()).expect("failed to open tempdir");
    let s = vblock::Store::with_path(tdb.path()).expect("failed to open store");
    let oid = vblock::Oid::from_hex("0123456789").expect("failed to construct Oid");
    s.put_object(&oid, "this-name", b"data").expect("failed to insert object");
    let d = s.get_object(&oid, "this-name").expect("getting object failed");
    assert_eq!(d, b"data");
}

#[test]
fn piece_put() {
    let tdb = tempdir::TempDir::new(module_path!()).expect("failed to open tempdir");
    let s = vblock::Store::with_path(tdb.path()).expect("failed to open store");
    s.put_piece(b"hi").unwrap();
}

#[test]
fn piece_round_trip() {
    let tdb = tempdir::TempDir::new(module_path!()).expect("failed to open tempdir");
    let s = vblock::Store::with_path(tdb.path()).expect("failed to open store");
    s.put_piece(b"hi").expect("putting piece failed");
    let d = s.get_object(&vblock::Oid::from_data(b"hi"), "piece").expect("getting piece failed");
    assert_eq!(d, b"hi");
}

#[test]
fn piece_put_twice() {
    let tdb = tempdir::TempDir::new(module_path!()).expect("failed to open tempdir");
    let s = vblock::Store::with_path(tdb.path()).expect("failed to open store");
    s.put_piece(b"hi").unwrap();
    s.put_piece(b"hi").unwrap();
}

#[test]
fn blob_put() {
    fn prop(data: Vec<u8>) -> bool {
        let tdb = tempdir::TempDir::new(module_path!()).expect("failed to open tempdir");
        let s = vblock::Store::with_path(tdb.path()).expect("failed to open store");
        s.put_blob(&data[..]).is_ok()
    }
    quickcheck::quickcheck(prop as fn(Vec<u8>) -> bool)
}
