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
use hex::{FromHex,ToHex};

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
/// 
/// TODO: blob multi-level formatting could be:
/// 
///  - balanced tree, with kind prefix in concatenated data of each level after 0th
///  - unbalanced tree, with (kind,oid) pairs in the blob piece entries. This `kind` would control
///    interpretation of data refered to by oid.
pub struct Store {
    base: openat::Dir,
    objects: openat::Dir,
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

    pub fn as_bytes(&self) -> [u8;8] {
        let mut x = [0u8;8];
        byteorder::LittleEndian::write_u64(&mut x[..], self.raw());
        x
    }

    pub fn write_to<W: Write>(&self, mut w: W) -> io::Result<()>
    {
        w.write_all(&self.as_bytes()[..])
    }

    fn read_from<R: Read>(mut r: R) -> io::Result<Self>
    {
        let mut b = [0u8;8];
        r.read_exact(&mut b)?;
        Self::from_bytes(&b)
    }

    fn len() -> usize {
        8
    }
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
    inner: Vec<u8>,
}

impl Oid {
    pub fn from_hex(key: &str) -> Result<Self,hex::FromHexError> {
        Ok(Oid {
            inner: ::hex::FromHex::from_hex(key)?
        })
    }

    pub fn from_bytes<A: Into<Vec<u8>>>(key: A) -> Self
    {
        // TODO: instead of converting & allocating, provide a view in hex?
        Oid {
            inner: key.into()
        }
    }

    fn from_data<A: AsRef<[u8]>>(data: A) -> Self {
        let mut key = [0u8;sodalite::HASH_LEN];
        sodalite::hash(&mut key, data.as_ref());
        Oid::from_bytes(&key[..])
    }

    /// TODO: this is very Index like, see if we can make that usable.
    fn get_part(&self, index: usize) -> OidPart {
        OidPart { inner: CString::new([self.as_ref()[index]].to_hex()).unwrap() }
    }

    /// TODO: this is very Index like, see if we can make that usable.
    fn get_part_rem(&self, index_start: usize) -> OidPart {
        OidPart { inner: CString::new((&self.as_ref()[index_start..]).to_hex()).unwrap() }
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.inner.as_ref()
    }

    fn len_str() -> usize {
        Self::len() * 2
    }

    fn len() -> usize {
        sodalite::HASH_LEN
    }
}

impl AsRef<[u8]> for Oid {
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
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

impl Store {
    pub fn with_dir(d: openat::Dir) -> io::Result<Self> {
        let o = d.create_dir_open("objects")?;

        Ok(Store {
            base: d,
            objects: o,
        })
    }

