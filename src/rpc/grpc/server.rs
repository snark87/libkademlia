use log::info;
use std::marker::PhantomData;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;

use crate::{
    key, node, operations,
    rpc::{
        grpc::server::kademlia_operations_server::KademliaOperationsServer, KademliaServer,
        KademliaServerFactory,
    },
    KademliaParameters,
};

use self::kademlia_operations_server::KademliaOperations;

use super::{proto::*, Error};

pub struct KademliaGrpcInterfaceFactory<P: KademliaParameters> {
    _phantom_p: PhantomData<P>,
    listen_addr: String,
}

pub struct KademliaGrpcInterface<P: KademliaParameters> {
    operations: Box<dyn operations::KademliaOperations<P, Link = String>>,
    listen_addr: String,
}

impl<P: KademliaParameters> KademliaServerFactory<P> for KademliaGrpcInterfaceFactory<P> {
    type Link = String;
    type Instance = KademliaGrpcInterface<P>;

    fn new_server(
        &self,
        operations: impl operations::KademliaOperations<P, Link = String> + 'static,
    ) -> Self::Instance {
        KademliaGrpcInterface::new(self.listen_addr.clone(), operations)
    }
}

impl<P: KademliaParameters> KademliaGrpcInterfaceFactory<P> {
    pub fn new(listen_addr: String) -> Self {
        Self {
            _phantom_p: PhantomData,
            listen_addr: listen_addr,
        }
    }
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

    async fn start_server(self, cancel: CancellationToken) -> Result<(), Self::Error> {
        let addr = self
            .listen_addr
            .parse()
            .map_err(|err| Error::AddrParseError { error: err })?;
        let server = KademliaOperationsServer::new(self);
        info!("gRPC server listens to {}", addr);
        let result = Server::builder()
            .add_service(server)
            .serve_with_shutdown(addr, cancel.cancelled())
            .await;
        result.map_err(|err| Error::TransportError { error: err })
    }

    fn get_link(&self) -> &Self::Link {
        todo!()
    }
}

impl<P: KademliaParameters> KademliaGrpcInterface<P> {
    pub fn new(
        listen_addr: String,
        operations: impl operations::KademliaOperations<P, Link = String> + 'static,
    ) -> Self {
        Self {
            operations: Box::new(operations),
            listen_addr: listen_addr,
        }
    }
}
