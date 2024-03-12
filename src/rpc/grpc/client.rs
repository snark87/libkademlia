use async_trait::async_trait;
use log::error;

use super::{
    proto::{self, *},
    Error,
};

#[async_trait]
pub trait GrpcClient: Sync + Send {
    async fn find_node(
        &self,
        addr: &str,
        request: FindNodeRequest,
    ) -> Result<FindNodeResponse, Error>;

    async fn ping(&self, addr: &str, request: PingRequest) -> Result<PingResponse, Error>;

    async fn find_value(
        &self,
        addr: &str,
        request: FindValueRequest,
    ) -> Result<FindValueResponse, Error>;
}

pub struct GrpcClientImpl {}

impl GrpcClientImpl {
    pub fn new() -> Self {
        GrpcClientImpl {}
    }
}

#[async_trait]
impl GrpcClient for GrpcClientImpl {
    async fn find_node(
        &self,
        addr: &str,
        request: FindNodeRequest,
    ) -> Result<FindNodeResponse, Error> {
        let mut client = proto::kademlia_operations_client::KademliaOperationsClient::connect(
            String::from(addr),
        )
        .await
        .map_err(|err| {
            error!("find node gRPC transport error: {}", err);
            Error::TransportError { error: err }
        })?;
        let result: tonic::Response<_> =
            client
                .find_node(request)
                .await
                .map_err(|status| Error::GrpcStatusError {
                    code: status.code(),
                    message: String::from(status.message()),
                })?;
        Ok(result.into_inner())
    }

    async fn ping(&self, addr: &str, request: PingRequest) -> Result<PingResponse, Error> {
        let mut client = proto::kademlia_operations_client::KademliaOperationsClient::connect(
            String::from(addr),
        )
        .await
        .map_err(|err| {
            error!("ping gRPC transport error: {}", err);
            Error::TransportError { error: err }
        })?;

        let result: tonic::Response<_> =
            client
                .ping(request)
                .await
                .map_err(|status| Error::GrpcStatusError {
                    code: status.code(),
                    message: String::from(status.message()),
                })?;

        Ok(result.into_inner())
    }

    async fn find_value(
        &self,
        addr: &str,
        request: FindValueRequest,
    ) -> Result<FindValueResponse, Error> {
        let mut client = proto::kademlia_operations_client::KademliaOperationsClient::connect(
            String::from(addr),
        )
        .await
        .map_err(|err| {
            error!("find_value gRPC transport error: {}", err);
            Error::TransportError { error: err }
        })?;

        let result: tonic::Response<_> =
            client
                .find_value(request)
                .await
                .map_err(|status| Error::GrpcStatusError {
                    code: status.code(),
                    message: String::from(status.message()),
                })?;

        Ok(result.into_inner())
    }
}
