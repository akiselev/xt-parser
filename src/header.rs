use serde::Serialize;

use crate::{ErrorKind, Result, Span, XtError};

const PART1: &[u8] = b"**PART1;";
const PART2: &[u8] = b"**PART2;";
const PART3: &[u8] = b"**PART3;";
const END: &[u8] = b"**END_OF_HEADER";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderPart {
    Part1,
    Part2,
    Part3,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeaderEntry {
    pub part: HeaderPart,
    pub name: String,
    /// Decoded value bytes. Header values are defined over 7-bit ASCII plus LF.
    pub value: Vec<u8>,
    /// Value bytes before caret escape decoding, with physical CR/LF removed.
    pub encoded_value: Vec<u8>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderDiagnosticKind {
    UnescapedSpace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeaderDiagnostic {
    pub kind: HeaderDiagnosticKind,
    pub span: Span,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommonHeader {
    pub preamble_span: Span,
    pub header_span: Span,
    pub body_offset: usize,
    pub entries: Vec<HeaderEntry>,
    pub diagnostics: Vec<HeaderDiagnostic>,
}

impl CommonHeader {
    pub fn parse(input: &[u8]) -> Result<Self> {
        let end_raw = find_subslice(input, END).ok_or_else(|| {
            XtError::new(
                ErrorKind::MissingHeaderMarker,
                Span::new(0, input.len()),
                "missing **END_OF_HEADER",
            )
        })?;

        let trailer_start = end_raw + END.len();
        let body_offset = match input[trailer_start..]
            .iter()
            .position(|byte| matches!(byte, b'\r' | b'\n'))
        {
            Some(relative) => {
                let mut offset = trailer_start + relative;
                while matches!(input.get(offset), Some(b'\r' | b'\n')) {
                    offset += 1;
                }
                offset
            }
            None => input.len(),
        };

        let logical = LogicalHeader::new(&input[..body_offset]);
        let p1 = logical.find(PART1).ok_or_else(|| {
            XtError::new(
                ErrorKind::MissingHeaderMarker,
                Span::new(0, body_offset),
                "missing **PART1;",
            )
        })?;
        let p2 = logical.find_after(PART2, p1 + PART1.len()).ok_or_else(|| {
            XtError::new(
                ErrorKind::MissingHeaderMarker,
                Span::new(0, body_offset),
                "missing **PART2;",
            )
        })?;
        let p3 = logical.find_after(PART3, p2 + PART2.len()).ok_or_else(|| {
            XtError::new(
                ErrorKind::MissingHeaderMarker,
                Span::new(0, body_offset),
                "missing **PART3;",
            )
        })?;
        let end = logical.find_after(END, p3 + PART3.len()).ok_or_else(|| {
            XtError::new(
                ErrorKind::MissingHeaderMarker,
                Span::new(0, body_offset),
                "missing ordered **END_OF_HEADER",
            )
        })?;

        if !(p1 < p2 && p2 < p3 && p3 < end) {
            return Err(XtError::new(
                ErrorKind::InvalidHeader,
                Span::new(0, body_offset),
                "common-header parts are out of order",
            ));
        }

        let mut entries = Vec::new();
        let mut diagnostics = Vec::new();
        parse_entries(
            &logical,
            HeaderPart::Part1,
            p1 + PART1.len(),
            p2,
            &mut entries,
            &mut diagnostics,
        )?;
        parse_entries(
            &logical,
            HeaderPart::Part2,
            p2 + PART2.len(),
            p3,
            &mut entries,
            &mut diagnostics,
        )?;
        parse_entries(
            &logical,
            HeaderPart::Part3,
            p3 + PART3.len(),
            end,
            &mut entries,
            &mut diagnostics,
        )?;

        Ok(Self {
            preamble_span: logical.raw_span(0, p1),
            header_span: Span::new(0, body_offset),
            body_offset,
            entries,
            diagnostics,
        })
    }

    pub fn values<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a [u8]> + 'a {
        let name = name.to_owned();
        self.entries
            .iter()
            .filter(move |entry| entry.name == name)
            .map(|entry| entry.value.as_slice())
    }

    pub fn value(&self, name: &str) -> Option<&[u8]> {
        self.values(name).next()
    }
}

struct LogicalHeader {
    bytes: Vec<u8>,
    raw_offsets: Vec<usize>,
    raw_end: usize,
}

impl LogicalHeader {
    fn new(raw: &[u8]) -> Self {
        let mut bytes = Vec::with_capacity(raw.len());
        let mut raw_offsets = Vec::with_capacity(raw.len());
        for (offset, byte) in raw.iter().copied().enumerate() {
            if !matches!(byte, b'\r' | b'\n') {
                bytes.push(byte);
                raw_offsets.push(offset);
            }
        }
        Self {
            bytes,
            raw_offsets,
            raw_end: raw.len(),
        }
    }

    fn find(&self, needle: &[u8]) -> Option<usize> {
        find_subslice(&self.bytes, needle)
    }

    fn find_after(&self, needle: &[u8], start: usize) -> Option<usize> {
        find_subslice(self.bytes.get(start..)?, needle).map(|relative| start + relative)
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

fn parse_entries(
    logical: &LogicalHeader,
    part: HeaderPart,
    start: usize,
    end: usize,
    output: &mut Vec<HeaderEntry>,
    diagnostics: &mut Vec<HeaderDiagnostic>,
) -> Result<()> {
    let mut cursor = start;
    while cursor < end {
        let entry_start = cursor;
        let name_start = cursor;
        while cursor < end && is_key_byte(logical.bytes[cursor]) {
            cursor += 1;
        }
        let name_len = cursor - name_start;
        if !(1..=80).contains(&name_len) {
            return Err(XtError::new(
                ErrorKind::InvalidHeaderKey,
                logical.raw_span(name_start, cursor.max(name_start + 1).min(end)),
                "header key must contain 1..=80 uppercase, digit, or underscore bytes",
            ));
        }
        if logical.bytes.get(cursor) != Some(&b'=') {
            return Err(XtError::new(
                ErrorKind::InvalidHeaderKey,
                logical.raw_span(cursor, (cursor + 1).min(end)),
                "expected '=' after header key",
            ));
        }
        cursor += 1;

        let value_start = cursor;
        let mut decoded = Vec::new();
        while cursor < end && logical.bytes[cursor] != b';' {
            let byte = logical.bytes[cursor];
            if byte == b'^' {
                let escape_start = cursor;
                cursor += 1;
                let escaped = logical.bytes.get(cursor).copied().ok_or_else(|| {
                    XtError::new(
                        ErrorKind::InvalidEscape,
                        logical.raw_span(escape_start, cursor),
                        "truncated caret escape",
                    )
                })?;
                decoded.push(match escaped {
                    b'n' => b'\n',
                    b'-' => b' ',
                    b';' => b';',
                    b'^' => b'^',
                    _ => {
                        return Err(XtError::new(
                            ErrorKind::InvalidEscape,
                            logical.raw_span(escape_start, cursor + 1),
                            "unknown header caret escape",
                        ));
                    }
                });
                cursor += 1;
            } else if byte == b' ' {
                diagnostics.push(HeaderDiagnostic {
                    kind: HeaderDiagnosticKind::UnescapedSpace,
                    span: logical.raw_span(cursor, cursor + 1),
                    context: "accepted de facto unescaped header space; canonical spelling is ^-"
                        .to_owned(),
                });
                decoded.push(byte);
                cursor += 1;
            } else if (0x21..=0x7e).contains(&byte) {
                decoded.push(byte);
                cursor += 1;
            } else {
                return Err(XtError::new(
                    ErrorKind::InvalidHeaderValue,
                    logical.raw_span(cursor, cursor + 1),
                    "header value contains an unescaped non-printing byte or space",
                ));
            }
        }
        if cursor == value_start {
            return Err(XtError::new(
                ErrorKind::InvalidHeaderValue,
                logical.raw_span(value_start, value_start),
                "header values must not be empty",
            ));
        }
        if cursor == end {
            return Err(XtError::new(
                ErrorKind::InvalidHeaderValue,
                logical.raw_span(entry_start, end),
                "header entry is missing its semicolon",
            ));
        }

        let name = String::from_utf8(logical.bytes[name_start..name_start + name_len].to_vec())
            .expect("validated header keys are ASCII");
        let encoded_value = logical.bytes[value_start..cursor].to_vec();
        cursor += 1;
        output.push(HeaderEntry {
            part,
            name,
            value: decoded,
            encoded_value,
            span: logical.raw_span(entry_start, cursor),
        });
    }
    Ok(())
}

fn is_key_byte(byte: u8) -> bool {
    byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wrapped_header_and_preserves_duplicates() {
        let raw = b"**ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz*************\r\n\
**PARASOLID ASCII PREAMBLE\r\n\
**PART1;APPL=Onshape;FRU=para\r\nsolid;USER=A^-B^;C^^D^nE;\r\n\
**PART2;SCH=SCH_3001168_30100;USFLD_SIZE=0;\r\n\
**PART3;VENDOR=one;VENDOR=two;\r\n\
**END_OF_HEADER*****************************************************\r\n\
T1 X1 Y0 ";

        let header = CommonHeader::parse(raw).unwrap();
        assert_eq!(header.value("FRU"), Some(b"parasolid".as_slice()));
        assert_eq!(header.value("USER"), Some(b"A B;C^D\nE".as_slice()));
        assert_eq!(
            header.values("VENDOR").collect::<Vec<_>>(),
            vec![b"one".as_slice(), b"two".as_slice()]
        );
        assert_eq!(&raw[header.body_offset..], b"T1 X1 Y0 ");
    }

    #[test]
    fn rejects_unknown_caret_escape() {
        let raw = b"preamble\n**PART1;APPL=bad^x;\n**PART2;SCH=S;\n**PART3;\n**END_OF_HEADER***\n";
        let error = CommonHeader::parse(raw).unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidEscape);
    }

    #[test]
    fn accepts_but_diagnoses_de_facto_unescaped_header_space() {
        let raw = b"preamble\n**PART1;FRU=Parasolid 30.1;\n**PART2;SCH=S;\n**PART3;\n**END_OF_HEADER***\n";
        let header = CommonHeader::parse(raw).unwrap();
        assert_eq!(header.value("FRU"), Some(b"Parasolid 30.1".as_slice()));
        assert_eq!(header.diagnostics.len(), 1);
        assert_eq!(
            header.diagnostics[0].kind,
            HeaderDiagnosticKind::UnescapedSpace
        );
    }
}
