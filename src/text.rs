use serde::Serialize;

use crate::{CommonHeader, ErrorKind, Result, Span, XtError};

/// XT text body after physical record separators and trailing record padding
/// have been removed. `raw_offsets` maps every logical byte back to the input.
#[derive(Debug, Clone)]
pub struct TextStream {
    logical: Vec<u8>,
    raw_offsets: Vec<usize>,
    raw_end: usize,
}

impl TextStream {
    pub fn from_raw(raw: &[u8], base_offset: usize) -> Self {
        let mut logical = Vec::with_capacity(raw.len());
        let mut raw_offsets = Vec::with_capacity(raw.len());
        let mut record_start = 0;

        for (relative, byte) in raw.iter().copied().enumerate() {
            if matches!(byte, b'\r' | b'\n') {
                while logical.len() > record_start && logical.last() == Some(&b' ') {
                    logical.pop();
                    raw_offsets.pop();
                }
                record_start = logical.len();
            } else {
                logical.push(byte);
                raw_offsets.push(base_offset + relative);
            }
        }

        Self {
            logical,
            raw_offsets,
            raw_end: base_offset + raw.len(),
        }
    }

    pub fn logical_bytes(&self) -> &[u8] {
        &self.logical
    }

    pub fn cursor(&self) -> TextCursor<'_> {
        TextCursor {
            stream: self,
            position: 0,
        }
    }

    pub fn cursor_at(&self, position: usize) -> Result<TextCursor<'_>> {
        if position > self.logical.len() {
            return Err(XtError::new(
                ErrorKind::UnexpectedEof,
                self.raw_span(self.logical.len(), self.logical.len()),
                "cursor position is past the logical stream",
            ));
        }
        Ok(TextCursor {
            stream: self,
            position,
        })
    }

    fn raw_span(&self, start: usize, end: usize) -> Span {
        let raw_start = self.raw_offsets.get(start).copied().unwrap_or(self.raw_end);
        let raw_end = if end > start {
            self.raw_offsets
                .get(end - 1)
                .map_or(self.raw_end, |offset| offset + 1)
        } else {
            raw_start
        };
        Span::new(raw_start, raw_end)
    }
}

#[derive(Debug, Clone)]
pub struct TextCursor<'a> {
    stream: &'a TextStream,
    position: usize,
}

impl<'a> TextCursor<'a> {
    pub fn position(&self) -> usize {
        self.position
    }

    pub fn remaining(&self) -> usize {
        self.stream.logical.len().saturating_sub(self.position)
    }

    pub fn span_from(&self, start: usize) -> Span {
        self.stream.raw_span(start, self.position)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        let (span, bytes) = self.required_number("unsigned byte")?;
        parse_number::<u8>(&bytes, span, "unsigned byte")
    }

    pub fn read_i16(&mut self) -> Result<Option<i16>> {
        self.optional_number("short integer")
            .and_then(|value| parse_optional(value, "short integer"))
    }

    pub fn read_u16(&mut self) -> Result<Option<u16>> {
        self.optional_number("unsigned short integer")
            .and_then(|value| parse_optional(value, "unsigned short integer"))
    }

    pub fn read_i32(&mut self) -> Result<Option<i32>> {
        self.optional_number("integer")
            .and_then(|value| parse_optional(value, "integer"))
    }

    pub fn read_index(&mut self) -> Result<Option<u32>> {
        self.optional_number("pointer index")
            .and_then(|value| parse_optional(value, "pointer index"))
    }

    /// Read the final terminator index. Unlike ordinary numeric fields, the
    /// last value has no following entry to separate and may end at EOF after
    /// physical record padding has been removed.
    pub fn read_final_index(&mut self) -> Result<Option<u32>> {
        if self.stream.logical.get(self.position) == Some(&b'?') {
            self.position += 1;
            return Ok(None);
        }
        let start = self.position;
        let tail = self
            .stream
            .logical
            .get(start..)
            .ok_or_else(|| self.eof("final pointer index"))?;
        let (bytes, consumed) = match tail.iter().position(|byte| *byte == b' ') {
            Some(length) => (&tail[..length], length + 1),
            None => (tail, tail.len()),
        };
        if bytes.is_empty() {
            return Err(XtError::new(
                ErrorKind::InvalidNumber,
                self.stream.raw_span(start, start),
                "final pointer index is empty",
            ));
        }
        self.position += consumed;
        parse_number(
            bytes,
            self.stream.raw_span(start, self.position),
            "final pointer index",
        )
        .map(Some)
    }

