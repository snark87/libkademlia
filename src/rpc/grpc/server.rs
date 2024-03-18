use crate::{key, node, operations, rpc::KademliaServer, KademliaParameters};

use self::kademlia_operations_server::KademliaOperations;

use super::{proto::*, Error};

pub struct KademliaGrpcInterface<P: KademliaParameters> {
    operations: Box<dyn operations::KademliaOperations<P, Link = String>>,
}

#[tonic::async_trait]
impl<P: KademliaParameters + 'static> KademliaOperations for KademliaGrpcInterface<P> {
    async fn find_node(
        &self,
        request: tonic::Request<FindNodeRequest>,
    ) -> std::result::Result<tonic::Response<FindNodeResponse>, tonic::Status> {
        let sender_from_req = request
            .get_ref()
            .sender
            .clone()
            .ok_or(tonic::Status::invalid_argument("missing sender"))?;
        let sender: Result<node::Node<P, _>, _> = sender_from_req.try_into();
        let sender = sender.map_err(|err| tonic::Status::from_error(Box::new(err)))?;

        let key_from_req = request
            .get_ref()
            .target_id
            .clone()
            .ok_or(tonic::Status::invalid_argument("missing target_id"))?;
        let key: Result<key::Key<P>, _> = (&key_from_req).try_into();
        let key = key.map_err(|err| tonic::Status::from_error(Box::new(err)))?;
        let discovered_nodes: Vec<Node> = self
            .operations
            .find_node(&sender, &key)
            .await
            .iter()
            .map(|n| n.into())
            .collect();

        Ok(tonic::Response::new(FindNodeResponse {
            discovered: Some(Nodes {
                nodes: discovered_nodes,
            }),
        }))
    }
    async fn ping(
        &self,
        request: tonic::Request<PingRequest>,
    ) -> std::result::Result<tonic::Response<PingResponse>, tonic::Status> {
        todo!();
    }
    async fn find_value(
        &self,
        request: tonic::Request<FindValueRequest>,
    ) -> std::result::Result<tonic::Response<FindValueResponse>, tonic::Status> {
        todo!();
    }
    async fn store_value(
        &self,
        request: tonic::Request<StoreValueRequest>,
    ) -> std::result::Result<tonic::Response<StoreValueResponse>, tonic::Status> {
        todo!();
    }
}

#[tonic::async_trait]
impl<P: KademliaParameters> KademliaServer<P> for KademliaGrpcInterface<P> {
    type Error = Error;
    type Link = String;

    fn with_operation_handler(
        self,
        handler: impl operations::KademliaOperations<P, Link = String>,
    ) -> Self {
        todo!()
    }

    async fn start_server(&self) -> Result<(), Self::Error> {
        todo!()
    }

    fn get_link(&self) -> &Self::Link {
        todo!()
    }
}

impl<P: KademliaParameters> KademliaGrpcInterface<P> {
    pub fn new() -> Self {
        todo!();
    }
}
