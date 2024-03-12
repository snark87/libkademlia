use log::error;
use tonic::async_trait;

use crate::rpc::{FindValueResult, NodeAvailabilityChecker};
use crate::{node, Communicator, Key, KeySizeParameters};

use super::proto::*;
use super::{client, Error};

type GrpcLink = String;
/// implements kademlia rpc communication using gRPC protocol
///
/// # Example
///
/// ```no_run
/// use tokio;
/// use kademlia::rpc::grpc::GrpcCommunicator;
/// use kademlia::{Communicator, DefaultKademliaParameters, Key, Node};
///
/// #[tokio::main]
/// async fn main() {
///     let node_id = Key::<DefaultKademliaParameters>::new();
///     let sender = Node::new(node_id, String::from("http://sender_grpc_address"));
///     let communicator = GrpcCommunicator::new(&sender);
///     let target_node_id = Key::new();
///     let response = communicator
///         .get_k_closest(&String::from("http://node_grpc_address"), &target_node_id)
///         .await
///         .unwrap();
/// }
///
/// ```
pub struct GrpcCommunicator<P: KeySizeParameters> {
    owner: node::Node<P, GrpcLink>,
    client: Box<dyn client::GrpcClient>,
}

impl<P: KeySizeParameters> GrpcCommunicator<P> {
    pub fn new(owner: &node::Node<P, GrpcLink>) -> Self {
        GrpcCommunicator {
            owner: owner.clone(),
            client: Box::new(client::GrpcClientImpl::new()),
        }
    }

    #[cfg(test)]
    pub fn with_client<C: client::GrpcClient + 'static>(
        owner: &node::Node<P, GrpcLink>,
        client: C,
    ) -> Self {
        GrpcCommunicator {
            owner: owner.clone(),
            client: Box::new(client),
        }
    }
}

#[async_trait]
impl<P: KeySizeParameters> NodeAvailabilityChecker for GrpcCommunicator<P> {
    type Link = String;
    type Error = Error;
    async fn ping(&self, link: &Self::Link) -> Result<(), Self::Error> {
        let result: Result<_, _> = {
            let _ = self
                .client
                .ping(
                    link,
                    PingRequest {
                        sender: Some(Node::from(&self.owner)),
                    },
                )
                .await?;

            Ok(())
        };

        result.inspect_err(|err| {
            error!("PING rpc to {:?} failed: {}", link, err);
        })
    }
}

#[async_trait]
impl<P: KeySizeParameters> Communicator<P> for GrpcCommunicator<P> {
    type Value = String;

    async fn get_k_closest(
        &self,
        link: &Self::Link,
        key: &Key<P>,
    ) -> Result<Vec<node::Node<P, Self::Link>>, Self::Error> {
        let result: Result<_, _> = {
            let response = self
                .client
                .find_node(
                    link,
                    FindNodeRequest {
                        sender: Some(Node::from(&self.owner)),
                        target_id: Some(NodeId::from(key)),
                    },
                )
                .await?;

            let nodes: Vec<Node> = response.discovered.map(|n| n.nodes).unwrap_or(vec![]);

            nodes
                .into_iter()
                .map(|node| node::Node::try_from(node))
                .collect()
        };

        result.inspect_err(|err| {
            error!("FIND_NODE rpc to {:?} failed: {}", link, err);
        })
    }

    async fn find_value(
        &self,
        link: &Self::Link,
        key: &Key<P>,
    ) -> Result<FindValueResult<P, Self::Link, Self::Value>, Self::Error> {
        let result: Result<_, _> = {
            let response = self
                .client
                .find_value(
                    link,
                    FindValueRequest {
                        sender: Some((&self.owner).into()),
                        key: Some(key.into()),
                    },
                )
                .await?;

            let result = response.result.ok_or(Error::MissingResult)?;
            match result {
                find_value_response::Result::Discovered(discovered) => {
                    let discovered_nodes: Result<Vec<_>, _> = discovered
                        .nodes
                        .into_iter()
                        .map(|n| node::Node::<P, Self::Link>::try_from(n))
                        .collect();
                    let discovered_nodes = discovered_nodes?;
                    Ok(FindValueResult::ClosestNodes(discovered_nodes))
                }
                find_value_response::Result::Value(value) => Ok(FindValueResult::FoundValue(value)),
            }
        };

        result.inspect_err(|err| {
            error!("FIND_VALUE rpc to {:?} failed: {}", link, err);
        })
    }

