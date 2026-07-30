use serde::Serialize;

use crate::{CommonHeader, ErrorKind, Result, Span, XtError};

const UNSET_INTEGRAL: i32 = -32_764;
const UNSET_REAL: f64 = -3.14158e13;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ByteOrder {
    BigEndian,
    LittleEndian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoubleEncoding {
    Ieee,
    VaxDFloat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterEncoding {
    Ascii,
    Ebcdic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MachineEncoding {
    pub byte_order: ByteOrder,
    pub doubles: DoubleEncoding,
    pub characters: CharacterEncoding,
}

impl MachineEncoding {
    pub const LITTLE_IEEE_ASCII: Self = Self {
        byte_order: ByteOrder::LittleEndian,
        doubles: DoubleEncoding::Ieee,
        characters: CharacterEncoding::Ascii,
    };

    pub const BIG_IEEE_ASCII: Self = Self {
        byte_order: ByteOrder::BigEndian,
        doubles: DoubleEncoding::Ieee,
        characters: CharacterEncoding::Ascii,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryKind {
    Bare,
    Typed,
    Neutral,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BinaryPrefix {
    pub kind: BinaryKind,
    pub machine: MachineEncoding,
    pub modeler_version: Vec<u8>,
    pub schema_name: Vec<u8>,
    pub max_node_types: Option<u16>,
    pub user_field_size: u8,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct BinaryEnvelope {
    pub header: CommonHeader,
    pub prefix: BinaryPrefix,
    pub node_start: usize,
}

impl BinaryEnvelope {
    pub fn parse(input: &[u8]) -> Result<Self> {
        Self::parse_with_bare_machine(input, None)
    }

    pub fn parse_with_bare_machine(
        input: &[u8],
        bare_machine: Option<MachineEncoding>,
    ) -> Result<Self> {
        let header = CommonHeader::parse(input)?;
        let mut cursor = PrefixCursor::new(input, header.body_offset);
        let start = cursor.position;

        let (kind, machine, bare_length_is_i32) = match cursor.peek(4) {
            Some(b"PS\0\0") => {
                cursor.position += 4;
                (BinaryKind::Neutral, MachineEncoding::BIG_IEEE_ASCII, false)
            }
            Some(b"PS\0\x01") => {
                cursor.position += 4;
                let descriptor = cursor.take(3, "typed-binary machine descriptor")?;
                let machine = MachineEncoding {
                    byte_order: match descriptor[0] {
                        0 => ByteOrder::BigEndian,
                        1 => ByteOrder::LittleEndian,
                        other => {
                            return Err(binary_error(
                                cursor.span_from(start),
                                format!("unknown typed-binary byte-order code {other}"),
                            ));
                        }
                    },
                    doubles: match descriptor[1] {
                        0 => DoubleEncoding::Ieee,
                        1 => DoubleEncoding::VaxDFloat,
                        other => {
                            return Err(binary_error(
                                cursor.span_from(start),
                                format!("unknown typed-binary double code {other}"),
                            ));
                        }
                    },
                    characters: match descriptor[2] {
                        0 => CharacterEncoding::Ascii,
                        1 => CharacterEncoding::Ebcdic,
                        other => {
                            return Err(binary_error(
                                cursor.span_from(start),
                                format!("unknown typed-binary character code {other}"),
                            ));
                        }
                    },
                };
                (BinaryKind::Typed, machine, false)
            }
            _ if cursor.peek(1) == Some(b"B") => {
                cursor.position += 1;
                let machine = bare_machine.ok_or_else(|| {
                    XtError::new(
                        ErrorKind::UnsupportedFormat,
                        Span::new(start, start + 1),
                        "bare binary is machine-dependent; supply its machine encoding",
                    )
                })?;
                (BinaryKind::Bare, machine, true)
            }
            _ => {
                return Err(XtError::new(
                    ErrorKind::UnsupportedFormat,
                    Span::new(start, (start + 7).min(input.len())),
                    "body is not bare, typed, or neutral binary XT",
                ));
            }
        };

        if machine.doubles != DoubleEncoding::Ieee {
            return Err(XtError::new(
                ErrorKind::UnsupportedFormat,
                cursor.span_from(start),
                "VAX D-float binary XT is identified but not decoded",
            ));
        }
        if machine.characters != CharacterEncoding::Ascii {
            return Err(XtError::new(
                ErrorKind::UnsupportedFormat,
                cursor.span_from(start),
                "EBCDIC binary XT is identified but not decoded",
            ));
        }

        cursor.machine = machine;
        let modeler_length = if bare_length_is_i32 {
            cursor.read_nonnegative_i32("modeler version length")?
        } else {
            cursor.read_nonnegative_i16("modeler version length")?
        };
        let modeler_version = cursor.take(modeler_length, "modeler version")?.to_vec();
        let schema_length = cursor.read_nonnegative_i32("schema name length")?;
        let schema_name = cursor.take(schema_length, "schema name")?.to_vec();
        let max_node_types = if has_embedded_base_schema(&schema_name) {
            Some(cursor.read_nonnegative_i16("maximum node type")? as u16)
        } else {
            None
        };
        let user_field_size = cursor.read_nonnegative_i32("user field size")?;
        if user_field_size > 16 {
            return Err(XtError::new(
                ErrorKind::NumberOutOfRange,
                cursor.span_from(start),
                "user field size must be in 0..=16",
            ));
        }
        let prefix = BinaryPrefix {
            kind,
            machine,
            modeler_version,
            schema_name,
            max_node_types,
            user_field_size: user_field_size as u8,
            span: cursor.span_from(start),
        };
        Ok(Self {
            header,
            prefix,
            node_start: cursor.position,
        })
    }

    pub(crate) fn cursor<'a>(&self, input: &'a [u8]) -> Result<BinaryCursor<'a>> {
        BinaryCursor::new(input, self.node_start, self.prefix.machine)
    }
}

pub(crate) struct BinaryCursor<'a> {
    input: &'a [u8],
    position: usize,
    machine: MachineEncoding,
}

impl<'a> BinaryCursor<'a> {
    fn new(input: &'a [u8], position: usize, machine: MachineEncoding) -> Result<Self> {
        if position > input.len() {
            return Err(XtError::new(
                ErrorKind::UnexpectedEof,
                Span::at(input.len()),
                "binary cursor starts beyond end of input",
            ));
        }
        Ok(Self {
            input,
            position,
            machine,
        })
    }

    pub(crate) fn position(&self) -> usize {
        self.position
    }

    pub(crate) fn remaining(&self) -> usize {
        self.input.len() - self.position
    }

    pub(crate) fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.position)
    }

    pub(crate) fn read_u8(&mut self) -> Result<u8> {
        Ok(self.take::<1>("unsigned byte")?[0])
    }

    pub(crate) fn read_i16(&mut self) -> Result<Option<i16>> {
        let value = self.raw_i16("short integer")?;
        Ok((i32::from(value) != UNSET_INTEGRAL).then_some(value))
    }

    pub(crate) fn read_u16(&mut self) -> Result<Option<u16>> {
        let value = self.raw_u16("unsigned short integer")?;
        Ok((value != UNSET_INTEGRAL as i16 as u16).then_some(value))
    }

    pub(crate) fn read_i32(&mut self) -> Result<Option<i32>> {
        let value = self.raw_i32("integer")?;
        Ok((value != UNSET_INTEGRAL).then_some(value))
    }

    pub(crate) fn read_index(&mut self) -> Result<Option<u32>> {
        let start = self.position;
        let remainder = self.raw_i16("pointer index")?;
        if remainder == 0 {
            return Ok(None);
        }
        let (quotient, positive_remainder) = if remainder < 0 {
            (
                u32::from(self.raw_u16("large pointer quotient")?),
                u32::from(remainder.unsigned_abs()),
            )
        } else {
            (0, remainder as u32)
        };
        let combined = quotient
            .checked_mul(32_767)
            .and_then(|value| value.checked_add(positive_remainder))
            .and_then(|value| value.checked_sub(1))
            .ok_or_else(|| {
                XtError::new(
                    ErrorKind::NumberOutOfRange,
                    self.span_from(start),
                    "binary pointer index overflows or has invalid zero encoding",
                )
            })?;
        Ok(Some(combined))
    }

    pub(crate) fn read_final_index(&mut self) -> Result<Option<u32>> {
        self.read_index()
    }

    pub(crate) fn read_f64(&mut self) -> Result<Option<f64>> {
        let bytes = self.take::<8>("double")?;
        let value = match self.machine.byte_order {
            ByteOrder::BigEndian => f64::from_be_bytes(bytes),
            ByteOrder::LittleEndian => f64::from_le_bytes(bytes),
        };
        Ok((value != UNSET_REAL).then_some(value))
    }

    pub(crate) fn read_logical(&mut self) -> Result<bool> {
        let start = self.position;
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(XtError::new(
                ErrorKind::InvalidLogical,
                self.span_from(start),
                format!("binary logical must be 0 or 1, got {other}"),
            )),
        }
    }

    pub(crate) fn read_char(&mut self) -> Result<u8> {
        self.read_u8()
    }

    pub(crate) fn read_chars(&mut self, count: usize) -> Result<Vec<u8>> {
        Ok(self.take_slice(count, "character array")?.to_vec())
    }

    pub(crate) fn read_short_string(&mut self) -> Result<Vec<u8>> {
        let length = usize::from(self.read_u8()?);
        self.read_chars(length)
    }

    pub(crate) fn read_vector(&mut self) -> Result<Option<[f64; 3]>> {
        let values = [self.read_f64()?, self.read_f64()?, self.read_f64()?];
        collapse_reals(
            values,
            self.span_from(self.position.saturating_sub(24)),
            "vector",
        )
    }

    pub(crate) fn read_interval(&mut self) -> Result<Option<[f64; 2]>> {
        let values = [self.read_f64()?, self.read_f64()?];
        collapse_reals(
            values,
            self.span_from(self.position.saturating_sub(16)),
            "interval",
        )
    }

    pub(crate) fn read_box(&mut self) -> Result<Option<[f64; 6]>> {
        let values = [
            self.read_f64()?,
            self.read_f64()?,
            self.read_f64()?,
            self.read_f64()?,
            self.read_f64()?,
            self.read_f64()?,
        ];
        collapse_reals(
            values,
            self.span_from(self.position.saturating_sub(48)),
            "box",
        )
    }

    fn raw_i16(&mut self, context: &str) -> Result<i16> {
        let bytes = self.take::<2>(context)?;
        Ok(match self.machine.byte_order {
            ByteOrder::BigEndian => i16::from_be_bytes(bytes),
            ByteOrder::LittleEndian => i16::from_le_bytes(bytes),
        })
    }

    fn raw_u16(&mut self, context: &str) -> Result<u16> {
        let bytes = self.take::<2>(context)?;
        Ok(match self.machine.byte_order {
            ByteOrder::BigEndian => u16::from_be_bytes(bytes),
            ByteOrder::LittleEndian => u16::from_le_bytes(bytes),
        })
    }

    fn raw_i32(&mut self, context: &str) -> Result<i32> {
        let bytes = self.take::<4>(context)?;
        Ok(match self.machine.byte_order {
            ByteOrder::BigEndian => i32::from_be_bytes(bytes),
            ByteOrder::LittleEndian => i32::from_le_bytes(bytes),
        })
    }

    fn take<const N: usize>(&mut self, context: &str) -> Result<[u8; N]> {
        let bytes = self.take_slice(N, context)?;
        Ok(bytes.try_into().expect("slice has requested fixed length"))
    }

    fn take_slice(&mut self, count: usize, context: &str) -> Result<&'a [u8]> {
        let start = self.position;
        let end = start.checked_add(count).ok_or_else(|| {
            XtError::new(
                ErrorKind::NumberOutOfRange,
                Span::at(start),
                format!("{context} length overflows"),
            )
        })?;
        let bytes = self.input.get(start..end).ok_or_else(|| {
            XtError::new(
                ErrorKind::UnexpectedEof,
                Span::new(start, self.input.len()),
                format!("truncated {context}: need {count} bytes"),
            )
        })?;
        self.position = end;
        Ok(bytes)
    }
}

