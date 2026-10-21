//! Evidence-driven native parsing primitives for Parasolid XT.
//!
//! This crate implements the common header, text physical stream,
//! schema-directed primitive reads, text prefix, embedded-schema edits, and the
//! raw node graph. Version-specific metadata and strongly typed node APIs live
//! in the separate generated `xt-schema` crate.

mod binary;
mod document;
mod error;
mod header;
pub mod schema;
mod span;
mod text;
pub mod typed;
pub mod write;

pub use binary::{
    BinaryEnvelope, BinaryKind, BinaryPrefix, ByteOrder, CharacterEncoding, DoubleEncoding,
    MachineEncoding,
};
pub use document::{
    BinaryXtDocument, FieldValue, NodeRecord, ScalarValue, SchemaUpdateKind, Terminator,
    UnresolvedPointer, XtDocument,
};
pub use error::{ErrorKind, Result, XtError};
pub use header::{CommonHeader, HeaderDiagnostic, HeaderDiagnosticKind, HeaderEntry, HeaderPart};
pub use span::Span;
pub use text::{TextCursor, TextEnvelope, TextPrefix, TextStream};
pub use typed::{
    AbstractClass, AnyNode, ExactNode, NodeIndex, TypedFields, TypedNode, XtBox, XtByte, XtChar,
    XtDouble, XtInteger, XtIntersectionPoint, XtInterval, XtLogical, XtPointer, XtShort, XtUnicode,
    XtUnsupported, XtVector,
};
