use super::*;
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const TEST_DIR_BASE: &str = "tmp/tor_openchannel/";
const TEST_ONION_ADDR: &str = "2gzyxa5ihm7j63nqf6v5q4t5m7xuiwz5c46b4wrs4n5b7fe6f4n6cjad.onion";

#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn connect_via_tor_and_open_channel() {
    initialize();

    let proxy_addr = start_mock_socks5(TEST_ONION_ADDR, NODE2_PEER_PORT).await;

    let test_dir_base = format!("{TEST_DIR_BASE}basic/");
    let test_dir_node1 = format!("{test_dir_base}node1");
    let test_dir_node2 = format!("{test_dir_base}node2");

    let node1_args = UserArgs {
        tor_proxy: Some(proxy_addr),
        tor_skip_proxy_for_clearnet_targets: true,
        ..Default::default()
    };
    let (node1_addr, _) =
        start_node_with_args(&test_dir_node1, NODE1_PEER_PORT, false, node1_args).await;
    let (node2_addr, _) = start_node(&test_dir_node2, NODE2_PEER_PORT, false).await;

    fund_and_create_utxos(node1_addr, None).await;
    fund_and_create_utxos(node2_addr, None).await;

    let node2_pubkey = node_info(node2_addr).await.pubkey;

    connect_peer(
        node1_addr,
        &node2_pubkey,
        &format!("{TEST_ONION_ADDR}:{NODE2_PEER_PORT}"),
    )
    .await;
    assert_eq!(list_peers(node1_addr).await.len(), 1);

    let _channel = open_channel(node1_addr, &node2_pubkey, None, None, None, None, None).await;

    let channels_1 = list_channels(node1_addr).await;
    let channels_2 = list_channels(node2_addr).await;
    assert_eq!(channels_1.len(), 1);
    assert_eq!(channels_2.len(), 1);
}

async fn start_mock_socks5(expected_host: &'static str, expected_port: u16) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                handle_socks5_connection(stream, expected_host, expected_port)
                    .await
                    .unwrap();
            });
        }
    });

    proxy_addr
}

async fn handle_socks5_connection(
    mut inbound: TcpStream,
    expected_host: &str,
    expected_port: u16,
) -> std::io::Result<()> {
    let version = inbound.read_u8().await?;
    assert_eq!(version, 0x05);
    let nmethods = inbound.read_u8().await?;
    let mut methods = vec![0; nmethods as usize];
    inbound.read_exact(&mut methods).await?;
    assert!(methods.contains(&0x00));
    inbound.write_all(&[0x05, 0x00]).await?;

    let version = inbound.read_u8().await?;
    assert_eq!(version, 0x05);
    let cmd = inbound.read_u8().await?;
    assert_eq!(cmd, 0x01);
    let _reserved = inbound.read_u8().await?;
    let atyp = inbound.read_u8().await?;

    let (host, port) = match atyp {
        0x01 => {
            let mut addr = [0; 4];
            inbound.read_exact(&mut addr).await?;
            let port = inbound.read_u16().await?;
            (std::net::Ipv4Addr::from(addr).to_string(), port)
        }
        0x03 => {
            let len = inbound.read_u8().await? as usize;
            let mut host = vec![0; len];
            inbound.read_exact(&mut host).await?;
            let port = inbound.read_u16().await?;
            (String::from_utf8(host).unwrap(), port)
        }
        0x04 => {
            let mut addr = [0; 16];
            inbound.read_exact(&mut addr).await?;
            let port = inbound.read_u16().await?;
            (std::net::Ipv6Addr::from(addr).to_string(), port)
        }
        _ => panic!("unsupported SOCKS5 address type {atyp}"),
    };

    assert_eq!(host, expected_host);
    assert_eq!(port, expected_port);

    let mut outbound = TcpStream::connect(("127.0.0.1", expected_port)).await?;
    inbound
        .write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0, 0])
        .await?;

    copy_bidirectional(&mut inbound, &mut outbound).await?;
    Ok(())
}
