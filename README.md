# Kademlia Library in Rust

Welcome to the Kademlia Library repository, a Rust-based project focused on implementing the fundamentals needed to build a Kademlia distributed hash table (DHT). Inspired by the foundational [Kademlia paper](https://pdos.csail.mit.edu/~petar/papers/maymounkov-kademlia-lncs.pdf).

## Overview

The Kademlia DHT is a peer-to-peer system that provides a framework for decentralized network construction. It's particularly noted for its efficiency in routing and data storage across a distributed network.

This library includes the core primitives necessary for the Kademlia DHT, such as the lookup algorithm. It's designed to be independent of the underlying network layer, facilitating adaptability and customization.

## Key Features

- **Core Primitives**: Asynchronous implementation of Kademlia's algorithms

- **Network Layer Abstraction**: Utilizes the `kademlia::Communicator` trait to abstract away the specifics of the network communication, allowing for flexible integration with different network protocols.

- **gRPC communicator**: Includes an example implementation of the `Communicator` trait using gRPC, demonstrating how to integrate the library with network services.

## Purpose

The primary goal of this repository is educational.

## License

This project is open source and available under MIT license.
