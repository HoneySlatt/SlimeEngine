//! Documents in the engine's own binary encoding: the document that a text holds, without its types,
//! read back without parsing text. [`format`](crate::format) says how its bytes are laid out
//! (`docs/adr/0094-the-binary-encoding-of-documents.md`).

use std::collections::{HashMap, HashSet};

use slime_reflect::{Path, Step, Value};

use crate::error::ReadProblem;
use crate::format::BINARY_VERSION as VERSION;
use crate::{MAX_DEPTH, MAX_SIZE};

/// The bytes that begin every document in the binary encoding: "Slime document, binary".
pub(crate) const MARK: &[u8; 4] = b"SLDB";

const FALSE: u8 = 0;
const TRUE: u8 = 1;
const INTEGER: u8 = 2;
const F32: u8 = 3;
const F64: u8 = 4;
const TEXT: u8 = 5;
const ARRAY: u8 = 6;
const TABLE: u8 = 7;

/// Why bytes are not a document, where: the byte, the path of the part at fault, the problem.
pub(crate) type Failure = (usize, Path, ReadProblem);

/// The document whose root table holds `fields` in the binary encoding. Its values are those a
/// document holds: booleans, integers, floats, texts, arrays and tables.
pub(crate) fn encode(fields: &[(String, Value)]) -> Vec<u8> {
    let mut strings = Strings::default();
    strings.table(fields);
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(MARK);
    out.push(VERSION);
    varint(&mut out, strings.order.len() as u64);
    for string in &strings.order {
        varint(&mut out, string.len() as u64);
        out.extend_from_slice(string.as_bytes());
    }
    table(&mut out, fields, &strings);
    out
}

/// Each string of a document, once, in the order in which a walk first meets it.
#[derive(Default)]
struct Strings<'v> {
    index: HashMap<&'v str, u64>,
    order: Vec<&'v str>,
}

impl<'v> Strings<'v> {
    fn add(&mut self, string: &'v str) {
        if !self.index.contains_key(string) {
            self.index.insert(string, self.order.len() as u64);
            self.order.push(string);
        }
    }

    fn table(&mut self, fields: &'v [(String, Value)]) {
        for (key, value) in fields {
            self.add(key);
            self.value(value);
        }
    }

    fn value(&mut self, value: &'v Value) {
        match value {
            Value::Text(text) => self.add(text),
            Value::List(elements) => elements.iter().for_each(|element| self.value(element)),
            Value::Struct(fields) => self.table(fields),
            _ => {}
        }
    }

    fn of(&self, string: &str) -> u64 {
        self.index[string]
    }
}

fn table(out: &mut Vec<u8>, fields: &[(String, Value)], strings: &Strings<'_>) {
    out.push(TABLE);
    varint(out, fields.len() as u64);
    for (key, value) in fields {
        varint(out, strings.of(key));
        self::value(out, value, strings);
    }
}

fn value(out: &mut Vec<u8>, value: &Value, strings: &Strings<'_>) {
    match value {
        Value::Bool(false) => out.push(FALSE),
        Value::Bool(true) => out.push(TRUE),
        Value::Int(number) => {
            out.push(INTEGER);
            varint(out, ((number << 1) ^ (number >> 63)) as u64);
        }
        Value::F32(number) => float(out, f64::from(*number)),
        Value::F64(number) => float(out, *number),
        Value::Text(text) => {
            out.push(TEXT);
            varint(out, strings.of(text));
        }
        Value::List(elements) => {
            out.push(ARRAY);
            varint(out, elements.len() as u64);
            for element in elements {
                self::value(out, element, strings);
            }
        }
        Value::Struct(fields) => table(out, fields, strings),
        other => unreachable!("a document holds no {}", other.kind()),
    }
}

