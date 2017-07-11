extern crate openat;
extern crate rand;
extern crate hex;
extern crate sodalite;
extern crate hash_roll;
extern crate byteorder;

use byteorder::ByteOrder;
use hash_roll::Split2;
use std::ffi::{CString,CStr};
use std::io::Seek;

mod fs;
use std::io::Read;
use fs::DirVblockExt;
use std::io::Write;
use std::io;
use std::io::Cursor;
use openat::{Dir,DirIter};

/// Contains `Object`s identified by an object-id (`Oid`). Objects all have a Kind and have zero or
/// more bytes of data. `Oid`s are the hash of the `kind + data` of the object.
/// 
/// The `Kind` of an object defines the interpretation of the object's bytes.
///
/// A `Piece` is the most basic type of object. Its data is just bytes, with no vblock level
/// interpretation.
///
/// `Blob`s contain a list of `Oid`s which refer to other `Blob`s or to `Pieces`.
/// 
/// TODO: right now oids/keys are tied to the disk format, consider allowing oids/keys that are
/// related by aren't the direct hash of the vblock files. For example, allowing the hash of an
/// entire file to be tracked may be useful.
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
#[derive(Debug,Eq,PartialEq,Clone)]
pub struct Oid {
    inner: ::std::ffi::CString,
}

/// Data stored has a given kind which controls it's interpretation
#[derive(Debug,Eq,PartialEq,Clone,Copy)]
pub enum Kind {
    /// Actual data, a plain sequence of bytes which has no further meaning to vblock.
    // XXX: consider name here, everything kind of a piece.
    Piece,

    /// A list of objects (`Blob`s & `Piece`s) which taken together compose a single sequence of
    /// bytes.
    Blob,

    /// A single level of a filesystem tree.
    // XXX: consider multiple levels in 1.
    // XXX: consider how splitting of large trees is handled.
    Tree,
}

impl Kind {
    fn raw(&self) -> u64 {
        match *self {
            Kind::Piece => 1,
            Kind::Blob =>  2,
            Kind::Tree  => 3,
        }
    }

    fn from_bytes(d: &[u8]) -> io::Result<Self> {
        match byteorder::LittleEndian::read_u64(&d[..]) {
            1 => Ok(Kind::Piece),
            2 => Ok(Kind::Blob),
            3 => Ok(Kind::Tree),
            e => Err(io::Error::new(io::ErrorKind::InvalidData, format!("kind {:?} is invalid", e))),
        }
    }

    fn as_bytes(&self) -> [u8;8] {
        let mut x = [0u8;8];
        byteorder::LittleEndian::write_u64(&mut x[..], self.raw());
        x
    }

    fn len() -> usize {
        8
    }
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

