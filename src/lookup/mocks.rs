use async_trait::async_trait;
use mockall::mock;
use thiserror::Error;

use crate::{key::Key, node::Node, params::DefaultKademliaParameters, rpc::FindValueResult};

#[derive(Debug, Clone, PartialEq)]
pub enum TestLink {
    Link1,
    Link2,
    Link3,
    Link4,
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum TestError {
    #[error("test error")]
    TestError,
}

pub type TestKey = Key<DefaultKademliaParameters>;
pub type TestNode = Node<DefaultKademliaParameters, TestLink>;

mock! {
    pub Communicator {
    }
    #[async_trait]
    impl crate::rpc::NodeAvailabilityChecker for Communicator {
        type Link = TestLink;
        type Error = TestError;

        async fn ping(&self, link: &TestLink) -> Result<(), TestError>;
    }

    #[async_trait]
    impl crate::rpc::Communicator<DefaultKademliaParameters> for Communicator {
        type Value = String;
        async fn get_k_closest(&self, link: &TestLink, key: &TestKey) -> Result<Vec<TestNode>, TestError>;
        async fn find_value(&self, link: &TestLink, key: &TestKey) -> Result<FindValueResult<DefaultKademliaParameters, TestLink, String>, TestError>;
        async fn store_value(&self, link: &TestLink, key: &TestKey, value: String) -> Result<(), TestError>;
    }
}

mock! {
    pub RoutingTable {}

    #[async_trait]
    impl crate::routing::RoutingTable<DefaultKademliaParameters> for RoutingTable {
        type Communicator = MockCommunicator;
        type Link = TestLink;

        async fn store(&self, communicator: &MockCommunicator, node: Node<DefaultKademliaParameters, TestLink>);
        async fn find_closest_nodes(&self, count: usize, key: &Key<DefaultKademliaParameters>) -> Vec<Node<DefaultKademliaParameters, TestLink>>;
        async fn find_by_id(&self, id: &Key<DefaultKademliaParameters>) -> Option<Node<DefaultKademliaParameters, TestLink>>;
    }
}
