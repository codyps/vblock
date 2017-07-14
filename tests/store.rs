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
                        Some(openat::SimpleType::File) => {
                            let mut b = vec![];
                            let mut f = self.d.open_file(fna).map_err(|_| ::std::fmt::Error)?;
                            f.read_to_end(&mut b).map_err(|_| std::fmt::Error)?;
                            write!(fmt, " > {:?}\n", b)?;
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
    s.put_object(vblock::Kind::Piece, b"data").expect("failed to insert object");
    println!("{}", PrintDirRec::new(s.dir(), CStr::from_bytes_with_nul(b".\0").unwrap()));
    let oid = CStr::from_bytes_with_nul(
        b"5/e/7/3/a68dec8dd148419b366b51ae24332b62aed50fcb9a0c8f759cde90394db7e73ccc6eb08f86534bece2439a07723bbc5619b116681a0b563455e53e45651b\0"
    ).unwrap();
    let mut f = s.dir().open_file(oid).expect("could not open data file");
    let mut d = vec![];
    f.read_to_end(&mut d).expect("reading data failed");
    assert_eq!(&d[0..8], &[1,0,0,0,0,0,0,0]);
    assert_eq!(&d[8..], b"data");
}

#[test]
fn object_round_trip() {
    let tdb = tempdir::TempDir::new(module_path!()).expect("failed to open tempdir");
    let s = vblock::Store::with_path(tdb.path()).expect("failed to open store");
    let oid = s.put_object(vblock::Kind::Piece, b"data").expect("failed to insert object");
    let d = s.get_object(&oid).expect("getting object failed").expect("object does not exist");
    assert_eq!(d, b"data");
}

#[test]
fn piece_put_twice() {
    let tdb = tempdir::TempDir::new(module_path!()).expect("failed to open tempdir");
    let s = vblock::Store::with_path(tdb.path()).expect("failed to open store");
    s.put_object(vblock::Kind::Piece, b"hi").unwrap();
    s.put_object(vblock::Kind::Piece, b"hi").unwrap();
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

#[test]
fn blob_get() {
    let tdb = tempdir::TempDir::new(module_path!()).expect("failed to open tempdir");
    let s = vblock::Store::with_path(tdb.path()).expect("failed to open store");

    let oid1 = s.put_object(vblock::Kind::Piece, b"2").expect("insert object 1 failed");
    let oid2 = s.put_object(vblock::Kind::Piece, b"3").expect("insert object 2 failed");

    let mut p = vec![];
    vblock::Kind::Piece.write_to(&mut p).unwrap();
    p.extend(oid1.to_bytes());
    p.extend(oid2.to_bytes());

    let oid_blob = s.put_object(vblock::Kind::Blob, p).expect("insert blob failed");
    
    let d = s.get_blob(&oid_blob).expect("get failed").expect("object does not exist");

    assert_eq!(d, b"23");
}

#[test]
fn blob_round_trip_empty() {
    let tdb = tempdir::TempDir::new(module_path!()).expect("failed to open tempdir");
    let s = vblock::Store::with_path(tdb.path()).expect("failed to open store");
    let oid = s.put_blob(&[]).expect("put failed");
    println!("{}", PrintDirRec::new(s.dir(), CStr::from_bytes_with_nul(b".\0").unwrap()));
    let rt_data = s.get_blob(&oid).expect("get failed").expect("object does not exist");
    let e: &[u8] = &[];
    assert_eq!(e,&rt_data[..]);
}

fn blob_rt<A: AsRef<[u8]>>(a: A)
{
    let a = a.as_ref();
    let tdb = tempdir::TempDir::new(module_path!()).expect("failed to open tempdir");
    let s = vblock::Store::with_path(tdb.path()).expect("failed to open store");
    let oid = s.put_blob(&a).expect("put failed");
    println!("{}", PrintDirRec::new(s.dir(), CStr::from_bytes_with_nul(b".\0").unwrap()));
    let rt_data = s.get_blob(&oid).expect("get failed").expect("object does not exist");
    assert_eq!(a,&rt_data[..]);
}

#[test]
fn blob_round_trip_1() {
    blob_rt(&[44, 42, 6, 37, 83, 73, 23, 6, 10, 21, 13, 37, 21, 29, 74, 63, 78, 70, 42, 67, 87, 26, 61, 79, 90, 4, 62, 99, 47, 96, 62, 63, 33, 5, 17, 67, 5, 69, 66, 92, 8, 10, 60, 14, 42, 40, 38, 33, 11, 78, 25, 42, 65, 54, 28, 72, 77, 62, 87, 39, 90, 61, 78, 85][..]);
}

#[test]
fn blob_round_trip_2() {
    blob_rt(&[66, 30, 21, 7, 69, 39, 93, 16, 4, 70, 62, 14, 83, 98, 38, 33, 86, 0, 98, 16, 84, 82, 31, 11, 99, 70, 72, 91, 62, 52, 0][..]);
}

#[test]
fn blob_round_trip() {
    fn prop(data: Vec<u8>) -> bool {
        let tdb = tempdir::TempDir::new(module_path!()).expect("failed to open tempdir");
        let s = vblock::Store::with_path(tdb.path()).expect("failed to open store");
        
        let oid = match s.put_blob(&data[..]) {
            Ok(v) => v,
            Err(_) => return false,
        };

        let rt_data = match s.get_blob(&oid) {
            Ok(Some(v)) => v,
            _ => return false,
        };

        data == rt_data 
    }
    quickcheck::quickcheck(prop as fn(Vec<u8>) -> bool)
}
