use super::*;

const TEST_DIR_BASE: &str = "tmp/tor_connection/";
const NODE1_TOR_PORT: u16 = 9881;
const NODE2_TOR_PORT: u16 = 9882;

/// Test that two nodes with Tor enabled can connect and communicate
#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn tor_enabled_basic_connection() {
    initialize();

    let test_dir_base = format!("{TEST_DIR_BASE}tor_enabled_basic/");
    let test_dir_node1 = format!("{test_dir_base}node1");
    let test_dir_node2 = format!("{test_dir_base}node2");

    // Start both nodes with Tor enabled
    let node1_addr = start_tor_daemon(&test_dir_node1, NODE1_TOR_PORT, true).await;
    let node2_addr = start_tor_daemon(&test_dir_node2, NODE2_TOR_PORT, true).await;

    // Initialize both nodes
    let password1 = format!("{test_dir_node1}.{NODE1_TOR_PORT}");
    let password2 = format!("{test_dir_node2}.{NODE2_TOR_PORT}");

    init_and_unlock_node(node1_addr, &password1).await;
    init_and_unlock_node(node2_addr, &password2).await;

    // Get node1's pubkey
    let node1_info = node_info(node1_addr).await;
    let node1_pubkey = node1_info.pubkey;

    // Connect node2 to node1 (Tor will be used automatically)
    connect_peer(node2_addr, &node1_pubkey, &format!("127.0.0.1:{NODE1_TOR_PORT}")).await;

    // Verify connection
    let peers = list_peers(node2_addr).await;
    assert_eq!(peers.len(), 1);
    assert!(peers.iter().any(|p| p.pubkey == node1_pubkey));

    // Verify from node1's perspective
    let peers = list_peers(node1_addr).await;
    assert_eq!(peers.len(), 1);

    println!("✓ Tor-enabled nodes connected successfully");
}

/// Test opening a channel between two Tor-enabled nodes
#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn tor_enabled_channel_open() {
    initialize();

    let test_dir_base = format!("{TEST_DIR_BASE}tor_channel/");
    let test_dir_node1 = format!("{test_dir_base}node1");
    let test_dir_node2 = format!("{test_dir_base}node2");

    // Start both nodes with Tor enabled
    let node1_addr = start_tor_daemon(&test_dir_node1, NODE1_TOR_PORT, true).await;
    let node2_addr = start_tor_daemon(&test_dir_node2, NODE2_TOR_PORT, true).await;

    let password1 = format!("{test_dir_node1}.{NODE1_TOR_PORT}");
    let password2 = format!("{test_dir_node2}.{NODE2_TOR_PORT}");

    init_and_unlock_node(node1_addr, &password1).await;
    init_and_unlock_node(node2_addr, &password2).await;

    // Fund both nodes
    fund_and_create_utxos(node1_addr, None).await;
    fund_and_create_utxos(node2_addr, None).await;

    // Get node2's info
    let node2_info = node_info(node2_addr).await;
    let node2_pubkey = node2_info.pubkey;

    // Open channel from node1 to node2 over Tor
    let channel = open_channel(
        node1_addr,
        &node2_pubkey,
        Some(NODE2_TOR_PORT),
        None,
        Some(1000000),
        None,
        None,
    )
    .await;

    // Verify channel is open
    let channels_1 = list_channels(node1_addr).await;
    let channels_2 = list_channels(node2_addr).await;

    assert_eq!(channels_1.len(), 1);
    assert_eq!(channels_2.len(), 1);
    assert_eq!(channels_1[0].channel_id, channel.channel_id);
    assert_eq!(channels_2[0].channel_id, channel.channel_id);
    assert!(channels_1[0].is_usable);
    assert!(channels_2[0].is_usable);

    println!("✓ Channel opened successfully over Tor");
}

/// Test that nodes without Tor can still connect normally
#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn tor_disabled_connection() {
    initialize();

    let test_dir_base = format!("{TEST_DIR_BASE}tor_disabled/");
    let test_dir_node1 = format!("{test_dir_base}node1");
    let test_dir_node2 = format!("{test_dir_base}node2");

    // Start both nodes with Tor DISABLED (default behavior)
    let node1_addr = start_tor_daemon(&test_dir_node1, NODE1_TOR_PORT, false).await;
    let node2_addr = start_tor_daemon(&test_dir_node2, NODE2_TOR_PORT, false).await;

    let password1 = format!("{test_dir_node1}.{NODE1_TOR_PORT}");
    let password2 = format!("{test_dir_node2}.{NODE2_TOR_PORT}");

    init_and_unlock_node(node1_addr, &password1).await;
    init_and_unlock_node(node2_addr, &password2).await;

    let node1_info = node_info(node1_addr).await;
    let node1_pubkey = node1_info.pubkey;

    // Connect without Tor
    connect_peer(node2_addr, &node1_pubkey, &format!("127.0.0.1:{NODE1_TOR_PORT}")).await;

    // Verify connection works normally
    let peers = list_peers(node2_addr).await;
    assert_eq!(peers.len(), 1);
    assert!(peers.iter().any(|p| p.pubkey == node1_pubkey));

    println!("✓ Non-Tor connection works as expected");
}

