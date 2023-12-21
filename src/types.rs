use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct GenericResponse {
    pub status: u16,
    pub message: String,
}