struct PrefixCursor<'a> {
    input: &'a [u8],
    position: usize,
    machine: MachineEncoding,
}

impl<'a> PrefixCursor<'a> {
    fn new(input: &'a [u8], position: usize) -> Self {
        Self {
            input,
            position,
            machine: MachineEncoding::BIG_IEEE_ASCII,
        }
    }

    fn peek(&self, count: usize) -> Option<&'a [u8]> {
        self.input.get(self.position..self.position + count)
    }

    fn take(&mut self, count: usize, context: &str) -> Result<&'a [u8]> {
        let start = self.position;
        let end = start.checked_add(count).ok_or_else(|| {
            XtError::new(
                ErrorKind::NumberOutOfRange,
                Span::at(start),
                format!("{context} length overflows"),
            )
        })?;
        let output = self.input.get(start..end).ok_or_else(|| {
            XtError::new(
                ErrorKind::UnexpectedEof,
                Span::new(start, self.input.len()),
                format!("truncated {context}"),
            )
        })?;
        self.position = end;
        Ok(output)
    }

    fn read_nonnegative_i16(&mut self, context: &str) -> Result<usize> {
        let start = self.position;
        let bytes: [u8; 2] = self
            .take(2, context)?
            .try_into()
            .expect("fixed length checked");
        let value = match self.machine.byte_order {
            ByteOrder::BigEndian => i16::from_be_bytes(bytes),
            ByteOrder::LittleEndian => i16::from_le_bytes(bytes),
        };
        usize::try_from(value).map_err(|_| {
            XtError::new(
                ErrorKind::NumberOutOfRange,
                self.span_from(start),
                format!("{context} must not be negative"),
            )
        })
    }

    fn read_nonnegative_i32(&mut self, context: &str) -> Result<usize> {
        let start = self.position;
        let bytes: [u8; 4] = self
            .take(4, context)?
            .try_into()
            .expect("fixed length checked");
        let value = match self.machine.byte_order {
            ByteOrder::BigEndian => i32::from_be_bytes(bytes),
            ByteOrder::LittleEndian => i32::from_le_bytes(bytes),
        };
        usize::try_from(value).map_err(|_| {
            XtError::new(
                ErrorKind::NumberOutOfRange,
                self.span_from(start),
                format!("{context} must not be negative"),
            )
        })
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.position)
    }
}

