use std::{marker::PhantomData, sync::Arc};

use async_trait::async_trait;

use crate::{routing::RoutingTable, KademliaParameters, Key, Node};

#[async_trait]
pub trait KademliaOperations<P: KademliaParameters>: Send + Sync {
    type Link: Clone;

    async fn find_node(
        &self,
        sender: &Node<P, Self::Link>,
        id: &Key<P>,
    ) -> Vec<Node<P, Self::Link>>;
}

pub struct KademliaOperationsHandler<
    P: KademliaParameters,
    Link: Clone,
    RT: RoutingTable<P, Link = Link>,
> {
    routing_table: Arc<RT>,
    _phantom_p: PhantomData<P>,
    _phantom_link: PhantomData<Link>,
}

impl<P: KademliaParameters, Link: Clone, RT: RoutingTable<P, Link = Link>>
    KademliaOperationsHandler<P, Link, RT>
{
    pub fn new(routing_table: Arc<RT>) -> Self {
        Self {
            routing_table: routing_table,
            _phantom_p: PhantomData,
            _phantom_link: PhantomData,
        }
    }
}

#[async_trait]
impl<
        P: KademliaParameters + Send + Sync,
        Link: Clone + Send + Sync,
        RT: RoutingTable<P, Link = Link> + Sync + Send,
    > KademliaOperations<P> for KademliaOperationsHandler<P, Link, RT>
{
    type Link = Link;

    async fn find_node(
        &self,
        sender: &Node<P, Self::Link>,
        id: &Key<P>,
    ) -> Vec<Node<P, Self::Link>> {
        let rt = self.routing_table.as_ref();
        rt.find_closest_nodes(P::K_PARAM as usize, id).await
    }
}
