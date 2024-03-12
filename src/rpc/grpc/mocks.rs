use async_trait::async_trait;
use mockall::mock;

use super::{client, proto::*, Error};

mock! {
    pub NodeDiscoveryClient{
    }

    #[async_trait]
    impl client::GrpcClient for NodeDiscoveryClient {
        async fn find_node(&self, addr: &str, request: FindNodeRequest) -> Result<FindNodeResponse, Error>;
        async fn ping(&self, addr: &str, request: PingRequest) -> Result<PingResponse, Error>;
        async fn find_value(&self, addr: &str, request: FindValueRequest) -> Result<FindValueResponse, Error>;
    }
}
