use std::{fmt, ops, str::FromStr};

use serde::{de, ser};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Memory {
    bytes: u64,
}

impl Memory {
    pub const BYTE: Self = Self::from_bytes(1);
    pub const KIBIBYTE: Self = Self::from_kibibytes(1);
    pub const MEBIBYTE: Self = Self::from_mebibytes(1);
    pub const GIBIBYTE: Self = Self::from_gibibytes(1);

    pub const fn from_bytes(bytes: u64) -> Self {
        Self { bytes }
    }

    pub const fn from_kibibytes(kibibytes: u64) -> Self {
        Self {
            bytes: kibibytes * 1024_u64.pow(1),
        }
    }

    pub const fn from_kibibytes_f64(kibibytes: f64) -> Self {
        Self {
            bytes: (kibibytes * 1024_u64.pow(1) as f64) as u64,
        }
    }

    pub const fn from_mebibytes(mebibytes: u64) -> Self {
        Self {
            bytes: mebibytes * 1024_u64.pow(2),
        }
    }

    pub fn from_mebibytes_f64(mebibytes: f64) -> Self {
        Self {
            bytes: (mebibytes * 1024_u64.pow(2) as f64) as u64,
        }
    }

    pub const fn from_gibibytes(gibibytes: u64) -> Self {
        Self {
            bytes: gibibytes * 1024_u64.pow(3),
        }
    }

    pub const fn from_gibibytes_f64(gibibytes: f64) -> Self {
        Self {
            bytes: (gibibytes as f64 * 1024_u64.pow(3) as f64) as u64,
        }
    }

    pub const fn bytes(self) -> u64 {
        self.bytes
    }

    pub const fn kibibytes(self) -> u64 {
        self.bytes / 1024_u64.pow(1)
    }

    pub fn kibibytes_f64(self) -> f64 {
        self.bytes as f64 / 1024_u64.pow(1) as f64
    }

    pub const fn mebibytes(self) -> u64 {
        self.bytes / 1024_u64.pow(2)
    }

    pub fn mebibytes_f64(self) -> f64 {
        self.bytes as f64 / 1024_u64.pow(2) as f64
    }

    pub const fn gibibytes(self) -> u64 {
        self.bytes / 1024_u64.pow(3)
    }

    pub fn gibibytes_f64(self) -> f64 {
        self.bytes as f64 / 1024_u64.pow(3) as f64
    }
}

impl ops::Add<Memory> for Memory {
    type Output = Memory;

    fn add(self, rhs: Self) -> Self::Output {
        Memory {
            bytes: self.bytes + rhs.bytes,
        }
    }
}

impl ops::Add<&Memory> for Memory {
    type Output = Memory;

    fn add(self, rhs: &Self) -> Self::Output {
        Memory {
            bytes: self.bytes + rhs.bytes,
        }
    }
}

impl ops::Add<Memory> for &Memory {
    type Output = Memory;

    fn add(self, rhs: Memory) -> Self::Output {
        Memory {
            bytes: self.bytes + rhs.bytes,
        }
    }
}

impl ops::Add<&Memory> for &Memory {
    type Output = Memory;

    fn add(self, rhs: &Memory) -> Self::Output {
        Memory {
            bytes: self.bytes + rhs.bytes,
        }
    }
}

impl ops::Mul<u64> for Memory {
    type Output = Memory;

    fn mul(self, rhs: u64) -> Self::Output {
        Memory {
            bytes: self.bytes * rhs,
        }
    }
}

impl ops::Mul<u64> for &Memory {
    type Output = Memory;

    fn mul(self, rhs: u64) -> Self::Output {
        Memory {
            bytes: self.bytes * rhs,
        }
    }
}

impl ops::Mul<&u64> for Memory {
    type Output = Memory;

    fn mul(self, rhs: &u64) -> Self::Output {
        Memory {
            bytes: self.bytes * rhs,
        }
    }
}

impl ops::Mul<&u64> for &Memory {
    type Output = Memory;

    fn mul(self, rhs: &u64) -> Self::Output {
        Memory {
            bytes: self.bytes * rhs,
        }
    }
}

fn fmt_bytes(m: &Memory, f: &mut fmt::Formatter) -> fmt::Result {
    let bytes = m.bytes();
    if let Some(p) = f.precision() {
        let num = format!("{bytes:.p$}B");
        f.pad_integral(true, "", &num)
    } else {
        let num = format!("{bytes:}B");
        f.pad_integral(true, "", &num)
    }
}

