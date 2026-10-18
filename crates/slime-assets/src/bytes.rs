//! The binary layout of the cooked formats that the engine defines (the manifest, meshes,
//! materials and scenes): four bytes that name the format, its version, then the values in order,
//! every number little-endian, a text as its length and its UTF-8 bytes, a list as its length and
//! its elements.

use crate::AssetId;
use crate::error::CookedError;

/// Writes a cooked file's values in order.
pub(crate) struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    /// A file of the format named `magic`, in `version`.
    pub(crate) fn new(magic: &[u8; 4], version: u32) -> Self {
        let mut writer = Self {
            bytes: magic.to_vec(),
        };
        writer.u32(version);
        writer
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn f32s(&mut self, values: &[f32]) {
        for &value in values {
            self.f32(value);
        }
    }

    pub(crate) fn id(&mut self, id: AssetId) {
        self.bytes.extend_from_slice(&id.as_u128().to_le_bytes());
    }

    /// A length, as 4 bytes.
    pub(crate) fn count(&mut self, count: usize) {
        self.u32(u32::try_from(count).expect("fewer than 4 billion elements"));
    }

    pub(crate) fn text(&mut self, text: &str) {
        self.count(text.len());
        self.bytes.extend_from_slice(text.as_bytes());
    }

    /// Bytes, after their length.
    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        self.count(bytes.len());
        self.bytes.extend_from_slice(bytes);
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// Reads a cooked file's values in order, refusing a file cut short.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    /// What the file is read as, for errors: `manifest`, `mesh`.
    what: &'static str,
}

impl<'a> Reader<'a> {
    /// Opens `bytes`, a file of the format named `magic`, which must be in `version`.
    pub(crate) fn open(
        bytes: &'a [u8],
        magic: &[u8; 4],
        version: u32,
        what: &'static str,
    ) -> Result<Self, CookedError> {
        let mut reader = Self { bytes, what };
        if reader.take(4).ok() != Some(magic.as_slice()) {
            return Err(reader.malformed(&format!(
                "it does not start with `{}`",
                String::from_utf8_lossy(magic)
            )));
        }
        let found = reader.u32()?;
        if found != version {
            return Err(CookedError::Version {
                what,
                found,
                expected: version,
            });
        }
        Ok(reader)
    }

    pub(crate) fn malformed(&self, reason: &str) -> CookedError {
        CookedError::Malformed {
            what: self.what,
            reason: reason.to_owned(),
        }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], CookedError> {
        if self.bytes.len() < count {
            return Err(self.malformed("it is cut short"));
        }
        let (taken, rest) = self.bytes.split_at(count);
        self.bytes = rest;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], CookedError> {
        Ok(self.take(N)?.try_into().expect("N bytes"))
    }

    pub(crate) fn u8(&mut self) -> Result<u8, CookedError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u32(&mut self) -> Result<u32, CookedError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, CookedError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(crate) fn f32(&mut self) -> Result<f32, CookedError> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    pub(crate) fn f64(&mut self) -> Result<f64, CookedError> {
        Ok(f64::from_le_bytes(self.array()?))
    }

    pub(crate) fn f32s<const N: usize>(&mut self) -> Result<[f32; N], CookedError> {
        let mut values = [0.0; N];
        for value in &mut values {
            *value = self.f32()?;
        }
        Ok(values)
    }

    pub(crate) fn id(&mut self) -> Result<AssetId, CookedError> {
        Ok(AssetId::from_u128(u128::from_le_bytes(self.array()?)))
    }

    /// A length of elements of at least `size` bytes each, refused when the bytes left cannot hold
    /// them, before anything is allocated for them.
    pub(crate) fn count(&mut self, size: usize) -> Result<usize, CookedError> {
        let count = self.u32()? as usize;
        if count.saturating_mul(size) > self.bytes.len() {
            return Err(self.malformed("it is cut short"));
        }
        Ok(count)
    }

    pub(crate) fn text(&mut self) -> Result<String, CookedError> {
        let length = self.count(1)?;
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| self.malformed("a text is not UTF-8"))
    }

    /// Bytes, after their length.
    pub(crate) fn bytes(&mut self) -> Result<Vec<u8>, CookedError> {
        let length = self.count(1)?;
        Ok(self.take(length)?.to_vec())
    }

    /// `count` values of `N` floats each.
    pub(crate) fn f32_arrays<const N: usize>(
        &mut self,
        count: usize,
    ) -> Result<Vec<[f32; N]>, CookedError> {
        (0..count).map(|_| self.f32s()).collect()
    }

    /// Refuses bytes after the last value.
    pub(crate) fn finish(self) -> Result<(), CookedError> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(self.malformed("bytes follow its last value"))
        }
    }
}
