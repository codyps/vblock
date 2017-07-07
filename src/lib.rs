extern crate openat;
extern crate rand;
extern crate hex;
extern crate sodalite;
extern crate hash_roll;

use hash_roll::Split2;
use std::ffi::{CString,CStr};

//extern crate sodalite;

mod fs;
use std::io::Read;
use fs::DirVblockExt;
use std::io::Write;
use std::io;
use openat::Dir;

/// 
///
/// Contains `Object`s identified by an object-id (`Oid`). Objects may contain 1 or more "values"
/// stored to different files, each with a given `name`.
///
/// `Piece`s are a type of object which have an `Oid` corresponding to the hash of their contents.
/// `Piece`s make up other types of things stored.
pub struct Store {
    base: openat::Dir,
}

/// Object Identifier
///
/// Very much like git, every object in a vblock store has a identifier that corresponds to it's
/// value.
///
// FIXME: we really want this to be both a series of bytes & a cstr.
//  - CStr is used for file paths
//  - bytes are used for file contents
pub struct Oid {
    inner: ::std::ffi::CString,
}

impl Oid {
    pub fn from_hex(key: &str) -> Result<Self,()> {
        let mut nh = Vec::with_capacity(key.len() + 1);
        let hv = b"0123456789abcdef";
        let hvu = b"ABCDEF";
        for c in key.as_bytes() {
            if hv.contains(c) {
                nh.push(*c)
            } else if hvu.contains(c) {
                nh.push(*c + (b'a' - b'A'))
            } else {
                return Err(())
            }
        }

        Ok(Oid {
            inner: ::std::ffi::CString::new(nh).unwrap()
        })
    }

    pub fn from_bytes<A: AsRef<[u8]>>(key: A) -> Self {
        // TODO: instead of converting & allocating, provide a view in hex?
        Oid {
            inner: ::std::ffi::CString::new(::hex::ToHex::to_hex(&key)).unwrap()
        }
    }

    pub fn from_data(data: &[u8]) -> Self {
        let mut key = [0u8;sodalite::HASH_LEN];
        sodalite::hash(&mut key, data);
        Oid::from_bytes(&key[..])
    }

    /// TODO: this is very Index like, see if we can make that usable.
    fn get_part(&self, index: usize) -> OidPart {
        let v = [self.inner.as_bytes()[index]];
        OidPart { inner: CString::new(&v[..]).unwrap() }
    }

    /// TODO: this is very Index like, see if we can make that usable.
    fn get_part_rem(&self, index_start: usize) -> OidPart {
        let v = &self.inner.as_bytes()[index_start..];
        OidPart { inner: CString::new(v).unwrap() }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        ::hex::FromHex::from_hex(&self.inner.as_bytes()).unwrap()
    }
}

struct OidPart {
    inner: CString
}

impl<'a> openat::AsPath for &'a OidPart {
    type Buffer = &'a CStr;
    fn to_path(self) -> Option<Self::Buffer> {
        Some(self.inner.as_ref())
    }
}

impl<'a> openat::AsPath for &'a Oid {
    type Buffer = &'a CStr;
    fn to_path(self) -> Option<Self::Buffer> {
        Some(self.inner.as_ref())
    }
}

impl Store {
    pub fn with_dir(d: openat::Dir) -> Self {
        Store {
            base: d
        }
    }

    pub fn with_path<P: openat::AsPath>(p: P) -> io::Result<Self> {
        let d = ::openat::Dir::open(p)?;
        Ok(Self::with_dir(d))
    }

    pub fn dir(&self) -> &::openat::Dir {
        &self.base
    }

    fn object_dir(&self, key: &Oid) -> io::Result<Dir> {
        // TODO: consider allowing configurable levels for key-splitting.
        let l = 3;
        let mut d = Vec::with_capacity(l);
        d.push(self.base.create_dir_open(&key.get_part(0))?);

        for i in 1..l {
            let n = d[i-1].create_dir_open(&key.get_part(i))?;
            d.push(n);
        }

        d[d.len()-1].create_dir_open(&key.get_part_rem(l))
    }

