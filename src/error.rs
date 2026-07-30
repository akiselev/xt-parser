use std::fmt;

use serde::Serialize;

use crate::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    UnexpectedEof,
    MissingHeaderMarker,
    InvalidHeader,
    InvalidHeaderKey,
    InvalidHeaderValue,
    InvalidEscape,
    UnsupportedFormat,
    InvalidNumber,
    NumberOutOfRange,
    ExpectedDelimiter,
    InvalidLogical,
    InvalidNode,
    DuplicateNodeIndex,
    UnsupportedFieldType,
    TrailingData,
    InvalidSchema,
    InvalidSchemaEdit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct XtError {
    pub kind: ErrorKind,
    pub span: Span,
    pub context: String,
}

impl XtError {
    pub fn new(kind: ErrorKind, span: Span, context: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            context: context.into(),
        }
    }
}

impl fmt::Display for XtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} at bytes {}..{}: {}",
            self.kind, self.span.start, self.span.end, self.context
        )
    }
}

impl std::error::Error for XtError {}

pub type Result<T> = std::result::Result<T, XtError>;
