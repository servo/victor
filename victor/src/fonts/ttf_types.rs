use std::fmt::{self, Write};
use std::mem;
use std::slice;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

macro_rules! big_endian_int_wrappers {
    ($( $Name: ident: $Int: ty; )+) => {
        $(
            /// A big-endian integer
            #[derive(Pod)]
            #[repr(C)]
            pub(crate) struct $Name($Int);

            impl $Name {
                /// Return the value in native-endian
                #[inline]
                pub(crate) fn value(&self) -> $Int {
                    <$Int>::from_be(self.0)
                }
            }
        )+
    }
}

big_endian_int_wrappers! {
    u32_be: u32;
    u16_be: u16;
    i16_be: i16;
}

pub(crate) type FWord = i16_be;
pub(crate) type UFWord = u16_be;

/// `Fixed` in https://www.microsoft.com/typography/otspec/otff.htm#dataTypes
#[repr(C)]
#[derive(Pod)]
pub(crate) struct FixedPoint {
    integral: u16_be,
    fractional: u16_be,
}

#[repr(C)]
#[derive(Pod)]
pub(crate) struct LongDateTime {
    // These two field represent a single i64.
    // We split it in two because TrueType only requires 4-byte alignment.
    pub upper_bits: u32_be,
    pub lower_bits: u32_be,
}

#[derive(Pod, Copy, Clone, PartialEq, Eq)]
#[repr(C)]
pub(crate) struct Tag([u8; 4]);

impl LongDateTime {
    fn seconds_since_1904_01_01_midnight(&self) -> i64 {
        let upper = (self.upper_bits.value() as u64) << 32;
        let lower = self.lower_bits.value() as u64;
        (upper | lower) as i64
    }

    fn to_system_time(&self) -> SystemTime {
        // `date --utc -d 1904-01-01 +%s`
        let truetype_epoch = UNIX_EPOCH - Duration::from_secs(2_082_844_800);
        let seconds = self.seconds_since_1904_01_01_midnight();
        if seconds >= 0 {
            truetype_epoch + Duration::from_secs(seconds as u64)
        } else {
            truetype_epoch - Duration::from_secs((-seconds) as u64)
        }
    }
}

impl fmt::Debug for LongDateTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.to_system_time().fmt(f)
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char('"')?;
        for &byte in &self.0 {
            if byte == b'"' {
                f.write_str(r#"\""#)?
            } else if b' ' <= byte && byte <= b'~' {
                // ASCII printable or space
                f.write_char(byte as char)?
            } else {
                write!(f, r"\x{:02X}", byte)?
            }
        }
        f.write_char('"')
    }
}

impl<'a> PartialEq<&'a [u8; 4]> for Tag {
    fn eq(&self, other: &&'a [u8; 4]) -> bool {
        self.0 == **other
    }
}

/// Plain old data: all bit patterns represent valid values
pub(crate) unsafe trait Pod: Sized {
    fn cast(bytes: &[u8], offset: usize) -> &Self {
        &Self::cast_slice(bytes, offset, 1)[0]
    }

    fn cast_slice(bytes: &[u8], offset: usize, n_items: usize) -> &[Self] {
        let required_alignment = mem::align_of::<Self>();
        assert!(required_alignment <= 4,
                "This type requires more alignment than TrueType promises");

        let bytes = &bytes[offset..];
        assert!((bytes.as_ptr() as usize) % required_alignment == 0);

        let required_len = mem::size_of::<Self>().saturating_mul(n_items);
        assert!(bytes.len() >= required_len);

        let ptr = bytes.as_ptr() as *const Self;
        unsafe {
            slice::from_raw_parts(ptr, n_items)
        }
    }
}

unsafe impl Pod for u8 {}
unsafe impl Pod for u16 {}
unsafe impl Pod for i16 {}
unsafe impl Pod for u32 {}
unsafe impl<T: Pod> Pod for [T; 4] {}