impl fmt::Debug for Memory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (num, unit) = if *self >= Self::GIBIBYTE {
            (self.gibibytes_f64(), "GiB")
        } else if *self >= Self::MEBIBYTE {
            (self.mebibytes_f64(), "MiB")
        } else if *self >= Self::KIBIBYTE {
            (self.kibibytes_f64(), "KiB")
        } else {
            return fmt_bytes(self, f);
        };

        let num = if let Some(p) = f.precision() {
            format!("{num:.p$}{unit}")
        } else {
            format!("{num}{unit}")
        };
        f.pad_integral(true, "", &num)
    }
}

impl fmt::Display for Memory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            fmt::Debug::fmt(self, f)
        } else {
            fmt_bytes(self, f)
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseMemoryError {
    Empty,
    InvalidUnit,
    InvalidLiteral,
}

impl fmt::Display for ParseMemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseMemoryError::Empty => "cannot parse memory from empty string",
            ParseMemoryError::InvalidUnit => "invalid memory unit (use KiB, MiB, etc)",
            ParseMemoryError::InvalidLiteral => "invalid literal value",
        }
        .fmt(f)
    }
}

impl std::error::Error for ParseMemoryError {}

impl FromStr for Memory {
    type Err = ParseMemoryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn fperr(_: std::num::ParseFloatError) -> ParseMemoryError {
            ParseMemoryError::InvalidLiteral
        }

        if s.is_empty() {
            Err(ParseMemoryError::Empty)
        } else if let Some(s) = s.strip_suffix("GiB") {
            Ok(Memory::from_gibibytes_f64(s.parse().map_err(fperr)?))
        } else if let Some(s) = s.strip_suffix("MiB") {
            Ok(Memory::from_mebibytes_f64(s.parse().map_err(fperr)?))
        } else if let Some(s) = s.strip_suffix("KiB") {
            Ok(Memory::from_kibibytes_f64(s.parse().map_err(fperr)?))
        } else if let Some(s) = s.strip_suffix("B") {
            match s.parse::<u64>() {
                Ok(bytes) => Ok(Memory::from_bytes(bytes)),
                Err(_) if s.ends_with(|c| matches!(c, '0'..'9')) => {
                    Err(ParseMemoryError::InvalidLiteral)
                }
                Err(_) => Err(ParseMemoryError::InvalidUnit),
            }
        } else {
            Err(ParseMemoryError::InvalidUnit)
        }
    }
}

impl<'de> de::Deserialize<'de> for Memory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Vtor;

        impl<'de> de::Visitor<'de> for Vtor {
            type Value = Memory;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    formatter,
                    "memory amount value with unit of B, KiB, MiB, or GiB"
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                v.parse().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(Vtor)
    }
}

impl ser::Serialize for Memory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn roundtrip(m: Memory, alt: bool, precision: usize) {
        let s = if alt {
            format!("{m:#.precision$}")
        } else {
            format!("{m:.precision$}")
        };

        let parsed: Memory = match s.parse() {
            Ok(x) => x,
            Err(e) => panic!("failed to parse {s:?} - {e}"),
        };

        assert_eq!(m, parsed, "{s:?} ({m:#}) re-parsed incorrectly")
    }

    #[test]
    fn format_basic() {
        assert_eq!(format!("{:#}", Memory::BYTE), "1B");
        assert_eq!(format!("{:#}", Memory::KIBIBYTE), "1KiB");
        assert_eq!(format!("{:#}", Memory::MEBIBYTE), "1MiB");
        assert_eq!(format!("{:#}", Memory::GIBIBYTE), "1GiB");
        assert_eq!(format!("{:#}", Memory::MEBIBYTE * (1024 + 512)), "1.5GiB");
        assert_eq!(format!("{:}", Memory::BYTE * 123934815901), "123934815901B");
        assert_eq!(
            format!("{:}", Memory::BYTE * 12393481590112381239),
            "12393481590112381239B"
        );

        assert_eq!(format!("{:#.1}", Memory::BYTE * 30_000_000), "28.6MiB");
    }

    #[test]
    fn constants_round_trip() {
        roundtrip(Memory::BYTE, true, 1);
        roundtrip(Memory::BYTE, false, 4);

        roundtrip(Memory::KIBIBYTE, true, 1);
        roundtrip(Memory::KIBIBYTE, false, 4);

        roundtrip(Memory::MEBIBYTE, true, 1);
        roundtrip(Memory::MEBIBYTE, false, 4);

        roundtrip(Memory::GIBIBYTE, true, 1);
        roundtrip(Memory::GIBIBYTE, false, 4);
    }
}
