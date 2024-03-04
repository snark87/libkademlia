use base64::{engine::general_purpose::STANDARD as base64_std, Engine};
use generic_array::GenericArray;

use crate::{key, node, KeySizeParameters};
use super::Error;

tonic::include_proto!("kademlia");

impl<P: KeySizeParameters> From<&key::Key<P>> for NodeId {
    fn from(value: &key::Key<P>) -> Self {
        Self {
            id_base64: base64_std.encode(value),
        }
    }
}

impl<P: KeySizeParameters> From<&key::Key<P>> for Key {
    fn from(value: &key::Key<P>) -> Self {
        Self {
            base64: base64_std.encode(value),
        }
    }
}

impl<P: KeySizeParameters> TryFrom<&NodeId> for key::Key<P> {
    type Error = Error;

    fn try_from(value: &NodeId) -> Result<Self, Self::Error> {
        let bytes = base64_std
            .decode(&value.id_base64)
            .map_err(|err| Error::NodeIdDecodeError { error: err })?;
        let actual_size = bytes.len();
        let decoded_array = GenericArray::try_from_iter(bytes.into_iter()).map_err(|_|
        Error::InvalidKeySize { actual_size: actual_size })?;

        Ok(Self(decoded_array))
    }
}

impl<P: KeySizeParameters> From<&node::Node<P, String>> for Node {
    fn from(value: &node::Node<P, String>) -> Node {
        Self { id: Some((&value.node_id).into()), address: value.link.clone() }
    }
}

impl<P: KeySizeParameters> TryFrom<Node> for node::Node<P, String> {
    type Error = Error;

    fn try_from(value: Node) -> Result<Self, Self::Error> {
        let link = value.address;
        let node_id = value
            .id
            .map(|id| key::Key::try_from(&id))
            .unwrap_or(Err(Error::MissingNodeId))?;
        Ok(Self {
            node_id: node_id,
            link: link,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{DefaultKademliaParameters, Key};

    use tokio;
    use super::*;

    #[tokio::test]
    async fn node_id_when_convert_to_base64_and_back_should_stay_the_same() {
        let key = Key::<DefaultKademliaParameters>::new();
        let node_id = NodeId::from(&key);
        let converted_key = Key::<DefaultKademliaParameters>::try_from(&node_id).unwrap();
        assert_eq!(key, converted_key);
    }
}
