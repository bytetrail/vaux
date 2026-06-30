//! Round-trip stress test for QoS 1 publishes between two vaux clients.
//!
//! Reproduces (without any external broker) the scenario where a publisher
//! sends a run of medium-sized payloads (default 48K) to a subscriber and
//! the subscriber appears to stop sending PubAck partway through. A minimal
//! in-process relay stands in for the 3rd-party broker: it CONNACKs both
//! sides, SUBACKs the subscriber, immediately PubAcks the publisher (as a
//! real broker would once it has persisted the message), and forwards each
//! PUBLISH on to the subscriber. The relay then counts how many PubAcks come
//! back from the subscriber.
//!
//! Run: cargo run --example roundtrip -- --count 200
//!
//! By default the subscriber's packet consumer is never drained at all,
//! which models an application that has no consumer (the channel fills
//! permanently, so the client correctly stops reading). Pass `--drain` to
//! simulate an application that actively drains received packets; combine
//! with `--consumer-delay-ms` to model a slow consumer, `--channel-size` to
//! override the default packet_out capacity (128), and
//! `--drain-start-delay-ms` to model a consumer that is briefly stuck (e.g.
//! a busy downstream task) before catching up. These reproduce the class of
//! bug fixed in `MqttClient::start()`: a backed-up consumer used to block
//! the client's entire event loop (including keep-alive and outbound
//! publishes) rather than just pausing inbound reads.

use std::time::{Duration, Instant};

use clap::Parser;
use tokio::{
    net::TcpListener,
    select,
    sync::{mpsc, oneshot},
};
use vaux_async::stream::{AsyncMqttStream, MqttStream, PacketStream};
use vaux_client::{ClientBuilder, MqttConnection, PacketChannel};
use vaux_mqtt::{
    ConnAck, Packet, PacketType, Publish, QoSLevel, Reason, subscribe::SubAck,
};

const TOPIC: &str = "roundtrip/test";
const MAX_RELAY_BUFFER: usize = 256 * 1024;
const KEEP_ALIVE: Duration = Duration::from_secs(30);

#[derive(Parser, Debug)]
#[command(about = "Round-trip large payload stress test (no external broker required)")]
struct Args {
    /// number of QoS 1 messages to publish
    #[arg(long, default_value_t = 200)]
    count: usize,
    /// payload size in bytes
    #[arg(long, default_value_t = 49_152)]
    size: usize,
    /// seconds to wait for all PubAcks before declaring failure
    #[arg(long, default_value_t = 30)]
    timeout: u64,
    /// drain the subscriber's packet consumer (simulates an active consumer)
    #[arg(long)]
    drain: bool,
    /// override the subscriber's packet_out channel size (diagnostic)
    #[arg(long)]
    channel_size: Option<u16>,
    /// simulate a slow consumer by sleeping this many ms per received packet
    /// (only applies with --drain; models a UI thread that drains but lags)
    #[arg(long, default_value_t = 0)]
    consumer_delay_ms: u64,
    /// subscriber keep-alive interval in seconds (vaux-client enforces a
    /// 30s minimum)
    #[arg(long, default_value_t = 30)]
    keep_alive_secs: u64,
    /// delay before the drain task's first recv() call, in ms (simulates a
    /// consumer that is briefly stuck, e.g. a busy UI thread) before catching up
    #[arg(long, default_value_t = 0)]
    drain_start_delay_ms: u64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    println!(
        "roundtrip: count={} size={}B drain={}",
        args.count, args.size, args.drain
    );

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind relay");
    let port = listener.local_addr().expect("failed to get local addr").port();
    println!("relay listening on 127.0.0.1:{port}");

    let (ready_tx, ready_rx) = oneshot::channel::<()>();
    let count = args.count;
    let relay_handle = tokio::spawn(async move { run_relay(listener, count, ready_tx).await });

    let mut sub_builder = ClientBuilder::new(MqttConnection::new().with_host("127.0.0.1").with_port(port))
        .with_client_id("roundtrip-sub")
        .with_auto_ack(true)
        .with_auto_packet_id(true)
        .with_keep_alive(Duration::from_secs(args.keep_alive_secs))
        .with_max_connect_wait(Duration::from_secs(5))
        .with_max_packet_size(MAX_RELAY_BUFFER);
    if let Some(channel_size) = args.channel_size {
        sub_builder = sub_builder.with_channel_size(channel_size);
        println!("subscriber: channel_size override = {channel_size}");
    }
    let mut sub_client = sub_builder.build().await.expect("failed to build subscriber");

