use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("node_id field is missing")]
    MissingNodeId,
    #[error("result field is missing")]
    MissingResult,
    #[error("key size {actual_size:?} is invalid")]
    InvalidKeySize { actual_size: usize },
    #[error("error decoding node id: {error:?}")]
    NodeIdDecodeError { error: base64::DecodeError },
    #[error("transport error: {error}")]
    TransportError{ error: tonic::transport::Error },
    #[error("gRPC response with code {code:?}: {message:?}")]
    GrpcStatusError{ code: tonic::Code, message: String },
}