    pub fn with_path<P: openat::AsPath>(p: P) -> io::Result<Self> {
        let d = ::openat::Dir::open(p)?;
        Self::with_dir(d)
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
    /// XXX:
    ///  - blob formating options: pieces could have markers, or blobs could have a bit in the
    ///    piece entry indicating further deref.
    ///    
    ///  - need a concrete model for how recursive blobs work. Ideally, we'd have a tree-like
    ///    setup, but specifics are needed
    pub fn put_blob<A: AsRef<[u8]>>(&self, data: A) -> io::Result<Oid>
    {
        self.put_blob_inner(Kind::Piece, data)
    }

    ///
    /// The `data` represents an object with `Kind` `kind`.
    ///
    fn put_blob_inner<A: AsRef<[u8]>>(&self, kind: Kind, data: A) -> io::Result<Oid>
    {
        let data = data.as_ref();

        // build an object containing a list of pieces
        let mut pieces = vec![];
        pieces.extend(kind.as_bytes().into_iter());
        let mut have_pieces = false;
        let mut hr = hash_roll::bup::BupBuf::default();

        let mut data = data;

        loop {
            if data.len() == 0 {
                if !have_pieces {
                    // encode this as a piece directly
                    // XXX: only top level objects should be empty, avoid blob-leaves having zero
                    // size.
                    break;
                } else {
                    // no data, emit pieces
                    return self.put_blob_inner(Kind::Blob, pieces)
                }
            }

            let used = hr.push(data);

            let used = if used == 0 {
                // all of `data` is a single piece
                if !have_pieces {
                    break;
                } else {
                    data.len()
                }
            } else if used == data.len() && !have_pieces {
                // all of `data` is a single piece
                break;
            } else {
                // `data` will be split further
                used
            };

            let oid = self.put_object(Kind::Piece, &data[..used])?;
            pieces.extend(oid.as_bytes());
            have_pieces = true;
            data = &{data}[used..];
        }

        self.put_object(kind, data)
    }

    pub fn load_blob<R: Read>(&self, kind: Kind, mut o: R) -> io::Result<Option<Vec<u8>>>
    {
        match kind {
            Kind::Blob => {
                let mut data = vec![];
                // sub-kind is how we should treat the next level of data we load
                let sub_kind = Kind::read_from(&mut o)?;
                match sub_kind {
                    Kind::Blob => {
                    },
                    Kind::Piece => {
                    }
                    Kind::Tree => {
                        // fast-path this error
                        return Err(io::Error::new(io::ErrorKind::InvalidData, "Sub-kind Kind::Tree not allowed"));
                    }
                }

                // resolve other items
                // TODO: use the length field
                loop {
                    let pi = match read_piece_entry(&mut o)? {
                        Some(v) => v,
                        None => break,
                    };

                    // XXX: we should only be getting pieces here based on our blob-splitting
                    // strategy, but it might make sense to allow more flexible loading to
                    // potentially support alternate splitting strategies in the future
                    let p = match self.get(&pi.oid)? {
                        Some(v) => v,
                        None => return Err(io::Error::new(io::ErrorKind::InvalidData,
                                            format!("missing object {:?}", pi.oid))),
                    };
                    if p.kind() != Kind::Piece {
                        return Err(io::Error::new(io::ErrorKind::InvalidData,
                                                  format!("objects {:?} is a {:?}, only Piece allowed",
                                                          pi.oid, p.kind())));
                    }

                    data.extend(p.as_ref());
                }

                // FIXME: handle this incrimentally
                self.load_blob(sub_kind, Cursor::new(data))
            },
            Kind::Piece => {
                // direct data
                let mut data = vec![];
                o.read_to_end(&mut data)?;
                Ok(Some(data))
            },
            Kind::Tree => {
                Err(io::Error::new(io::ErrorKind::InvalidData, "Kind::Tree, not allowed"))
            }
        }
    }

    // TODO: logically, this should probably be handled by Object or similar directly, and allow us
    // to seek & so forth.
    pub fn get_blob(&self, oid: &Oid) -> io::Result<Option<Vec<u8>>>
    {
        let o = match self.get(oid)? {
                Some(v) => v, None => return Ok(None),
        };

        let kind = o.kind();
        // FIXME: map error to include oid
        self.load_blob(kind, o)
    }

    pub fn objects<'a>(&'a self) -> ObjectIter<'a>
    {
        ObjectIter::new(self)
    }
}

struct PieceEntry {
    oid: Oid,
}

fn read_piece_entry<R: Read>(mut r: R) -> io::Result<Option<PieceEntry>>
{
    let mut p = [0u8;64];

    // FIXME: this read() likely needs more checking to catch short reads.
    let l = r.read(&mut p)?;
    if l == 0 {
        return Ok(None);
    }

    let oid = Oid::from_bytes(&p[..64]);

    Ok(Some(PieceEntry {
        oid: oid
    }))
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
    pub fn new(parent: &'a Store, kind: Kind) -> io::Result<Self>
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

    pub fn append<A: AsRef<[u8]>>(mut self, data: A) -> io::Result<Self> {
       self.write_all(data.as_ref())?;
       Ok(self)
    }

    pub fn commit(mut self) -> io::Result<Oid> {
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
    _parent: &'a Store,
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
            _parent: parent,
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

impl<'a> std::convert::AsRef<[u8]> for Object<'a>
{
    fn as_ref(&self) -> &[u8]
    {
        let x: &[u8] = self.file.get_ref().as_ref();
        &x[8..]
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
    dirs: Vec<(Dir,CString)>,
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
        };

        loop {
            let iter = self.iters.last().unwrap();
            let n = self.iters.len() - 1;
            match iter.next() {
                Some(Ok(v)) => {
                    match v.simple_type() {
                        Some(SimpleType::Dir) => {
                            /* Go deeper */
                            let sub_name = v.file_name()?;
                            let nd = cd.sub_dir(sub_name)?;
                            let ndi = nd.list_dir()?;
                            self.iters.push(ndi);
                            self.dirs.push((nd, sub_name));
                            continue;
                        },

                        Some(SimpleType::File) => {
                            // found an object
                            let mut name = Vec::with_capacity(CString::from_vec(

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