    async fn store_value(
        &self,
        link: &Self::Link,
        key: &Key<P>,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        let result = self
            .client
            .store_value(
                link,
                StoreValueRequest {
                    sender: Some((&self.owner).into()),
                    key: Some(key.into()),
                    value: value,
                },
            )
            .await
            .map(|_| ());

        result.inspect_err(|err| {
            error!("FIND_VALUE rpc to {:?} failed: {}", link, err);
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{key, DefaultKademliaParameters};

    use super::super::mocks::*;
    use super::*;

    #[tokio::test]
    async fn when_empty_ok_response() {
        // Arrange
        let mut client = MockNodeDiscoveryClient::new();
        client.expect_find_node().once().returning(|_, _| {
            Ok(FindNodeResponse {
                discovered: Some(Nodes { nodes: vec![] }),
            })
        });

        let node_id = Key::new();
        let owner = node::Node::new(node_id, String::from("owner_link"));
        let target_key = Key::new();
        let link = String::from("test_link");
        let communicator =
            GrpcCommunicator::<DefaultKademliaParameters>::with_client(&owner, client);

        // Act
        let nodes = communicator
            .get_k_closest(&link, &target_key)
            .await
            .unwrap();

        // Assert
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn when_empty_err_response() {
        // Arrange
        let mut client = MockNodeDiscoveryClient::new();
        client.expect_find_node().once().returning(|_, _| {
            Err(Error::GrpcStatusError {
                code: tonic::Code::Internal,
                message: String::from(""),
            })
        });

        let node_id = Key::new();
        let owner = node::Node::new(node_id, String::from("owner_link"));
        let target_key = Key::new();
        let link = String::from("test_link");
        let communicator =
            GrpcCommunicator::<DefaultKademliaParameters>::with_client(&owner, client);

        // Act
        let response = communicator.get_k_closest(&link, &target_key).await;

        // Assert
        assert!(response.is_err());
    }

    #[tokio::test]
    async fn get_k_closest_when_ok_valid_key() {
        // Arrange
        let mut client = MockNodeDiscoveryClient::new();
        let key = Key::<DefaultKademliaParameters>::new();
        let addr = String::from("test_address");
        let node = Node {
            id: Some(NodeId::from(&key)),
            address: addr.clone(),
        };
        client.expect_find_node().once().returning(move |_, _| {
            Ok(FindNodeResponse {
                discovered: Some(Nodes {
                    nodes: vec![node.clone()],
                }),
            })
        });

        let node_id = Key::new();
        let owner = node::Node::new(node_id, String::from("owner_link"));
        let target_key = Key::new();
        let link = String::from("test_link");
        let communicator =
            GrpcCommunicator::<DefaultKademliaParameters>::with_client(&owner, client);

        // Act
        let nodes = communicator
            .get_k_closest(&link, &target_key)
            .await
            .unwrap();

        // Assert
        let keys: Vec<_> = nodes.clone().into_iter().map(|n| n.node_id).collect();
        let links: Vec<_> = nodes.clone().into_iter().map(|n| n.link).collect();
        assert_eq!(vec![key], keys);
        assert_eq!(vec![addr], links);
    }

    #[tokio::test]
    async fn get_k_closest_when_ok_and_invalid_key_it_should_return_err() {
        // Arrange
        let mut client = MockNodeDiscoveryClient::new();
        let addr = String::from("test_address");
        let node = Node {
            id: Some(NodeId {
                id_base64: String::from("invalid_id"),
            }),
            address: addr.clone(),
        };
        client.expect_find_node().once().returning(move |_, _| {
            Ok(FindNodeResponse {
                discovered: Some(Nodes {
                    nodes: vec![node.clone()],
                }),
            })
        });

        let node_id = Key::new();
        let owner = node::Node::new(node_id, String::from("owner_link"));
        let target_key = Key::new();
        let link = String::from("test_link");
        let communicator =
            GrpcCommunicator::<DefaultKademliaParameters>::with_client(&owner, client);

        // Act
        let response = communicator.get_k_closest(&link, &target_key).await;

        // Assert
        assert!(response.is_err());
    }

    #[tokio::test]
    async fn get_k_closest_when_ok_and_no_key_it_should_return_err() {
        // Arrange
        let mut client = MockNodeDiscoveryClient::new();
        let addr = String::from("test_address");
        let node = Node {
            id: None,
            address: addr.clone(),
        };
        client.expect_find_node().once().returning(move |_, _| {
            Ok(FindNodeResponse {
                discovered: Some(Nodes {
                    nodes: vec![node.clone()],
                }),
            })
        });

        let node_id = Key::new();
        let owner = node::Node::new(node_id, String::from("owner_link"));
        let target_key = Key::new();
        let link = String::from("test_link");
        let communicator =
            GrpcCommunicator::<DefaultKademliaParameters>::with_client(&owner, client);

        // Act
        let response = communicator.get_k_closest(&link, &target_key).await;

        // Assert
        assert!(response.is_err());
    }

    #[tokio::test]
    async fn find_value_when_value_is_found_it_should_return_value() {
        // Arrange
        let mut client = MockNodeDiscoveryClient::new();
        let value = String::from("test value");
        let return_value = value.clone();
        client.expect_find_value().once().returning(move |_, _| {
            let return_value = return_value.clone();
            Ok(FindValueResponse {
                result: Some(find_value_response::Result::Value(return_value)),
            })
        });
        let node_id = Key::new();
        let owner = node::Node::new(node_id, String::from("owner_link"));
        let target_key = Key::new();
        let link = String::from("test_link");
        let communicator =
            GrpcCommunicator::<DefaultKademliaParameters>::with_client(&owner, client);

        // Act
        let response = communicator.find_value(&link, &target_key).await.unwrap();

        // Assert
        match response {
            FindValueResult::ClosestNodes(_) => panic!("it should not return closest nodes"),
            FindValueResult::FoundValue(actual_value) => assert_eq!(value, actual_value),
        }
    }

    #[tokio::test]
    async fn find_value_when_error_should_return_error() {
        // Arrange
        let mut client = MockNodeDiscoveryClient::new();
        client.expect_find_value().once().returning(move |_, _| {
            Err(Error::GrpcStatusError {
                code: tonic::Code::Internal,
                message: String::from("test error"),
            })
        });
        let node_id = Key::new();
        let owner = node::Node::new(node_id, String::from("owner_link"));
        let target_key = Key::new();
        let link = String::from("test_link");
        let communicator =
            GrpcCommunicator::<DefaultKademliaParameters>::with_client(&owner, client);

        // Act
        let response = communicator.find_value(&link, &target_key).await;

        // Assert
        assert!(response.is_err());
    }

    #[tokio::test]
    async fn find_value_when_empty_result_it_should_return_value() {
        // Arrange
        let mut client = MockNodeDiscoveryClient::new();
        client
            .expect_find_value()
            .once()
            .returning(move |_, _| Ok(FindValueResponse { result: None }));
        let node_id = Key::new();
        let owner = node::Node::new(node_id, String::from("owner_link"));
        let target_key = Key::new();
        let link = String::from("test_link");
        let communicator =
            GrpcCommunicator::<DefaultKademliaParameters>::with_client(&owner, client);

        // Act
        let response = communicator.find_value(&link, &target_key).await;

        // Assert
        assert!(response.is_err());
    }

    #[tokio::test]
    async fn find_value_when_value_not_found_it_should_return_nodes() {
        // Arrange
        let mut client = MockNodeDiscoveryClient::new();
        let nodes = vec![node::Node::<DefaultKademliaParameters, String>::new(
            key::Key::new(),
            String::from("test link"),
        )];
        let return_nodes = nodes.clone();
        client.expect_find_value().once().returning(move |_, _| {
            Ok(FindValueResponse {
                result: Some(find_value_response::Result::Discovered(Nodes {
                    nodes: return_nodes.iter().map(|n| n.into()).collect(),
                })),
            })
        });
        let node_id = Key::new();
        let owner = node::Node::new(node_id, String::from("owner_link"));
        let target_key = Key::new();
        let link = String::from("test_link");
        let communicator =
            GrpcCommunicator::<DefaultKademliaParameters>::with_client(&owner, client);

        // Act
        let response = communicator.find_value(&link, &target_key).await.unwrap();

        // Assert
        match response {
            FindValueResult::ClosestNodes(actual_nodes) => {
                assert_eq!(nodes, actual_nodes)
            }
            FindValueResult::FoundValue(_) => panic!("it should not return value"),
        }
    }
}
