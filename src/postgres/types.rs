//! Column types and cell values, plus their Postgres text/binary encodings.

/// PostgreSQL column types that [`crate::postgres::FakePostgres`] can describe
/// and encode in mocked result sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PgType {
    /// The PostgreSQL `boolean` type.
    Bool,
    /// The PostgreSQL variable-length binary string (`bytea`) type.
    Bytea,
    /// The PostgreSQL two-byte signed integer (`smallint`) type.
    Int2,
    /// The PostgreSQL four-byte signed integer (`integer`) type.
    Int4,
    /// The PostgreSQL eight-byte signed integer (`bigint`) type.
    Int8,
    /// The PostgreSQL four-byte floating-point (`real`) type.
    Float4,
    /// The PostgreSQL eight-byte floating-point (`double precision`) type.
    Float8,
    /// The PostgreSQL variable-length text (`text`) type.
    Text,
    /// The PostgreSQL variable-length character (`varchar`) type.
    Varchar,
    /// The PostgreSQL textual JSON (`json`) type.
    Json,
    /// The PostgreSQL binary JSON (`jsonb`) type.
    Jsonb,
    /// The PostgreSQL universally unique identifier (`uuid`) type.
    Uuid,
}

impl PgType {
    /// Returns the PostgreSQL object identifier (OID) for this type.
    pub fn oid(self) -> u32 {
        use PgType::*;
        match self {
            Bool => 16,
            Bytea => 17,
            Int8 => 20,
            Int2 => 21,
            Int4 => 23,
            Text => 25,
            Json => 114,
            Float4 => 700,
            Float8 => 701,
            Varchar => 1043,
            Uuid => 2950,
            Jsonb => 3802,
        }
    }

    /// Returns the fixed size of this type in bytes, or `-1` for a
    /// variable-length type.
    ///
    /// This value is included in the PostgreSQL `RowDescription` message.
    pub fn size(self) -> i16 {
        use PgType::*;
        match self {
            Bool => 1,
            Int2 => 2,
            Int4 | Float4 => 4,
            Int8 | Float8 => 8,
            Uuid => 16,
            Bytea | Text | Varchar | Json | Jsonb => -1,
        }
    }
}

/// A column definition in a mocked PostgreSQL result set.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    /// The column name reported to the client.
    pub name: String,
    /// The PostgreSQL type used to describe and encode the column's values.
    pub ty: PgType,
}

/// A single cell value in a mocked PostgreSQL result row.
///
/// Encoding depends on the column's [`PgType`]. Integer values support integer
/// and floating-point columns, text values support text, JSON, and UUID columns,
/// and byte values support `bytea` columns.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A SQL `NULL` value.
    Null,
    /// A boolean value for a [`PgType::Bool`] column.
    Bool(bool),
    /// A signed integer value for an integer or floating-point column.
    Int(i64),
    /// A floating-point value for a [`PgType::Float4`] or [`PgType::Float8`]
    /// column.
    Float(f64),
    /// A UTF-8 string for a text, `varchar`, JSON, JSONB, or UUID column.
    Text(String),
    /// Arbitrary binary data for a [`PgType::Bytea`] column.
    Bytes(Vec<u8>),
}

macro_rules! from_int {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            fn from(v: $t) -> Self { Value::Int(v as i64) }
        }
    )*};
}
from_int!(u8, u16, u32, i16, i32, i64);

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}
impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::Float(v as f64)
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_owned())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}
impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Bytes(v)
    }
}
impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        v.map_or(Value::Null, Into::into)
    }
}

fn fmt_f64(f: f64) -> String {
    if f.is_nan() {
        "NaN".into()
    } else if f.is_infinite() {
        if f > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        f.to_string()
    }
}

fn fmt_f32(f: f32) -> String {
    if f.is_nan() {
        "NaN".into()
    } else if f.is_infinite() {
        if f > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        f.to_string()
    }
}

fn parse_uuid(s: &str) -> Option<[u8; 16]> {
    let digits: Vec<u8> = s
        .chars()
        .filter(|c| *c != '-')
        .map(|c| c.to_digit(16).map(|d| d as u8))
        .collect::<Option<_>>()?;
    if digits.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, pair) in digits.chunks(2).enumerate() {
        out[i] = pair[0] << 4 | pair[1];
    }
    Some(out)
}

/// Encode `v` for a column of type `ty` in text (`binary == false`) or binary format.
/// `Ok(None)` means SQL NULL.
pub(crate) fn encode(ty: PgType, v: &Value, binary: bool) -> Result<Option<Vec<u8>>, String> {
    use PgType::*;
    if matches!(v, Value::Null) {
        return Ok(None);
    }
    let bad = || format!("FakePostgres: cannot encode {:?} as {:?}", v, ty);
    let int = |min: i64, max: i64| -> Result<i64, String> {
        match v {
            Value::Int(i) if (min..=max).contains(i) => Ok(*i),
            _ => Err(bad()),
        }
    };
    let float = || -> Result<f64, String> {
        match v {
            Value::Float(f) => Ok(*f),
            Value::Int(i) => Ok(*i as f64),
            _ => Err(bad()),
        }
    };
    let text = || -> Result<&str, String> {
        match v {
            Value::Text(s) => Ok(s.as_str()),
            _ => Err(bad()),
        }
    };

    let out = match ty {
        Bool => match v {
            Value::Bool(b) if binary => vec![*b as u8],
            Value::Bool(b) => vec![if *b { b't' } else { b'f' }],
            _ => return Err(bad()),
        },
        Int2 => {
            let i = int(i16::MIN as i64, i16::MAX as i64)?;
            if binary {
                (i as i16).to_be_bytes().to_vec()
            } else {
                i.to_string().into_bytes()
            }
        }
        Int4 => {
            let i = int(i32::MIN as i64, i32::MAX as i64)?;
            if binary {
                (i as i32).to_be_bytes().to_vec()
            } else {
                i.to_string().into_bytes()
            }
        }
        Int8 => {
            let i = int(i64::MIN, i64::MAX)?;
            if binary {
                i.to_be_bytes().to_vec()
            } else {
                i.to_string().into_bytes()
            }
        }
        Float4 => {
            let f = float()? as f32;
            if binary {
                f.to_be_bytes().to_vec()
            } else {
                fmt_f32(f).into_bytes()
            }
        }
        Float8 => {
            let f = float()?;
            if binary {
                f.to_be_bytes().to_vec()
            } else {
                fmt_f64(f).into_bytes()
            }
        }
        Text | Varchar | Json => text()?.as_bytes().to_vec(),
        Jsonb => {
            let s = text()?;
            if binary {
                let mut b = vec![1u8];
                b.extend_from_slice(s.as_bytes());
                b
            } else {
                s.as_bytes().to_vec()
            }
        }
        Uuid => {
            let s = text()?;
            let bytes = parse_uuid(s).ok_or_else(bad)?;
            if binary {
                bytes.to_vec()
            } else {
                s.as_bytes().to_vec()
            }
        }
        Bytea => match v {
            Value::Bytes(b) if binary => b.clone(),
            Value::Bytes(b) => {
                let mut s = String::from("\\x");
                for byte in b {
                    s.push_str(&format!("{byte:02x}"));
                }
                s.into_bytes()
            }
            _ => return Err(bad()),
        },
    };
    Ok(Some(out))
}