    pub fn read_f64(&mut self) -> Result<Option<f64>> {
        self.optional_number("double")
            .and_then(|value| parse_optional(value, "double"))
    }

    pub fn read_logical(&mut self) -> Result<bool> {
        let start = self.position;
        let byte = self.take_byte("logical")?;
        match byte {
            b'F' => Ok(false),
            b'T' => Ok(true),
            _ => Err(XtError::new(
                ErrorKind::InvalidLogical,
                self.stream.raw_span(start, self.position),
                "logical value must be F or T",
            )),
        }
    }

    pub fn read_char(&mut self) -> Result<u8> {
        let bytes = self.read_chars(1)?;
        Ok(bytes[0])
    }

    /// Read exactly `count` decoded character values. The documented `\\9`
    /// compression sequence expands to nine spaces and is rejected if it would
    /// cross the schema-provided character-field boundary.
    pub fn read_chars(&mut self, count: usize) -> Result<Vec<u8>> {
        let start = self.position;
        let mut output = Vec::with_capacity(count);
        while output.len() < count {
            let byte = self.take_byte("character field")?;
            if byte != b'\\' {
                output.push(byte);
                continue;
            }

            let escape_start = self.position - 1;
            let escaped = self.take_byte("character escape")?;
            match escaped {
                b'0' => output.push(0),
                // The primary reference names these from the represented
                // character: \n is CR and \r is LF.
                b'n' => output.push(b'\r'),
                b'r' => output.push(b'\n'),
                b'\\' => output.push(b'\\'),
                b'9' => {
                    if output.len() + 9 > count {
                        return Err(XtError::new(
                            ErrorKind::InvalidEscape,
                            self.stream.raw_span(escape_start, self.position),
                            "\\9 expansion crosses the requested character-field boundary",
                        ));
                    }
                    output.extend_from_slice(b"         ");
                }
                _ => {
                    return Err(XtError::new(
                        ErrorKind::InvalidEscape,
                        self.stream.raw_span(escape_start, self.position),
                        "unknown XT text character escape",
                    ));
                }
            }
        }
        debug_assert_eq!(output.len(), count);
        debug_assert!(self.position >= start);
        Ok(output)
    }

    pub fn read_short_string(&mut self) -> Result<Vec<u8>> {
        let length = usize::from(self.read_u8()?);
        self.read_chars(length)
    }

    pub fn read_vector(&mut self) -> Result<Option<[f64; 3]>> {
        let Some(x) = self.read_f64()? else {
            return Ok(None);
        };
        let y = self.required_f64("vector y")?;
        let z = self.required_f64("vector z")?;
        Ok(Some([x, y, z]))
    }

    pub fn read_interval(&mut self) -> Result<Option<[f64; 2]>> {
        let Some(low) = self.read_f64()? else {
            return Ok(None);
        };
        let high = self.required_f64("interval high")?;
        Ok(Some([low, high]))
    }

    pub fn read_box(&mut self) -> Result<Option<[f64; 6]>> {
        let Some(x_low) = self.read_f64()? else {
            return Ok(None);
        };
        let mut values = [0.0; 6];
        values[0] = x_low;
        for (index, slot) in values.iter_mut().enumerate().skip(1) {
            *slot = self.required_f64(&format!("box component {index}"))?;
        }
        Ok(Some(values))
    }

    pub fn read_prefix(&mut self) -> Result<TextPrefix> {
        let start = self.position;
        let flag = self.take_byte("text format flag")?;
        if flag != b'T' {
            return Err(XtError::new(
                ErrorKind::UnsupportedFormat,
                self.stream.raw_span(start, self.position),
                "text body must begin with T",
            ));
        }

        let modeler_length = self.required_nonnegative_length("modeler version length")?;
        let modeler_version = self.read_chars(modeler_length)?;
        let schema_length = self.required_nonnegative_length("schema name length")?;
        let schema_name = self.read_chars(schema_length)?;
        let max_node_types = if has_embedded_base_schema(&schema_name) {
            let value = self.read_i16()?.ok_or_else(|| {
                XtError::new(
                    ErrorKind::InvalidNumber,
                    self.stream
                        .raw_span(self.position.saturating_sub(1), self.position),
                    "maximum node type cannot be unset",
                )
            })?;
            Some(u16::try_from(value).map_err(|_| {
                XtError::new(
                    ErrorKind::NumberOutOfRange,
                    self.stream
                        .raw_span(self.position.saturating_sub(1), self.position),
                    "maximum node type must not be negative",
                )
            })?)
        } else {
            None
        };
        let user_field_size = self.required_i32("user field size")?;
        if !(0..=16).contains(&user_field_size) {
            return Err(XtError::new(
                ErrorKind::NumberOutOfRange,
                self.stream.raw_span(start, self.position),
                "user field size must be in 0..=16",
            ));
        }

        Ok(TextPrefix {
            modeler_version,
            schema_name,
            max_node_types,
            user_field_size: user_field_size as u8,
            span: self.stream.raw_span(start, self.position),
        })
    }

