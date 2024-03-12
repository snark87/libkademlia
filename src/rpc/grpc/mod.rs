mod client;
mod communicator;
mod errors;
mod proto;

#[cfg(test)]
pub mod mocks;

use thiserror::Error;

pub use communicator::GrpcCommunicator;
pub use errors::Error;
