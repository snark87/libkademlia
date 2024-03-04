use futures::Stream;
use tokio::sync::mpsc::{channel, Sender};
use tokio_stream::wrappers::ReceiverStream;

/// Creates a stream paired with a sender, backed by a channel with specified capacity.
///
/// This function initializes a stream that is backed by a channel of the given capacity, returning
/// a tuple containing the stream and a sender part of the channel. This setup allows for concurrent
/// production and asynchronous consumption of items of type `T`, leveraging Rust's concurrency
/// features efficiently.
///
/// The returned `Stream` yields items as they are sent through the `Sender<T>`. The channel's
/// capacity controls back-pressure, suspending sends when full until space is available again.
///
/// # Parameters
///
/// * `bound``: Capacity of the underlying channel. Determines the number of items the channel can
///   hold before blocking further sends. A size of 0 may indicate an unbounded channel, depending
///   on implementation, which requires careful handling to avoid excessive memory usage.
///
/// # Returns
///
/// A tuple of:
/// - An implementation of `Stream` yielding items of type `T`. The stream completes when the sender
///   is dropped and all items are consumed.
/// - A `Sender<T>` for sending items into the stream. Dropping the sender closes the stream.
pub fn create_channel_stream<T>(bound: usize) -> (impl Stream<Item = T>, Sender<T>) {
    let (tx, rx) = channel(bound);
    (ReceiverStream::new(rx), tx)
}

#[cfg(test)]
mod tests {
    use tokio_stream::StreamExt;

    use super::*;

    #[tokio::test]
    async fn create_channel_stream_should_receive() {
        let (stream, tx) = create_channel_stream::<i32>(2);
        let send_fut = async move {
            tx.send(42).await.unwrap();
            let _ = tx.send(43).await.unwrap();
            let _ = tx.send(44).await.unwrap();
            let _ = tx.send(45).await.unwrap();
        };
        let recv_fut = async move {
            let rcvd: Vec<i32> = stream.collect().await;
            assert_eq!(vec!(42, 43, 44, 45), rcvd);
        };
        tokio::join!(recv_fut, send_fut);
    }
}
