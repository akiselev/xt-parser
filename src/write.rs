//! XT text serialisation: the inverse of [`TextCursor`](crate::TextCursor).
//!
//! A [`TextSink`] accumulates the logical stream (numbers end in one space, `?`
//! is an unset value and takes no space, character and logical fields run
//! together without separators) and [`TextSink::finish`] breaks it into
//! physical lines that never end in a space, because the text reader drops
//! trailing blanks at every line end.

use std::fmt;

use crate::schema::FieldType;
use crate::{FieldValue, NodeRecord, ScalarValue};

/// A value the text form cannot carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkError {
    /// A real that is NaN or infinite.
    NonFinite { context: String },
    /// A field whose values do not match its declared type.
    FieldTypeMismatch { field: String },
}

impl fmt::Display for SinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite { context } => write!(f, "non-finite real in {context}"),
            Self::FieldTypeMismatch { field } => {
                write!(f, "field {field} holds values of another type")
            }
        }
    }
}

impl std::error::Error for SinkError {}

/// Text sink for the logical XT stream.
#[derive(Debug, Clone, Default)]
pub struct TextSink {
    logical: Vec<u8>,
}

impl TextSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn logical_bytes(&self) -> &[u8] {
        &self.logical
    }

    fn token(&mut self, text: &str) {
        self.logical.extend_from_slice(text.as_bytes());
        self.logical.push(b' ');
    }

    fn unset(&mut self) {
        self.logical.push(b'?');
    }

    pub fn write_u8(&mut self, value: u8) {
        self.token(&value.to_string());
    }

    pub fn write_i16(&mut self, value: Option<i16>) {
        match value {
            Some(value) => self.token(&value.to_string()),
            None => self.unset(),
        }
    }

    /// A 16-bit code unit is written as a signed short.
    pub fn write_u16(&mut self, value: Option<u16>) {
        self.write_i16(value.map(|value| value as i16));
    }

    pub fn write_i32(&mut self, value: Option<i32>) {
        match value {
            Some(value) => self.token(&value.to_string()),
            None => self.unset(),
        }
    }

    pub fn write_index(&mut self, value: Option<u32>) {
        match value {
            Some(value) => self.token(&value.to_string()),
            None => self.unset(),
        }
    }

    /// The final terminator index: no trailing space.
    pub fn write_final_index(&mut self, value: u32) {
        self.logical.extend_from_slice(value.to_string().as_bytes());
    }

    pub fn write_f64(&mut self, value: Option<f64>, context: &str) -> Result<(), SinkError> {
        match value {
            Some(value) => {
                if !value.is_finite() {
                    return Err(SinkError::NonFinite {
                        context: context.to_owned(),
                    });
                }
                self.token(&format_f64(value));
            }
            None => self.unset(),
        }
        Ok(())
    }

    pub fn write_logical(&mut self, value: bool) {
        self.logical.push(if value { b'T' } else { b'F' });
    }

    /// Characters with the text escapes (`\0`, CR as `\n`, LF as `\r`, `\\`).
    pub fn write_chars(&mut self, bytes: &[u8]) {
        for byte in bytes {
            match byte {
                0 => self.logical.extend_from_slice(b"\\0"),
                b'\r' => self.logical.extend_from_slice(b"\\n"),
                b'\n' => self.logical.extend_from_slice(b"\\r"),
                b'\\' => self.logical.extend_from_slice(b"\\\\"),
                other => self.logical.push(*other),
            }
        }
    }

    /// Length-prefixed string, as in the text prefix.
    pub fn write_short_string(&mut self, bytes: &[u8]) {
        self.token(&bytes.len().to_string());
        self.write_chars(bytes);
    }

    pub fn write_vector(
        &mut self,
        value: Option<[f64; 3]>,
        context: &str,
    ) -> Result<(), SinkError> {
        self.write_array(value.as_ref().map(|v| &v[..]), context)
    }

    pub fn write_interval(
        &mut self,
        value: Option<[f64; 2]>,
        context: &str,
    ) -> Result<(), SinkError> {
        self.write_array(value.as_ref().map(|v| &v[..]), context)
    }

    pub fn write_box(&mut self, value: Option<[f64; 6]>, context: &str) -> Result<(), SinkError> {
        self.write_array(value.as_ref().map(|v| &v[..]), context)
    }

    fn write_array(&mut self, value: Option<&[f64]>, context: &str) -> Result<(), SinkError> {
        match value {
            None => self.unset(),
            Some(values) => {
                for value in values {
                    self.write_f64(Some(*value), context)?;
                }
            }
        }
        Ok(())
    }

    pub fn write_scalar(&mut self, value: &ScalarValue, context: &str) -> Result<(), SinkError> {
        match value {
            ScalarValue::UnsignedByte(v) => self.write_u8(*v),
            ScalarValue::Character(v) => self.write_chars(&[*v]),
            ScalarValue::Logical(v) => self.write_logical(*v),
            ScalarValue::ShortInteger(v) => self.write_i16(*v),
            ScalarValue::UnicodeCharacter(v) => self.write_u16(*v),
            ScalarValue::Integer(v) => self.write_i32(*v),
            ScalarValue::PointerIndex(v) => self.write_index(*v),
            ScalarValue::Double(v) => self.write_f64(*v, context)?,
            ScalarValue::Interval(v) => self.write_interval(*v, context)?,
            ScalarValue::Vector(v) => self.write_vector(*v, context)?,
            ScalarValue::Box(v) => self.write_box(*v, context)?,
            ScalarValue::IntersectionPoint(v) => self.write_vector(*v, context)?,
        }
        Ok(())
    }

    pub fn write_field(&mut self, field: &FieldValue) -> Result<(), SinkError> {
        let is_character = field.field_type == FieldType::Character;
        for value in &field.values {
            if is_character != matches!(value, ScalarValue::Character(_)) {
                return Err(SinkError::FieldTypeMismatch {
                    field: field.name.clone(),
                });
            }
            self.write_scalar(value, &field.name)?;
        }
        Ok(())
    }

    /// One node: type, variable-length count if any, file index, fields.
    pub fn write_record(&mut self, record: &NodeRecord) -> Result<(), SinkError> {
        self.token(&record.node_type.to_string());
        if let Some(count) = record.variable_length {
            self.token(&count.to_string());
        }
        self.token(&record.file_index.to_string());
        for field in &record.fields {
            self.write_field(field)?;
        }
        Ok(())
    }

    /// The terminator node (`1 0`).
    pub fn write_terminator(&mut self) {
        self.token("1");
        self.write_final_index(0);
    }

    /// Break the stream into lines of about `width` columns. A break goes
    /// before a space that follows a non-space, so no line ends in a space and
    /// removing the breaks restores the logical stream exactly.
    pub fn finish(self, width: usize) -> String {
        let bytes = self.logical;
        let width = width.max(2);
        let mut out = Vec::with_capacity(bytes.len() + bytes.len() / width + 1);
        let mut start = 0;
        while bytes.len() - start > width {
            let breakable = |p: usize| bytes[p] == b' ' && bytes[p - 1] != b' ';
            let split = (start + 1..=start + width)
                .rev()
                .find(|p| breakable(*p))
                .or_else(|| (start + width + 1..bytes.len()).find(|p| breakable(*p)));
            match split {
                Some(p) => {
                    out.extend_from_slice(&bytes[start..p]);
                    out.push(b'\n');
                    start = p;
                }
                None => break,
            }
        }
        out.extend_from_slice(&bytes[start..]);
        out.push(b'\n');
        // The stream only ever holds ASCII or bytes copied from character fields.
        String::from_utf8_lossy(&out).into_owned()
    }
}

