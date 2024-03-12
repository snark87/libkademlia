use kademlia::{
    routing::SimpleRoutingTable,
    rpc::grpc::{GrpcCommunicator, KademliaGrpcInterface},
    storage::HashMapKVS,
    DefaultKademliaParameters, KademliaNodeInstance, Key, Node,
};
use log::error;

#[tokio::main]
async fn main() {
    // create node descriptor (id and link)
    let owner_id = Key::<DefaultKademliaParameters>::new();
    let owner = Node::new(owner_id.clone(), String::from("http://localhost:8080"));

    // create instance of communicator to reach other nodes
    let communicator = GrpcCommunicator::new(&owner);

    // create grpc interface for the node
    let node_interface = KademliaGrpcInterface::new();

    // create in-memory storage
    let storage = HashMapKVS::<_, String>::new();

    // create routing table
    let routing_table = SimpleRoutingTable::new(&owner_id);

    // finally, create node instance
    let node_instance =
        KademliaNodeInstance::new(node_interface, communicator, storage, routing_table);

    let err = node_instance.start().await;
    let _ = err.inspect_err(|err| error!("error running instance: {}", err));
}
