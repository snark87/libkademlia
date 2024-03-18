use async_trait::async_trait;
use log::error;
use tonic::{transport::Channel, Response, Status};

use self::kademlia_operations_client::KademliaOperationsClient;

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

    async fn store_value(
        &self,
        addr: &str,
        request: StoreValueRequest,
    ) -> Result<StoreValueResponse, Error>;
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
        let mut client = connect(addr).await?;

        map_status_to_error(client.find_node(request).await)
    }

    async fn ping(&self, addr: &str, request: PingRequest) -> Result<PingResponse, Error> {
        let mut client = connect(addr).await?;

        map_status_to_error(client.ping(request).await)
    }

    async fn find_value(
        &self,
        addr: &str,
        request: FindValueRequest,
    ) -> Result<FindValueResponse, Error> {
        let mut client = connect(addr).await?;
        map_status_to_error(client.find_value(request).await)
    }

    async fn store_value(
        &self,
        addr: &str,
        request: StoreValueRequest,
    ) -> Result<StoreValueResponse, Error> {
        let mut client = connect(addr).await?;

        map_status_to_error(client.store_value(request).await)
    }
}

async fn connect(addr: &str) -> Result<KademliaOperationsClient<Channel>, Error> {
    proto::kademlia_operations_client::KademliaOperationsClient::connect(String::from(addr))
        .await
        .map_err(|err| {
            error!("gRPC transport error connecting to {}: {}", addr, err);
            Error::TransportError { error: err }
        })
}

fn map_status_to_error<T>(result: Result<Response<T>, Status>) -> Result<T, Error> {
    let result = result.map_err(|status| Error::GrpcStatusError {
        code: status.code(),
        message: String::from(status.message()),
    })?;

    Ok(result.into_inner())
}