/// Test mixed scenario: one node with Tor, one without
#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn tor_mixed_mode_connection() {
    initialize();

    let test_dir_base = format!("{TEST_DIR_BASE}tor_mixed/");
    let test_dir_node1 = format!("{test_dir_base}node1");
    let test_dir_node2 = format!("{test_dir_base}node2");

    // Node1 with Tor enabled, Node2 without
    let node1_addr = start_tor_daemon(&test_dir_node1, NODE1_TOR_PORT, true).await;
    let node2_addr = start_tor_daemon(&test_dir_node2, NODE2_TOR_PORT, false).await;

    let password1 = format!("{test_dir_node1}.{NODE1_TOR_PORT}");
    let password2 = format!("{test_dir_node2}.{NODE2_TOR_PORT}");

    init_and_unlock_node(node1_addr, &password1).await;
    init_and_unlock_node(node2_addr, &password2).await;

    let node1_info = node_info(node1_addr).await;
    let node1_pubkey = node1_info.pubkey;

    // Node2 (no Tor) connects to Node1 (with Tor) via regular IP
    connect_peer(node2_addr, &node1_pubkey, &format!("127.0.0.1:{NODE1_TOR_PORT}")).await;

    // Verify connection
    let peers = list_peers(node2_addr).await;
    assert_eq!(peers.len(), 1);
    assert!(peers.iter().any(|p| p.pubkey == node1_pubkey));

    let peers = list_peers(node1_addr).await;
    assert_eq!(peers.len(), 1);

    println!("✓ Mixed Tor/non-Tor connection works");
}

/// Test sending payment over Tor-enabled channel
#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn tor_enabled_payment() {
    initialize();

    let test_dir_base = format!("{TEST_DIR_BASE}tor_payment/");
    let test_dir_node1 = format!("{test_dir_base}node1");
    let test_dir_node2 = format!("{test_dir_base}node2");

    let node1_addr = start_tor_daemon(&test_dir_node1, NODE1_TOR_PORT, true).await;
    let node2_addr = start_tor_daemon(&test_dir_node2, NODE2_TOR_PORT, true).await;

    let password1 = format!("{test_dir_node1}.{NODE1_TOR_PORT}");
    let password2 = format!("{test_dir_node2}.{NODE2_TOR_PORT}");

    init_and_unlock_node(node1_addr, &password1).await;
    init_and_unlock_node(node2_addr, &password2).await;

    fund_and_create_utxos(node1_addr, None).await;
    fund_and_create_utxos(node2_addr, None).await;

    let node2_info = node_info(node2_addr).await;
    let node2_pubkey = node2_info.pubkey;

    // Open channel over Tor
    open_channel(
        node1_addr,
        &node2_pubkey,
        Some(NODE2_TOR_PORT),
        None,
        Some(1000000),
        None,
        None,
    )
    .await;

    // Create invoice on node2
    let amount_msat = Some(10000);
    let ln_invoice_response = ln_invoice(node2_addr, amount_msat, None, None, 900).await;
    let invoice = ln_invoice_response.invoice;

    // Send payment from node1 to node2 over Tor
    send_payment(node1_addr, invoice.clone()).await;

    // Verify payment succeeded
    let decoded = decode_ln_invoice(node1_addr, &invoice).await;
    let payment = get_payment(node1_addr, &decoded.payment_hash).await;
    assert_eq!(payment.status, HTLCStatus::Succeeded);
    assert_eq!(payment.amt_msat, amount_msat);

    let payment = get_payment(node2_addr, &decoded.payment_hash).await;
    assert_eq!(payment.status, HTLCStatus::Succeeded);
    assert_eq!(payment.amt_msat, amount_msat);

    println!("✓ Payment succeeded over Tor connection");
}

/// Helper: Start daemon with Tor option
async fn start_tor_daemon(node_test_dir: &str, node_peer_port: u16, enable_tor: bool) -> SocketAddr {
    let listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
    let node_address = listener.local_addr().unwrap();
    std::fs::create_dir_all(node_test_dir).unwrap();

    let args = LdkUserInfo {
        storage_dir_path: node_test_dir.into(),
        ldk_peer_listening_port: node_peer_port,
        enable_tor,
        tor_socks_port: None,
        ..Default::default()
    };

    tokio::spawn(async move {
        let (router, app_state) = app(args).await.unwrap();
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal(app_state))
            .await
            .unwrap();
    });

    // Wait a bit for Tor to bootstrap if enabled
    if enable_tor {
        println!("Waiting for Tor bootstrap on port {node_peer_port}...");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }

    node_address
}

/// Helper: Initialize and unlock a node
async fn init_and_unlock_node(node_addr: SocketAddr, password: &str) {
    // Check if already initialized
    let is_initialized = Path::new(&format!("tmp/tor_connection/*node*/{}/.ldk/init", node_addr.port())).exists();

    if !is_initialized {
        let payload = InitRequest {
            password: password.to_string(),
        };
        let res = reqwest::Client::new()
            .post(format!("http://{node_addr}/init"))
            .json(&payload)
            .send()
            .await
            .unwrap();
        _check_response_is_ok(res)
            .await
            .json::<InitResponse>()
            .await
            .unwrap();
    }

    unlock(node_addr, password).await;
}