/// A float: an `f32`'s four bytes when it is one exactly, NaN as the `f32`'s with its sign, as a
/// text writes it, `nan` or `-nan`; an `f64`'s eight otherwise.
fn float(out: &mut Vec<u8>, number: f64) {
    let narrow = if number.is_nan() {
        Some(if number.is_sign_negative() {
            -f32::NAN
        } else {
            f32::NAN
        })
    } else {
        Some(number as f32).filter(|&narrow| f64::from(narrow).to_bits() == number.to_bits())
    };
    match narrow {
        Some(narrow) => {
            out.push(F32);
            out.extend_from_slice(&narrow.to_le_bytes());
        }
        None => {
            out.push(F64);
            out.extend_from_slice(&number.to_le_bytes());
        }
    }
}

fn varint(out: &mut Vec<u8>, mut number: u64) {
    while number >= 0x80 {
        out.push((number as u8 & 0x7f) | 0x80);
        number >>= 7;
    }
    out.push(number as u8);
}

/// The root table of the document that `bytes` hold, as a text's parsing gives it: floats as
/// [`Value::F64`].
///
/// # Errors
///
/// Bytes that do not begin with [`MARK`], of version 0 or of a later version of the encoding, cut
/// short, with a tag, a varint, a count or a string index that the encoding does not have, a string
/// that is not UTF-8 or that comes twice, a key given twice in a table, values nested deeper than
/// [`MAX_DEPTH`], keys and texts that hold more than [`MAX_SIZE`] bytes counted where they are used,
/// or bytes past the root table: at the byte, with the path of the part at fault.
pub(crate) fn decode(bytes: &[u8]) -> Result<Vec<(String, Value)>, Failure> {
    let mut decoder = Decoder::new(bytes)?;
    let root = decoder.root()?;
    if decoder.at != bytes.len() {
        return Err(decoder.fail("bytes past the root table"));
    }
    Ok(root)
}

/// Where the part at `at`, a path through the values of the document that `bytes` hold, lies in
/// them: the byte of its key if `key` and it has one, of its value otherwise; of the nearest part
/// that holds it when the document has no such part; 0 for the root. `bytes` are a document that
/// [`decode`] read: bytes that it would refuse may give `None`, or any byte.
pub(crate) fn locate(bytes: &[u8], at: &Path, key: bool) -> Option<usize> {
    let mut decoder = Decoder::new(bytes).ok()?;
    let mut found = 0;
    for step in at.steps() {
        let tag = decoder.byte().ok()?;
        let count = match tag {
            TABLE | ARRAY => decoder.count().ok()?,
            _ => return Some(found),
        };
        // The byte of the step's key, if it has one, its value next.
        let key_at = match (tag, step) {
            (TABLE, Step::Field(name)) => {
                let mut found_key = None;
                for _ in 0..count {
                    let key_at = decoder.at;
                    let index = decoder.varint().ok()?;
                    if decoder.strings.get(index as usize) == Some(&name.as_str()) {
                        found_key = Some(key_at);
                        break;
                    }
                    decoder.skip(1).ok()?;
                }
                match found_key {
                    Some(key_at) => Some(key_at),
                    None => return Some(found),
                }
            }
            (ARRAY, &Step::Index(index)) if (index as u64) < count => {
                decoder.skip(index as u64).ok()?;
                None
            }
            _ => return Some(found),
        };
        found = match key_at {
            Some(key_at) if key => key_at,
            _ => decoder.at,
        };
    }
    Some(found)
}

/// What a count of things to read reserves: at most 1024, the rest grown as they are read, so that
/// counts nested within a few bytes, each at most the bytes left, reserve little.
fn capacity(count: u64) -> usize {
    count.min(1024) as usize
}

/// Reads the binary encoding of a document, byte after byte.
struct Decoder<'b> {
    bytes: &'b [u8],
    at: usize,
    strings: Vec<&'b str>,
    /// The parts that lead to the one read: the index of a key's string, or of an array's element.
    path: Vec<Part>,
    /// How many bytes of strings were copied into the document read so far.
    copied: usize,
}

#[derive(Clone, Copy)]
enum Part {
    Key(usize),
    Element(usize),
}

