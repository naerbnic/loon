
use std::{alloc::Layout, mem::MaybeUninit, sync::atomic::AtomicUsize};

struct StringHeader {
    ref_count: AtomicUsize,
    len: usize,
}

pub struct ImmBytes(RawData);

impl ImmBytes {
    pub fn from_bytes<I>(iter: I) -> Self
    where
        I: ExactSizeIterator<Item = u8>,
    {
        let raw = RawData::from_bytes(iter);
        raw.header()
            .ref_count
            .store(1, std::sync::atomic::Ordering::Release);
        Self(raw)
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.data()
    }
}

impl std::fmt::Debug for ImmBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_bytes(), f)
    }
}

impl std::ops::Deref for ImmBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl Clone for ImmBytes {
    fn clone(&self) -> Self {
        self.0
            .header()
            .ref_count
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Self(self.0)
    }
}

impl Drop for ImmBytes {
    fn drop(&mut self) {
        let header = self.0.header();
        if header
            .ref_count
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel)
            == 1
        {
            // Safety: The ref count is 0, so no other references exist.
            unsafe { self.0.destroy() }
        }
    }
}

impl PartialEq for ImmBytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for ImmBytes {}

impl PartialOrd for ImmBytes {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ImmBytes {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }
}

impl std::hash::Hash for ImmBytes {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state);
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImmString(ImmBytes);

impl ImmString {
    pub fn from_str(s: &str) -> Self {
        // Safety: The type of the input validates it as a valid string.
        unsafe { Self::try_from_bytes_unsafe(s.bytes()) }
    }

    pub fn try_from_bytes<I>(iter: I) -> Result<Self, std::str::Utf8Error>
    where
        I: ExactSizeIterator<Item = u8>,
    {
        let bytes = ImmBytes::from_bytes(iter);
        // Validate the data is valid UTF-8.
        std::str::from_utf8(&bytes[..])?;
        Ok(Self(bytes))
    }

    pub unsafe fn try_from_bytes_unsafe<I>(iter: I) -> Self
    where
        I: ExactSizeIterator<Item = u8>,
    {
        let bytes = ImmBytes::from_bytes(iter);
        Self(bytes)
    }

    pub fn as_str(&self) -> &str {
        // Safety: The data was validated during construction.
        unsafe { std::str::from_utf8_unchecked(&self.0[..]) }
    }
}

impl std::fmt::Debug for ImmString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl std::ops::Deref for ImmString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl From<String> for ImmString {
    fn from(s: String) -> Self {
        Self::from_str(&s)
    }
}

impl From<&str> for ImmString {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

impl std::borrow::Borrow<str> for ImmString {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

#[derive(Copy, Clone)]
struct RawData(*const u8);

fn make_data_layout(len: usize) -> (Layout, usize) {
    const HEADER_LAYOUT: Layout = Layout::new::<StringHeader>();
    let data_layout = Layout::array::<u8>(len).expect("Failed to create layout for data.");
    let (layout, offset) = HEADER_LAYOUT
        .extend(data_layout)
        .expect("Failed to extend layout.");

    debug_assert!(offset == HEADER_LAYOUT.size());
    debug_assert!(layout.size() > 0);
    (layout.pad_to_align(), offset)
}

impl RawData {
    pub fn from_bytes<I>(data: I) -> Self
    where
        I: ExactSizeIterator<Item = u8>,
    {
        let (layout, offset) = make_data_layout(data.len());
        // Since the data is at byte alignment, the offset should be the same as
        // the StringHeader size.
        let data_ptr = unsafe { std::alloc::alloc(layout) };
        // The front part of the data is the header. It must be well-defined
        // before we can use the data.
        // Safety: The layout included the header at the start of the data, and the layout's
        // alignment includes it.
        unsafe {
            std::ptr::write(
                #[allow(clippy::cast_ptr_alignment)]
                data_ptr.cast::<StringHeader>(),
                StringHeader {
                    ref_count: AtomicUsize::new(0),
                    len: data.len(),
                },
            );
        };

        // Safety: The array was allocated with the layout and provided offset,
        // and the uninitialized data is overlaid with MaybeUninit.
        let buffer: &mut [MaybeUninit<u8>] = unsafe {
            std::slice::from_raw_parts_mut(
                (data_ptr.cast::<MaybeUninit<u8>>()).add(offset),
                data.len(),
            )
        };

        for (src_byte, dest_byte) in data.zip(buffer.iter_mut()) {
            dest_byte.write(src_byte);
        }
        RawData(data_ptr)
    }

    pub fn header(&self) -> &StringHeader {
        // Safety: The header is at the start of the allocation.
        #[allow(clippy::cast_ptr_alignment)]
        unsafe {
            &*(self.0.cast::<StringHeader>())
        }
    }

    pub fn data(&self) -> &[u8] {
        let header = self.header();
        // Safety: Layout validated during construction.
        let data_ptr = unsafe { self.0.add(std::mem::size_of::<StringHeader>()) };
        // Safety: The header is at the start of the allocation, and the data
        // is directly after it.
        unsafe { std::slice::from_raw_parts(data_ptr, header.len) }
    }

    pub unsafe fn destroy(&self) {
        let len = self.header().len;
        unsafe { std::alloc::dealloc(self.0.cast_mut(), make_data_layout(len).0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_string() {
        let string = "This is a test!";
        let imm_str = ImmString::from_str(string);
        assert_eq!(&*imm_str, string);
    }

    #[test]
    fn clone_does_not_fail() {
        // This relies of Miri to catch leaks or double-frees
        let string = "This is a test!";
        let imm_str = ImmString::from_str(string);
        let imm_str_clone = imm_str.clone();
        assert_eq!(&*imm_str, string);
        assert_eq!(&*imm_str_clone, string);
    }
}