    let _sub_handle = sub_client
        .try_start(Duration::from_secs(5), true)
        .await
        .expect("subscriber failed to connect");

    if args.drain {
        let mut consumer = sub_client
            .take_packet_consumer()
            .expect("failed to take subscriber consumer");
        let delay = Duration::from_millis(args.consumer_delay_ms);
        let start_delay = Duration::from_millis(args.drain_start_delay_ms);
        tokio::spawn(async move {
            if !start_delay.is_zero() {
                tokio::time::sleep(start_delay).await;
            }
            while consumer.recv().await.is_some() {
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
            }
        });
        println!(
            "subscriber: draining packet consumer (start_delay={}ms, delay={}ms/packet, keep_alive={}s)",
            args.drain_start_delay_ms, args.consumer_delay_ms, args.keep_alive_secs
        );
    } else {
        println!("subscriber: NOT draining packet consumer (reproduces back-pressure)");
    }

    sub_client
        .subscribe(1, &[TOPIC], QoSLevel::AtLeastOnce)
        .await
        .expect("subscribe failed");

    ready_rx.await.expect("relay dropped before subscriber was ready");
    println!("subscriber subscribed, starting publisher");

    let mut puback_chan = PacketChannel::new_with_size(args.count + 16);
    let mut pub_client = ClientBuilder::new(MqttConnection::new().with_host("127.0.0.1").with_port(port))
        .with_client_id("roundtrip-pub")
        .with_auto_ack(true)
        .with_auto_packet_id(true)
        .with_keep_alive(KEEP_ALIVE)
        .with_max_connect_wait(Duration::from_secs(5))
        .with_filtered_consumer(PacketType::PubAck, puback_chan.sender())
        .build()
        .await
        .expect("failed to build publisher");

    let _pub_handle = pub_client
        .try_start(Duration::from_secs(5), true)
        .await
        .expect("publisher failed to connect");

    // drain anything that lands on the publisher's main consumer (e.g. SubAck leakage);
    // PubAck is routed to puback_chan above and intentionally left undrained but sized
    // to never block.
    let mut pub_consumer = pub_client
        .take_packet_consumer()
        .expect("failed to take publisher consumer");
    tokio::spawn(async move {
        while pub_consumer.recv().await.is_some() {}
    });
    let _puback_receiver = puback_chan.take_receiver();

    let payload = bytes::Bytes::from(vec![0xABu8; args.size]);
    let producer = pub_client.packet_producer();
    let publish_start = Instant::now();

    for i in 1..=args.count {
        let mut publish = Publish::default();
        publish.topic_name = TOPIC.to_string();
        publish.payload = Some(payload.clone());
        publish.set_qos(QoSLevel::AtLeastOnce);

        producer
            .send(Packet::Publish(publish))
            .await
            .expect("failed to queue publish");

        if i % 50 == 0 || i == args.count {
            println!("  queued {i}/{}", args.count);
        }
    }
    println!(
        "all {} publishes queued in {:.2?}",
        args.count,
        publish_start.elapsed()
    );

    match tokio::time::timeout(Duration::from_secs(args.timeout), relay_handle).await {
        Ok(Ok(Ok((received, elapsed)))) if received == args.count => {
            println!("PASS: {received}/{} PubAcks received in {elapsed:.2?}", args.count);
        }
        Ok(Ok(Ok((received, _)))) => {
            println!("FAIL: only {received}/{} PubAcks received", args.count);
            std::process::exit(1);
        }
        Ok(Ok(Err(e))) => {
            println!("FAIL: relay error: {e}");
            std::process::exit(1);
        }
        Ok(Err(e)) => {
            println!("FAIL: relay task panicked: {e}");
            std::process::exit(1);
        }
        Err(_) => {
            println!(
                "FAIL: timed out after {}s waiting for PubAcks (subscriber likely stalled)",
                args.timeout
            );
            std::process::exit(1);
        }
    }

    let _ = pub_client.stop().await;
    let _ = sub_client.stop().await;
}

