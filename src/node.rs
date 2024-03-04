use crate::{key::Key, params::KeySizeParameters};


#[derive(Debug, Clone, PartialEq)]
pub struct Node<Params: KeySizeParameters, Link: Clone> {
    pub node_id: Key<Params>,
    pub link: Link
}



impl<P : KeySizeParameters, Link: Clone> Node<P, Link> {
    pub fn new(id: Key<P>, link: Link) -> Self {
        Node{
            node_id: id,
            link,
        }
    }
}
