//! The mark a sidecar keeps of the XMP beside its frame. Reading and
//! writing the XMP itself is greycard-edit-io's.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// What was last taken from the XMP beside a frame: which file it
/// was, how long it was, and a hash of its bytes.
///
/// The `.gcd` keeps this beside the meta so that a load can ask the
/// only question worth asking — *has the XMP changed since it was
/// last read?* — rather than comparing two files' modification times
/// and guessing. Times are the wrong instrument here: a `.gcd`
/// written for an exposure would then hide a later rating from
/// Lightroom, a rating cleared here would be undone by an untouched
/// XMP that happens to be newer, and a folder copied without `-p`
/// would have neither. A hash compares the XMP to itself, and
/// survives the copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adopted {
    /// The name, not the path: a folder that moves keeps its mark.
    pub file: String,
    pub len: u64,
    /// FNV-1a over the packet's bytes, in hex. Not a cryptographic
    /// question: nobody is trying to forge a sidecar.
    pub hash: String,
}

impl Adopted {
    /// The mark for the packet `bytes` read from or written to `path`.
    pub fn of(path: &Path, bytes: &[u8]) -> Adopted {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for b in bytes {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Adopted {
            file: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            len: bytes.len() as u64,
            hash: format!("{hash:016x}"),
        }
    }
}

/// What was last taken from a culling tool's `IMG.xmp` that a raw
/// shares with its JPEG: each field as the file said it, none where
/// it was silent. A field is taken again only when the file says
/// something else for it, so a tool that rewrites its file for a
/// field of its own (a tag, a color class) or for the label alone
/// does not put back a rating given here since.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Culled {
    /// The rating as the file gave it, `-1` for a reject.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<i8>,
    /// The label's name as the file gave it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}
