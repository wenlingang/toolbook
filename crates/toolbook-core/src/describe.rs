//! What a note takes and gives, for `toolbook describe` and skills.

use crate::doc::Document;

/// An input a note takes: a `param` block or a `secret` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputSpec {
    /// The block's name.
    pub name: String,
    /// The block's tool.
    pub tool: String,
    /// The block's content (empty for a secret).
    pub default: String,
    /// Whether it is a secret.
    pub secret: bool,
}

/// What a note takes and gives.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NoteSpec {
    /// The note's first paragraph.
    pub description: Option<String>,
    /// Its inputs, in order.
    pub inputs: Vec<InputSpec>,
    /// Its named blocks, in order: what `--output` can pick.
    pub outputs: Vec<String>,
}

/// Describes a note.
pub fn describe(doc: &Document) -> NoteSpec {
    let mut spec = NoteSpec { description: doc.description(), ..NoteSpec::default() };
    for (_, b) in doc.blocks() {
        let Some(name) = &b.info.name else { continue };
        let secret = b.info.tool == "secret";
        if secret || b.info.has_flag("param") {
            spec.inputs.push(InputSpec {
                name: name.clone(),
                tool: b.info.tool.clone(),
                default: if secret { String::new() } else { b.content.clone() },
                secret,
            });
        }
        spec.outputs.push(name.clone());
    }
    spec
}
