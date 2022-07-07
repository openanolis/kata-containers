use crate::network;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ResourceState {
    pub endpoint: Vec<network::EndpointState>,
}
#[derive(Serialize, Deserialize)]
pub struct SandboxState {
    pub resource: Option<ResourceState>,
}
