use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct DrumMapEntry {
    pub pitch: u8,
    pub name: String,
    pub head: Option<String>,
    pub line: Option<i8>,
    pub voice: Option<u8>,
    pub stem: Option<i8>,
    pub shortcut: Option<String>,
}