impl<'b> Decoder<'b> {
    /// Checks the mark and the version, and reads the strings.
    fn new(bytes: &'b [u8]) -> Result<Self, Failure> {
        let mut decoder = Self {
            bytes,
            at: 0,
            strings: Vec::new(),
            path: Vec::new(),
            copied: 0,
        };
        if bytes.get(..MARK.len()) != Some(MARK) {
            return Err(decoder.fail("not the engine's mark, `SLDB`"));
        }
        decoder.at = MARK.len();
        let version = decoder.byte()?;
        if version == 0 {
            return Err((
                MARK.len(),
                Path::default(),
                ReadProblem::Binary("version 0, where versions start at 1".to_owned()),
            ));
        }
        if version > VERSION {
            return Err((
                MARK.len(),
                Path::default(),
                ReadProblem::NewerEncoding {
                    found: version,
                    known: VERSION,
                },
            ));
        }
        let count = decoder.count()?;
        decoder.strings.reserve(capacity(count));
        let mut seen = HashSet::new();
        for _ in 0..count {
            let start = decoder.at;
            let length = decoder.count()? as usize;
            let string = std::str::from_utf8(&bytes[decoder.at..decoder.at + length])
                .map_err(|_| decoder.fail("a string that is not UTF-8"))?;
            // Once each, so that a key is given twice in a table only by its index.
            if !seen.insert(string) {
                decoder.at = start;
                return Err(decoder.fail("a string given twice"));
            }
            decoder.at += length;
            decoder.strings.push(string);
        }
        Ok(decoder)
    }

    fn root(&mut self) -> Result<Vec<(String, Value)>, Failure> {
        match self.byte()? {
            TABLE => self.table(1),
            _ => {
                self.at -= 1;
                Err(self.fail("a root that is not a table"))
            }
        }
    }

    /// A table's entries, the table at `depth`, its tag read.
    fn table(&mut self, depth: usize) -> Result<Vec<(String, Value)>, Failure> {
        self.deep(depth)?;
        let count = self.count()?;
        let mut fields: Vec<(String, Value)> = Vec::with_capacity(capacity(count));
        // A few keys looked through, many hashed.
        let mut few: Vec<usize> = Vec::new();
        let mut many: HashSet<usize> = HashSet::new();
        for _ in 0..count {
            let key_at = self.at;
            let key = self.string()?;
            let twice = if count < 16 {
                let twice = few.contains(&key);
                few.push(key);
                twice
            } else {
                !many.insert(key)
            };
            if twice {
                self.at = key_at;
                return Err(self.fail("a key given twice in a table"));
            }
            self.path.push(Part::Key(key));
            let value = self.value(depth + 1)?;
            self.path.pop();
            fields.push((self.copy(key)?, value));
        }
        Ok(fields)
    }

    /// A value, at `depth`.
    fn value(&mut self, depth: usize) -> Result<Value, Failure> {
        self.deep(depth)?;
        let start = self.at;
        Ok(match self.byte()? {
            FALSE => Value::Bool(false),
            TRUE => Value::Bool(true),
            INTEGER => {
                let zigzag = self.varint()?;
                Value::Int(((zigzag >> 1) as i64) ^ -((zigzag & 1) as i64))
            }
            F32 => {
                let narrow = f32::from_le_bytes(self.array()?);
                // NaN with its sign, which a cast does not promise to keep.
                Value::F64(if narrow.is_nan() {
                    f64::NAN.copysign(if narrow.is_sign_negative() { -1.0 } else { 1.0 })
                } else {
                    f64::from(narrow)
                })
            }
            F64 => Value::F64(f64::from_le_bytes(self.array()?)),
            TEXT => {
                let index = self.string()?;
                Value::Text(self.copy(index)?)
            }
            ARRAY => {
                let count = self.count()?;
                let mut elements = Vec::with_capacity(capacity(count));
                for index in 0..count as usize {
                    self.path.push(Part::Element(index));
                    elements.push(self.value(depth + 1)?);
                    self.path.pop();
                }
                Value::List(elements)
            }
            TABLE => Value::Struct(self.table(depth)?),
            _ => {
                self.at = start;
                return Err(self.fail("a tag that the encoding does not have"));
            }
        })
    }