    fn required_nonnegative_length(&mut self, context: &str) -> Result<usize> {
        let value = self.required_i32(context)?;
        usize::try_from(value).map_err(|_| {
            XtError::new(
                ErrorKind::NumberOutOfRange,
                self.stream.raw_span(self.position, self.position),
                format!("{context} must not be negative"),
            )
        })
    }

    fn required_i32(&mut self, context: &str) -> Result<i32> {
        self.read_i32()?.ok_or_else(|| {
            XtError::new(
                ErrorKind::InvalidNumber,
                self.stream
                    .raw_span(self.position.saturating_sub(1), self.position),
                format!("{context} cannot be unset"),
            )
        })
    }

    fn required_f64(&mut self, context: &str) -> Result<f64> {
        self.read_f64()?.ok_or_else(|| {
            XtError::new(
                ErrorKind::InvalidNumber,
                self.stream
                    .raw_span(self.position.saturating_sub(1), self.position),
                format!("{context} cannot be unset independently"),
            )
        })
    }

    fn optional_number(&mut self, context: &str) -> Result<Option<(Span, Vec<u8>)>> {
        if self.stream.logical.get(self.position) == Some(&b'?') {
            let start = self.position;
            self.position += 1;
            let _span = self.stream.raw_span(start, self.position);
            return Ok(None);
        }
        self.required_number(context).map(Some)
    }

    fn required_number(&mut self, context: &str) -> Result<(Span, Vec<u8>)> {
        let start = self.position;
        let tail = self
            .stream
            .logical
            .get(start..)
            .ok_or_else(|| self.eof(context))?;
        let length = tail.iter().position(|byte| *byte == b' ').ok_or_else(|| {
            XtError::new(
                ErrorKind::ExpectedDelimiter,
                self.stream.raw_span(start, self.stream.logical.len()),
                format!("{context} is missing its trailing space"),
            )
        })?;
        if length == 0 {
            return Err(XtError::new(
                ErrorKind::InvalidNumber,
                self.stream.raw_span(start, start + 1),
                format!("{context} is empty"),
            ));
        }
        let end = start + length;
        let bytes = self.stream.logical[start..end].to_vec();
        self.position = end + 1;
        Ok((self.stream.raw_span(start, self.position), bytes))
    }

    fn take_byte(&mut self, context: &str) -> Result<u8> {
        let byte = self
            .stream
            .logical
            .get(self.position)
            .copied()
            .ok_or_else(|| self.eof(context))?;
        self.position += 1;
        Ok(byte)
    }

    fn eof(&self, context: &str) -> XtError {
        XtError::new(
            ErrorKind::UnexpectedEof,
            self.stream.raw_span(self.position, self.position),
            format!("unexpected end while reading {context}"),
        )
    }
}

fn parse_optional<T>(value: Option<(Span, Vec<u8>)>, context: &str) -> Result<Option<T>>
where
    T: std::str::FromStr,
{
    value
        .map(|(span, bytes)| parse_number(&bytes, span, context))
        .transpose()
}