    fn from_data<A: AsRef<[u8]>>(data: A) -> Self {
        let mut key = [0u8;sodalite::HASH_LEN];
        sodalite::hash(&mut key, data.as_ref());
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

    fn split_ct(&self) -> usize
    {
        4
    }

    fn object_dir(&self, key: &Oid) -> io::Result<Dir> {
        // TODO: consider allowing configurable levels for key-splitting.
        let l = self.split_ct();
        let mut d = Vec::with_capacity(l);
        d.push(self.base.create_dir_open(&key.get_part(0))?);

        for i in 1..(l-1) {
            let n = d[i-1].create_dir_open(&key.get_part(i))?;
            d.push(n);
        }

        d[l-2].create_dir_open(&key.get_part(l-1))
    }

    fn object_name(&self, key: &Oid) -> OidPart
    {
        key.get_part_rem(self.split_ct())
    }

    /// TODO: consider multi-(name,data) API
    /// TODO: consider data being sourced incrimentally
    ///
    /// Note: `key` and `name` should only need to be valid `Path` fragments (`OsString`s). The
    /// restriction to `str` here could be lifted if needed.
    pub fn put_object<A: AsRef<[u8]>>(&self, kind: Kind, data: A) -> io::Result<Oid>
    {
        let mut o = self.put(kind)?;
        o.write_all(data.as_ref())?;
        o.commit()
    }

    pub fn get_object(&self, key: &Oid) -> io::Result<Option<Vec<u8>>> {
        let mut v = match self.get(key)? {
            Some(x) => x,
            None => return Ok(None),
        };
        let mut b = vec![];
        v.read_to_end(&mut b)?;
        Ok(Some(b))
    }

    pub fn put<'a>(&'a self, kind: Kind) -> io::Result<ObjectBuilder<'a>> {
        ObjectBuilder::new(self, kind)
    }

    pub fn get<'a>(&'a self, oid: &Oid) -> io::Result<Option<Object<'a>>> {
        Object::from_oid(self, oid.clone())
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
        // build an object containing a list of pieces
        let mut pieces = vec![];
        let mut hr = hash_roll::bup::BupBuf::default();

        let mut data = data;

        if data.len() == 0 {
            return self.put_object(Kind::Piece, data);
        }

        while data.len() > 0 {
            let used = hr.push(data);
            let used = if used == 0 {
                if pieces.len() == 0 {
                    return self.put_object(Kind::Piece, data);
                } else {
                    data.len()
                }
            } else {
                if used == data.len() && pieces.len() == 0 {
                    return self.put_object(Kind::Piece, data);
                } else {
                    used
                }
            };

            let oid = self.put_object(Kind::Piece, data)?;
            pieces.extend(&[
                // entry_len: u16 // 4 + 64 + 8 = 76
                76, 0,
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
        self.put_object(Kind::Blob, pieces)
    }

    // TODO: logically, this should probably be handled by Object or similar directly, and allow us
    // to seek & so forth.
    pub fn get_blob(&self, oid: &Oid) -> io::Result<Option<Vec<u8>>>
    {
        let mut o = match self.get(oid)? {
                Some(v) => v, None => return Ok(None),
        };

        let mut data = vec![];
        match o.kind() {
            Kind::Blob => {
                // resolve other items
                let mut p = [0u8;76];
                loop {
                    let l = o.read(&mut p)?;
                    if l != 76 {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("piece entry read is {} instead of 76", l)));
                    }

                    let elen = p[0] as u16 | ((p[1] as u16) << 8);
                    if elen != 76 {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("piece entry length is {} instead of 76", elen)));
                    }

                    let kind = p[2] as u16 | ((p[3] as u16) << 8);
                    if kind != 1 {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("piece entry kind is {} instead of 1", kind)))
                    }

                    let soid = Oid::from_bytes(&p[4..(4+64)]);

                    let p = match self.get_blob(&soid)? {
                        Some(v) => v,
                        None => return Err(io::Error::new(io::ErrorKind::InvalidData,
                                            format!("piece {:?} is missing for object {:?}", soid, oid))),
                    };

                    data.extend(p);
                }
            },
            Kind::Piece => {
                // direct data
                o.read_to_end(&mut data)?;
            },
            Kind::Tree => {
                return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{:?} has Kind::Tree, not allowed", oid)));
            }
        }

        Ok(Some(data))
    }

    pub fn objects<'a>(&'a self) -> ObjectIter<'a>
    {
        ObjectIter::new(self)
    }
}

pub struct ObjectBuilder<'a> {
    parent: &'a Store,
    kind: Kind,

    // FIXME: tempdir should really be something that will remove itself.
    // Or we could just use tempfiles in a fixed-name directory.
    // FIXME: right now we allow tempdir to continue to exist. Need cleanup mechanism.
    tempdir: Dir,

    // FIXME: right now we leak the on-disk File when no commit occurs.
    file: ::std::fs::File,

    // FIXME: send data directly to file, hash progressively. Or hash speculatively if WriteAt &
    // Seek are needed.
    data: Vec<u8>,
}

