mod client;
mod errors;
mod proto;
mod communicator;

#[cfg(test)]
pub mod mocks;

use thiserror::Error;

pub use errors::Error;
pub use communicator::GrpcCommunicator;