    /// TODO: consider multi-(name,data) API
    /// TODO: consider data being sourced incrimentally
    ///
    /// Note: `key` and `name` should only need to be valid `Path` fragments (`OsString`s). The
    /// restriction to `str` here could be lifted if needed.
    pub fn put_object(&self, key: &Oid, name: &str, data: &[u8]) -> io::Result<()>
    {
        // TODO: encapsulate logic around tempdir, tempfiles, and renaming to allow us to be cross
        // platform.
        let t = self.base.tempdir("vblock-temp.")?;
        let mut f = t.create_file(key, 0o666)?;
        f.write_all(data)?;
        let d = self.object_dir(key)?;
        ::openat::rename(&t, key, &d, name)?;
        Ok(())
    }

    /// TODO: consider data being read inrementally
    pub fn get_object(&self, key: &Oid, name: &str) -> io::Result<Vec<u8>> {
        let d = self.object_dir(key)?;
        let mut b = vec![];
        let mut f = d.open_file(name)?;
        f.read_to_end(&mut b)?;
        Ok(b)
    }

    pub fn put_piece<A: AsRef<[u8]>>(&self, data: A) -> io::Result<Oid>
    {
        // TODO: verify data if object already exists
        let data = data.as_ref();
        let oid = Oid::from_data(data);
        self.put_object(&oid, "piece", data)?;
        Ok(oid)
    }

    /// A blob is a list of pieces. That list is then also split into pieces (recursively)
    ///
    /// The Oid of a blob is the overall hash of the data, which simply contains the Oid of the
    /// top-level piece of the list of pieces.
    ///
    /// TODO: avoid needing the entire blob in memory at once. Use a streaming style api here.
    ///
    ///
    pub fn put_blob(&self, data: &[u8]) -> io::Result<Oid>
    {
        let oid = Oid::from_data(data);

        // build an object containing a list of pieces
        let mut pieces = vec![];

        let mut hr = hash_roll::bup::BupBuf::default();

        let mut data = data;
        while data.len() > 0 {
            let used = hr.push(data);
            let used = if used == 0 {
                data.len()
            } else {
                used
            };
            let oid = self.put_piece(data)?;
            pieces.extend(&[
                // entry_len: u16 // 64 + 8 = 72
                72, 0,
                // kind: u16      // 1 = (oid: [u8;64], len: u64)
                1,  0,
            ][..]);
            pieces.extend(oid.to_bytes());
            pieces.extend(&[
                used as u8,
                (used >> 8) as u8,
                (used >> 16) as u8,
                (used >> 24) as u8,
                (used >> 32) as u8,
                (used >> 40) as u8,
                (used >> 48) as u8,
                (used >> 56) as u8
            ][..]);
            data = &{data}[used..];
        }

        // FIXME: pieces should also be split.
        let p_oid = self.put_piece(pieces)?;
        self.put_object(&oid, "blob", &p_oid.to_bytes()[..])?;
        Ok(oid)
    }

    pub fn get_blob(&self, oid: &Oid) -> io::Result<Vec<u8>>
    {
        let pieces_oid = Oid::from_bytes(self.get_object(oid, "blob")?);
        let pieces = self.get_object(&pieces_oid, "piece")?; 
        let mut p = &pieces[..];
        let mut data = vec![];
        loop {
            if p.len() == 0 {
                return Ok(data)
            }

            if p.len() < 4 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{} spare bytes too small for header", p.len())));
            }

            let elen = p[0] as u16 | ((p[1] as u16) << 8);
            if elen != 72 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, format!("piece entry length is {} instead of 72", elen)));
            }

            let kind = p[2] as u16 | ((p[3] as u16) << 8);
            if kind != 1 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, format!("piece entry kind is {} instead of 1", kind)))
            }

            if p.len() < elen as usize {
                return Err(io::Error::new(io::ErrorKind::InvalidData, format!("piece entry len {} is not provided by piece desc which has {} bytes left",
                                                                              elen, p.len())));
            }

            let oid = Oid::from_bytes(&p[4..(4+64)]);

            data.extend(self.get_object(&oid, "piece")?);

            p = &{p}[(4+64+8)..]
        }
    }
}

