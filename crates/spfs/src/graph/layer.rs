use crate::encoding;
use crate::Result;

#[cfg(test)]
#[path = "./layer_test.rs"]
mod layer_test;

/// Layers represent a logical collection of software artifacts.
///
/// Layers are considered completely immutable, and are
/// uniquely identifyable by the computed hash of all
/// relevant file and metadata.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Layer {
    pub manifest: encoding::Digest,
}

impl Layer {
    pub fn new(manifest: encoding::Digest) -> Self {
        Layer { manifest: manifest }
    }

    /// Return the child object of this one in the object DG.
    pub fn child_objects(&self) -> Vec<encoding::Digest> {
        vec![self.manifest]
    }
}

impl encoding::Encodable for Layer {
    fn encode(&self, writer: &mut impl std::io::Write) -> Result<()> {
        encoding::write_digest(writer, &self.manifest)
    }
}

impl encoding::Decodable for Layer {
    fn decode(reader: &mut impl std::io::Read) -> Result<Self> {
        Ok(Layer {
            manifest: encoding::read_digest(reader)?,
        })
    }
}