impl<'a> ObjectBuilder<'a> {
    fn new(parent: &'a Store, kind: Kind) -> io::Result<Self>
    {
        // TODO: encapsulate logic around tempdir, tempfiles, and renaming to allow us to be cross
        // platform.
        let t = parent.base.tempdir("vblock-temp.")?;
        let f = t.create_file("new-object", 0o666)?;

        let mut x = ObjectBuilder {
            parent: parent,
            kind: kind,
            tempdir: t,
            file: f,
            data: Vec::with_capacity(Kind::len()),
        };
        x.data.extend(x.kind.as_bytes().iter());
        Ok(x)
    }

    fn commit(mut self) -> io::Result<Oid> {
        let oid = Oid::from_data(&self.data);
        self.file.write_all(&self.data[..])?;
        let d = self.parent.object_dir(&oid)?;
        let name = self.parent.object_name(&oid);
        ::openat::rename(&self.tempdir, "new-object", &d, &name)?;
        Ok(oid)
    }
}

impl<'a> std::io::Write for ObjectBuilder<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>
    {
        self.data.write(buf)
    }

    fn flush(&mut self) -> io::Result<()>
    {
        self.data.flush()
    }
}
    
/// An object that exists in the `Store`
pub struct Object<'a> {
    parent: &'a Store,
    oid: Oid,
    kind: Kind,
    
    // Consider if we even need this. May be better just to use cached value, which we read on
    // creation anyhow to check hash.
    file: Cursor<Vec<u8>>,
}

impl<'a> Object<'a> {
    fn from_oid(parent: &'a Store, oid: Oid) -> io::Result<Option<Self>> {
        let d = parent.object_dir(&oid)?;
        let mut f = match d.open_file(&parent.object_name(&oid)) {
            Err(e) => {
                return match e.kind() {
                    io::ErrorKind::NotFound => Ok(None),
                    _ => Err(e)
                }
            },
            Ok(v) => v,
        };

        let mut b = vec![];
        f.read_to_end(&mut b)?;
        f.seek(io::SeekFrom::Start(Kind::len() as u64))?;

        let calc_key = Oid::from_data(&b);
        if calc_key != oid {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("piece {:?} is corrupt, has calculated oid {:?}",
                                                                            oid, calc_key)));
        }

        let kind = Kind::from_bytes(&b)?;
        let mut c = Cursor::new(b);
        c.set_position(8);

        Ok(Some(Object {
            parent: parent,
            oid: oid,
            kind: kind,
            file: c,
        }))
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn oid(&self) -> &Oid {
        &self.oid
    }
}

impl<'a> std::io::Read for Object<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>
    {
        self.file.read(buf)
    }
}

pub struct ObjectIter<'a> {
    parent: &'a Store,
    iters: Vec<DirIter>,
    dirs: Vec<Dir>,
}

impl<'a> ObjectIter<'a> {
    fn new(parent: &'a Store) -> Self
    {
        ObjectIter {
            parent: parent,
            dirs: vec![],
            iters: vec![],
        }
    }

    /*
    fn next_inner(&mut self) -> io::Result<Option<Object<'a>>> {
        let cd = if iters.empty() {
            self.iters.push(self.parent.list_dir()?);
            self.parent.dir()
        } else {
            self.dirs.last().unwrap();
        }

        loop {
            let iter = self.iters.last().unwrap();
            let n = self.iters.len() - 1;
            match iter.next() {
                Some(Ok(v)) => {
                    match v.simple_type() {
                        Some(SimpleType::Dir) => {
                            /* Go deeper */
                            let nd = cd.sub_dir(v.file_name())?;
                            let ndi = nd.list_dir()?;
                            self.iters.push(ndi);
                            self.dirs.push(nd);
                            continue;
                        },

                        Some(SimpleType::File) => {
                            // Could be an object

                        }

                        Some(_),None => {
                            // TODO: probe further, but for now assume we don't care and should look at
                            // the next entry.
                        }
                    }
                },
                Some(Err(e)) => {

                },
                None => {

                }
            }
        }
    }
    */
}

impl<'a> Iterator for ObjectIter<'a> {
    type Item = io::Result<Object<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        unimplemented!();
    }
}