/// Minimal relay that stands in for a 3rd-party broker. Accepts the
/// subscriber connection first, then the publisher connection, then forwards
/// publishes from publisher to subscriber and reports how many PubAcks come
/// back from the subscriber.
async fn run_relay(
    listener: TcpListener,
    expected: usize,
    ready_tx: oneshot::Sender<()>,
) -> Result<(usize, Duration), String> {
    let (sub_tcp, _) = listener.accept().await.map_err(|e| e.to_string())?;
    let mut sub_stream = PacketStream::new(
        AsyncMqttStream(MqttStream::TcpStream(sub_tcp)),
        None,
        Some(MAX_RELAY_BUFFER),
    );

    match sub_stream.read().await {
        Ok(Some(Packet::Connect(_))) => {
            let mut connack = Packet::ConnAck(ConnAck::default());
            sub_stream.write(&mut connack).await.map_err(|e| e.to_string())?;
        }
        other => return Err(format!("expected CONNECT from subscriber, got {other:?}")),
    }

    let subscribe_packet_id = match sub_stream.read().await {
        Ok(Some(Packet::Subscribe(sub))) => sub.packet_id,
        other => return Err(format!("expected SUBSCRIBE from subscriber, got {other:?}")),
    };
    let mut suback = SubAck::new_with_packet_id(subscribe_packet_id).map_err(|e| e.to_string())?;
    suback.reason_codes.push(Reason::Success);
    sub_stream
        .write(&mut Packet::SubAck(suback))
        .await
        .map_err(|e| e.to_string())?;

    let _ = ready_tx.send(());

    let (pub_tcp, _) = listener.accept().await.map_err(|e| e.to_string())?;
    let mut pub_stream = PacketStream::new(
        AsyncMqttStream(MqttStream::TcpStream(pub_tcp)),
        None,
        Some(MAX_RELAY_BUFFER),
    );

    match pub_stream.read().await {
        Ok(Some(Packet::Connect(_))) => {
            let mut connack = Packet::ConnAck(ConnAck::default());
            pub_stream.write(&mut connack).await.map_err(|e| e.to_string())?;
        }
        other => return Err(format!("expected CONNECT from publisher, got {other:?}")),
    }

    let (forward_tx, mut forward_rx) = mpsc::channel::<Publish>(expected + 16);

    let pub_relay = tokio::spawn(async move {
        loop {
            match pub_stream.read().await {
                Ok(Some(Packet::Publish(publish))) => {
                    if let Some(packet_id) = publish.packet_id() {
                        let ack = vaux_mqtt::PubAck::new_puback_with_packet_id(packet_id);
                        if pub_stream
                            .write(&mut Packet::PubAck(ack))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    if forward_tx.send(publish).await.is_err() {
                        break;
                    }
                }
                Ok(Some(Packet::PingRequest(_))) => {
                    if pub_stream
                        .write(&mut Packet::PingResponse(Default::default()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Some(Packet::Disconnect(_))) | Ok(None) | Err(_) => break,
                Ok(Some(_)) => {}
            }
        }
    });

    let start = Instant::now();
    let mut received = 0usize;
    let mut next_id: u16 = 1;

    while received < expected {
        select! {
            result = sub_stream.read() => {
                match result {
                    Ok(Some(Packet::PubAck(_))) => {
                        received += 1;
                        if received % 25 == 0 || received == expected {
                            eprintln!("  relay: {received}/{expected} PubAcks");
                        }
                    }
                    Ok(Some(Packet::PingRequest(_))) => {
                        eprintln!("  relay: PINGREQ from subscriber at {received}/{expected} ACKs (elapsed {:.2?})", start.elapsed());
                        if sub_stream.write(&mut Packet::PingResponse(Default::default())).await.is_err() {
                            return Err(format!("failed to write PINGRESP after {received}/{expected} ACKs"));
                        }
                    }
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        return Err(format!("subscriber disconnected after {received}/{expected} ACKs"));
                    }
                    Err(e) => {
                        return Err(format!("subscriber read error after {received}/{expected} ACKs: {e}"));
                    }
                }
            }
            maybe_publish = forward_rx.recv() => {
                match maybe_publish {
                    Some(mut publish) => {
                        let _ = publish.set_packet_id(Some(next_id));
                        next_id = next_id.wrapping_add(1);
                        if next_id == 0 {
                            next_id = 1;
                        }
                        if sub_stream.write(&mut Packet::Publish(publish)).await.is_err() {
                            return Err(format!("failed to forward publish after {received}/{expected} ACKs"));
                        }
                    }
                    None => {
                        return Err(format!("publisher relay closed after {received}/{expected} ACKs"));
                    }
                }
            }
        }
    }

    let elapsed = start.elapsed();
    pub_relay.abort();
    Ok((received, elapsed))
}