fn collapse_reals<const N: usize>(
    values: [Option<f64>; N],
    span: Span,
    context: &str,
) -> Result<Option<[f64; N]>> {
    if values.iter().all(Option::is_none) {
        return Ok(None);
    }
    if values.iter().any(Option::is_none) {
        return Err(XtError::new(
            ErrorKind::InvalidNumber,
            span,
            format!("binary {context} mixes set and unset components"),
        ));
    }
    Ok(Some(
        values.map(|value| value.expect("all components checked")),
    ))
}

fn has_embedded_base_schema(schema_name: &[u8]) -> bool {
    let parts = schema_name.split(|byte| *byte == b'_').collect::<Vec<_>>();
    parts.len() == 4
        && parts[0] == b"SCH"
        && parts[1..]
            .iter()
            .all(|part| !part.is_empty() && part.iter().all(u8::is_ascii_digit))
}

fn binary_error(span: Span, context: impl Into<String>) -> XtError {
    XtError::new(ErrorKind::UnsupportedFormat, span, context)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(body: &[u8]) -> Vec<u8> {
        let mut input = b"preamble\n\
**PART1;FORMAT=binary;\n\
**PART2;SCH=SCH_3701243_37102;USFLD_SIZE=0;\n\
**PART3;\n\
**END_OF_HEADER***\n"
            .to_vec();
        input.extend_from_slice(body);
        input
    }

    #[test]
    fn parses_neutral_prefix() {
        let mut body = b"PS\0\0\0\x01X\0\0\0\x17SCH_3701243_37102_13006".to_vec();
        body.extend_from_slice(&[0, 239, 0, 0, 0, 0]);
        let input = header(&body);
        let envelope = BinaryEnvelope::parse(&input).unwrap();
        assert_eq!(envelope.prefix.kind, BinaryKind::Neutral);
        assert_eq!(envelope.prefix.machine, MachineEncoding::BIG_IEEE_ASCII);
        assert_eq!(envelope.prefix.modeler_version, b"X");
        assert_eq!(envelope.prefix.schema_name, b"SCH_3701243_37102_13006");
        assert_eq!(envelope.prefix.max_node_types, Some(239));
        assert_eq!(envelope.prefix.user_field_size, 0);
    }

    #[test]
    fn bare_binary_requires_an_explicit_machine() {
        let input = header(b"B");
        let error = BinaryEnvelope::parse(&input).unwrap_err();
        assert_eq!(error.kind, ErrorKind::UnsupportedFormat);
    }
}