    /// Skips `count` values, which a decoding of the whole document found sound.
    fn skip(&mut self, count: u64) -> Result<(), Failure> {
        for _ in 0..count {
            match self.byte()? {
                FALSE | TRUE => {}
                INTEGER | TEXT => {
                    self.varint()?;
                }
                F32 => {
                    self.array::<4>()?;
                }
                F64 => {
                    self.array::<8>()?;
                }
                ARRAY => {
                    let count = self.count()?;
                    self.skip(count)?;
                }
                TABLE => {
                    for _ in 0..self.count()? {
                        self.varint()?;
                        self.skip(1)?;
                    }
                }
                _ => return Err(self.fail("a tag that the encoding does not have")),
            }
        }
        Ok(())
    }

    /// Refuses a value nested deeper than [`MAX_DEPTH`], as a text's parsing does.
    fn deep(&self, depth: usize) -> Result<(), Failure> {
        if depth > MAX_DEPTH {
            return Err((
                self.at,
                self.path(),
                ReadProblem::TooDeep { limit: MAX_DEPTH },
            ));
        }
        Ok(())
    }

    fn byte(&mut self) -> Result<u8, Failure> {
        let byte = *self
            .bytes
            .get(self.at)
            .ok_or_else(|| self.fail("cut short"))?;
        self.at += 1;
        Ok(byte)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Failure> {
        let bytes = self
            .bytes
            .get(self.at..self.at + N)
            .ok_or_else(|| self.fail("cut short"))?;
        self.at += N;
        Ok(bytes.try_into().expect("N bytes"))
    }

    fn varint(&mut self) -> Result<u64, Failure> {
        let start = self.at;
        let mut number = 0_u64;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            let bits = u64::from(byte & 0x7f);
            if shift == 63 && bits > 1 {
                break;
            }
            number |= bits << shift;
            if byte & 0x80 == 0 {
                return Ok(number);
            }
        }
        self.at = start;
        Err(self.fail("a varint past 64 bits"))
    }

    /// A count of what follows, each of which takes a byte at least: at most the bytes left.
    fn count(&mut self) -> Result<u64, Failure> {
        let start = self.at;
        let count = self.varint()?;
        if count > (self.bytes.len() - self.at) as u64 {
            self.at = start;
            return Err(self.fail("a count past the bytes left"));
        }
        Ok(count)
    }

    /// The string at `index`, copied: each copy counts, so that the keys and texts of a document read
    /// hold at most [`MAX_SIZE`] bytes, as those of a text do, however few bytes use them.
    fn copy(&mut self, index: usize) -> Result<String, Failure> {
        let string = self.strings[index];
        self.copied += string.len();
        if self.copied > MAX_SIZE {
            return Err((
                self.at,
                self.path(),
                ReadProblem::TooLarge {
                    size: self.copied as u64,
                    limit: MAX_SIZE,
                },
            ));
        }
        Ok(string.to_owned())
    }

    /// The index of a string, which the strings hold.
    fn string(&mut self) -> Result<usize, Failure> {
        let start = self.at;
        let index = self.varint()?;
        if index >= self.strings.len() as u64 {
            self.at = start;
            return Err(self.fail("the index of a string that the document does not hold"));
        }
        Ok(index as usize)
    }

    fn path(&self) -> Path {
        self.path
            .iter()
            .fold(Path::default(), |path, part| match *part {
                Part::Key(key) => path.field(self.strings[key].to_owned()),
                Part::Element(index) => path.index(index),
            })
    }

    fn fail(&self, problem: &str) -> Failure {
        (
            self.at,
            self.path(),
            ReadProblem::Binary(problem.to_owned()),
        )
    }
}
