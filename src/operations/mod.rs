use async_trait::async_trait;

use crate::{KademliaParameters, Key, Node};

#[async_trait]
pub trait KademliaOperations<P: KademliaParameters>: Send + Sync {
    type Link: Clone;

    async fn find_node(
        &self,
        sender: &Node<P, Self::Link>,
        id: &Key<P>,
    ) -> Vec<Node<P, Self::Link>>;
}