/// Shortest decimal text that parses back to the same `f64`.
pub fn format_f64(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    let magnitude = value.abs();
    if !(1e-5..1e16).contains(&magnitude) {
        format!("{value:e}")
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextStream;

    #[test]
    fn numbers_end_in_one_space_and_unset_takes_none() {
        let mut sink = TextSink::new();
        sink.write_i32(None);
        sink.write_i32(Some(10));
        sink.write_chars(b"+");
        sink.write_index(Some(0));
        sink.write_logical(true);
        assert_eq!(sink.logical_bytes(), b"?10 +0 T");
    }

    #[test]
    fn written_text_reads_back_through_the_cursor() {
        let mut sink = TextSink::new();
        sink.write_f64(Some(1e-8), "x").unwrap();
        sink.write_f64(Some(0.1), "x").unwrap();
        sink.write_f64(Some(-2.5e20), "x").unwrap();
        sink.write_u16(Some(0xFF11));
        sink.write_short_string(b"a b\\\r\n");
        sink.write_vector(Some([1.0, 2.0, 3.0]), "x").unwrap();
        sink.write_vector(None, "x").unwrap();
        let text = sink.finish(5);
        assert!(text.lines().all(|line| !line.ends_with(' ')));
        let stream = TextStream::from_raw(text.as_bytes(), 0);
        let mut cursor = stream.cursor();
        assert_eq!(cursor.read_f64().unwrap(), Some(1e-8));
        assert_eq!(cursor.read_f64().unwrap(), Some(0.1));
        assert_eq!(cursor.read_f64().unwrap(), Some(-2.5e20));
        assert_eq!(cursor.read_u16().unwrap(), Some(0xFF11));
        assert_eq!(cursor.read_short_string().unwrap(), b"a b\\\r\n");
        assert_eq!(cursor.read_vector().unwrap(), Some([1.0, 2.0, 3.0]));
        assert_eq!(cursor.read_vector().unwrap(), None);
        assert_eq!(cursor.remaining(), 0);
    }

    #[test]
    fn non_finite_reals_are_refused() {
        let mut sink = TextSink::new();
        assert!(sink.write_f64(Some(f64::NAN), "x").is_err());
    }
}