fn parse_number<T>(bytes: &[u8], span: Span, context: &str) -> Result<T>
where
    T: std::str::FromStr,
{
    let text = std::str::from_utf8(bytes).map_err(|_| {
        XtError::new(
            ErrorKind::InvalidNumber,
            span,
            format!("{context} is not ASCII"),
        )
    })?;
    text.parse::<T>().map_err(|_| {
        XtError::new(
            ErrorKind::NumberOutOfRange,
            span,
            format!("invalid or out-of-range {context}: {text:?}"),
        )
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextPrefix {
    pub modeler_version: Vec<u8>,
    pub schema_name: Vec<u8>,
    /// Present when the schema name carries the V13 base-schema suffix.
    pub max_node_types: Option<u16>,
    pub user_field_size: u8,
    pub span: Span,
}

fn has_embedded_base_schema(schema_name: &[u8]) -> bool {
    let parts = schema_name.split(|byte| *byte == b'_').collect::<Vec<_>>();
    parts.len() == 4
        && parts[0] == b"SCH"
        && parts[1..]
            .iter()
            .all(|part| !part.is_empty() && part.iter().all(u8::is_ascii_digit))
}

#[derive(Debug, Clone)]
pub struct TextEnvelope {
    pub header: CommonHeader,
    pub stream: TextStream,
    pub prefix: TextPrefix,
    pub node_start: usize,
}

impl TextEnvelope {
    pub fn parse(input: &[u8]) -> Result<Self> {
        let header = CommonHeader::parse(input)?;
        let format = header.value("FORMAT").ok_or_else(|| {
            XtError::new(
                ErrorKind::InvalidHeader,
                header.header_span,
                "missing FORMAT common-header key",
            )
        })?;
        if !format.eq_ignore_ascii_case(b"text") {
            return Err(XtError::new(
                ErrorKind::UnsupportedFormat,
                header.header_span,
                format!("FORMAT is {:?}, not text", String::from_utf8_lossy(format)),
            ));
        }

        let stream = TextStream::from_raw(&input[header.body_offset..], header.body_offset);
        let mut cursor = stream.cursor();
        let prefix = cursor.read_prefix()?;
        let node_start = cursor.position();
        Ok(Self {
            header,
            stream,
            prefix,
            node_start,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_record_breaks_and_padding_but_keeps_leading_space() {
        let stream = TextStream::from_raw(b"12  \r\n 1\nT", 100);
        assert_eq!(stream.logical_bytes(), b"12 1T");
        assert_eq!(stream.raw_offsets, vec![100, 101, 106, 107, 109]);
    }

    #[test]
    fn typed_reads_disambiguate_adjacent_values() {
        let stream = TextStream::from_raw(b"?10 0 +0 FTT", 0);
        let mut cursor = stream.cursor();
        assert_eq!(cursor.read_i32().unwrap(), None);
        assert_eq!(cursor.read_i32().unwrap(), Some(10));
        assert_eq!(cursor.read_i32().unwrap(), Some(0));
        assert_eq!(cursor.read_f64().unwrap(), Some(0.0));
        assert!(!cursor.read_logical().unwrap());
        assert!(cursor.read_logical().unwrap());
        assert!(cursor.read_logical().unwrap());
        assert_eq!(cursor.remaining(), 0);
    }

    #[test]
    fn expands_documented_space_compression_inside_field_boundary() {
        let stream = TextStream::from_raw(b"10 a\\9", 0);
        let mut cursor = stream.cursor();
        assert_eq!(cursor.read_short_string().unwrap(), b"a         ");
        assert_eq!(cursor.remaining(), 0);
    }

    #[test]
    fn parses_text_prefix_across_physical_records() {
        let input = b"preamble\n\
**PART1;FORMAT=text;\n\
**PART2;SCH=SCH_3000000_30000;USFLD_SIZE=0;\n\
**PART3;\n\
**END_OF_HEADER***\n\
T51 : TRANSMIT FILE created by modeller version 3000000\n\
17 SCH_3000000_30000\n\
0 12 1 ";
        let envelope = TextEnvelope::parse(input).unwrap();
        assert_eq!(
            envelope.prefix.modeler_version,
            b": TRANSMIT FILE created by modeller version 3000000"
        );
        assert_eq!(envelope.prefix.schema_name, b"SCH_3000000_30000");
        assert_eq!(envelope.prefix.max_node_types, None);
        assert_eq!(envelope.prefix.user_field_size, 0);
        let mut nodes = envelope.stream.cursor_at(envelope.node_start).unwrap();
        assert_eq!(nodes.read_i32().unwrap(), Some(12));
        assert_eq!(nodes.read_i32().unwrap(), Some(1));
    }

    #[test]
    fn parses_embedded_schema_maximum_node_type() {
        let input = b"preamble\n\
**PART1;FORMAT=text;\n\
**PART2;SCH=SCH_3001168_30100_13006;USFLD_SIZE=0;\n\
**PART3;\n\
**END_OF_HEADER***\n\
T1 X23 SCH_3001168_30100_13006214 0 12 ";
        let envelope = TextEnvelope::parse(input).unwrap();
        assert_eq!(envelope.prefix.schema_name, b"SCH_3001168_30100_13006");
        assert_eq!(envelope.prefix.max_node_types, Some(214));
        assert_eq!(envelope.prefix.user_field_size, 0);
        let mut nodes = envelope.stream.cursor_at(envelope.node_start).unwrap();
        assert_eq!(nodes.read_i32().unwrap(), Some(12));
    }

    #[test]
    fn rejects_normal_number_without_delimiter() {
        let stream = TextStream::from_raw(b"12", 0);
        let error = stream.cursor().read_i32().unwrap_err();
        assert_eq!(error.kind, ErrorKind::ExpectedDelimiter);
    }
}
