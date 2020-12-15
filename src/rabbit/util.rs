/// Utility functions for the rabbit module
use super::RabbitBroker;
use futures::task::Poll;
use futures_util::stream::StreamExt;

/// Attempts to fetch an item from an established consumer, if it is available.
/// Returns a queue consumer to be used incrementally in conjucntion with rabbit::util::try_fetch_queue_item
///
/// # Arguments
///
/// * `consumer` - A lapin::Consumer of a Channel
///
/// # Examples
///
/// ```
/// let broker = match RabbitBroker::new(&addr).await {
/// Some(b) => b,
/// None => panic!("Could not establish rabbit connection"),
/// };
/// let consumer = broker.consume_queue('1234', QueueLabel::Sysinfo.as_str()).await;
/// let item = crate::rabbit::util::try_fetch_consumer_item(c).await;
/// if let Some(data) = item {
///     info!("{}", data);
/// }
///
/// ```
pub async fn try_fetch_consumer_item(consumer: &mut lapin::Consumer) -> Option<Vec<u8>> {
    let waker = futures::task::noop_waker_ref();
    let mut cx = std::task::Context::from_waker(waker);

    match consumer.poll_next_unpin(&mut cx) {
        Poll::Pending => None,
        Poll::Ready(None) => None,
        Poll::Ready(Some(delivery)) => {
            let (channel, delivery) = delivery.expect("error in consumer");
            let data = delivery.data;
            RabbitBroker::ack(&channel, delivery.delivery_tag).await;
            Some(data)
        }
    }
}
