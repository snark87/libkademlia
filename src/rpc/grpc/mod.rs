mod client;
mod communicator;
mod errors;
mod proto;
mod server;

#[cfg(test)]
pub mod mocks;

use thiserror::Error;

pub use communicator::GrpcCommunicator;
pub use errors::Error;
pub use server::KademliaGrpcInterface;
pub use server::KademliaGrpcInterfaceFactory;
