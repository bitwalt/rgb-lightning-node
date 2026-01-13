use super::*;

const TEST_DIR_BASE: &str = "tmp/hodl_invoice/";

#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn hodl_invoice_basic_flow() {
    initialize();

    let test_dir_node1 = format!("{TEST_DIR_BASE}basic_flow/node1");
    let test_dir_node2 = format!("{TEST_DIR_BASE}basic_flow/node2");
    let (node1_addr, _) = start_node(&test_dir_node1, NODE1_PEER_PORT, false).await;
    let (node2_addr, _) = start_node(&test_dir_node2, NODE2_PEER_PORT, false).await;

    fund_and_create_utxos(node1_addr, None).await;
    fund_and_create_utxos(node2_addr, None).await;

    let node2_pubkey = node_info(node2_addr).await.pubkey;

    open_channel(
        node1_addr,
        &node2_pubkey,
        Some(NODE2_PEER_PORT),
        Some(600000),
        Some(400000),
        None,
        None,
    )
    .await;

    // Wait for channels to be usable
    wait_for_usable_channels(node1_addr, 1).await;
    wait_for_usable_channels(node2_addr, 1).await;

    // Create a HODL invoice on node1
    let amt_msat = 100000;
    let payload = HodlInvoiceRequest {
        amt_msat: Some(amt_msat),
        expiry_sec: 900,
        payment_hash: None, // Let the node generate the preimage/hash
    };
    let hodl_res: HodlInvoiceResponse = reqwest::Client::new()
        .post(format!("http://{node1_addr}/hodlinvoice"))
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    println!("Created HODL invoice: {}", hodl_res.invoice);
    println!("Payment hash: {}", hodl_res.payment_hash);
    println!("Payment secret: {}", hodl_res.payment_secret);

    // Check invoice status - should be Pending
    let status = check_invoice_status(node1_addr, &hodl_res.invoice).await;
    assert_eq!(status, InvoiceStatus::Pending);

    // Node2 sends payment to the HODL invoice
    println!("Node2 sending payment to HODL invoice...");
    let send_payload = SendPaymentRequest {
        invoice: hodl_res.invoice.clone(),
        amt_msat: None,
    };
    let _send_res = reqwest::Client::new()
        .post(format!("http://{node2_addr}/sendpayment"))
        .json(&send_payload)
        .send()
        .await
        .unwrap();

    // Wait a bit for the payment to arrive
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Check invoice status - should now be Held (not auto-claimed)
    let status = check_invoice_status(node1_addr, &hodl_res.invoice).await;
    assert_eq!(status, InvoiceStatus::Held);
    println!("✓ Invoice is in Held status");

    // For testing, we need to derive the preimage that was generated
    // Since we can't retrieve it, we'll create a new HODL invoice with a known preimage
    // Let's use a different approach: create a HODL invoice with custom hash where we know the preimage

    // Cancel the first one
    let cancel_payload = CancelInvoiceRequest {
        payment_hash: hodl_res.payment_hash.clone(),
    };
    let _cancel_res: CancelInvoiceResponse = reqwest::Client::new()
        .post(format!("http://{node1_addr}/cancelinvoice"))
        .json(&cancel_payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Create a new HODL invoice with known preimage
    let known_preimage = "0101010101010101010101010101010101010101010101010101010101010101";
    let known_hash = compute_payment_hash(known_preimage);

    let payload2 = HodlInvoiceRequest {
        amt_msat: Some(amt_msat),
        expiry_sec: 900,
        payment_hash: Some(known_hash.clone()),
    };
    let hodl_res2: HodlInvoiceResponse = reqwest::Client::new()
        .post(format!("http://{node1_addr}/hodlinvoice"))
        .json(&payload2)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Node2 pays the new invoice
    let send_payload2 = SendPaymentRequest {
        invoice: hodl_res2.invoice.clone(),
        amt_msat: None,
    };
    let _send_res2 = reqwest::Client::new()
        .post(format!("http://{node2_addr}/sendpayment"))
        .json(&send_payload2)
        .send()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Verify it's held
    let status = check_invoice_status(node1_addr, &hodl_res2.invoice).await;
    assert_eq!(status, InvoiceStatus::Held);

    // Now settle with the known preimage
    let settle_payload = SettleInvoiceRequest {
        payment_hash: known_hash,
        payment_preimage: known_preimage.to_string(),
    };
    let settle_res: SettleInvoiceResponse = reqwest::Client::new()
        .post(format!("http://{node1_addr}/settleinvoice"))
        .json(&settle_payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(settle_res.success);
    println!("✓ Successfully settled HODL invoice");

    // Wait for settlement to complete
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Check final status - should be Succeeded
    let status = check_invoice_status(node1_addr, &hodl_res2.invoice).await;
    assert_eq!(status, InvoiceStatus::Succeeded);
    println!("✓ Invoice is in Succeeded status");

    shutdown(&[node1_addr, node2_addr]).await;
}

#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn hodl_invoice_cancel_flow() {
    initialize();

    let test_dir_node1 = format!("{TEST_DIR_BASE}cancel_flow/node1");
    let test_dir_node2 = format!("{TEST_DIR_BASE}cancel_flow/node2");
    let (node1_addr, _) = start_node(&test_dir_node1, NODE1_PEER_PORT, false).await;
    let (node2_addr, _) = start_node(&test_dir_node2, NODE2_PEER_PORT, false).await;

    fund_and_create_utxos(node1_addr, None).await;
    fund_and_create_utxos(node2_addr, None).await;

    let node2_pubkey = node_info(node2_addr).await.pubkey;

    open_channel(
        node1_addr,
        &node2_pubkey,
        Some(NODE2_PEER_PORT),
        Some(600000),
        Some(400000),
        None,
        None,
    )
    .await;

    // Wait for channels to be usable
    wait_for_usable_channels(node1_addr, 1).await;
    wait_for_usable_channels(node2_addr, 1).await;

    // Create a HODL invoice
    let payload = HodlInvoiceRequest {
        amt_msat: Some(50000),
        expiry_sec: 900,
        payment_hash: None,
    };
    let hodl_res: HodlInvoiceResponse = reqwest::Client::new()
        .post(format!("http://{node1_addr}/hodlinvoice"))
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Node2 sends payment
    let send_payload = SendPaymentRequest {
        invoice: hodl_res.invoice.clone(),
        amt_msat: None,
    };
    let _send_res = reqwest::Client::new()
        .post(format!("http://{node2_addr}/sendpayment"))
        .json(&send_payload)
        .send()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Verify it's held
    let status = check_invoice_status(node1_addr, &hodl_res.invoice).await;
    assert_eq!(status, InvoiceStatus::Held);

    // Cancel the invoice instead of settling
    let cancel_payload = CancelInvoiceRequest {
        payment_hash: hodl_res.payment_hash.clone(),
    };
    let cancel_res: CancelInvoiceResponse = reqwest::Client::new()
        .post(format!("http://{node1_addr}/cancelinvoice"))
        .json(&cancel_payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(cancel_res.success);
    println!("✓ Successfully cancelled HODL invoice");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Check final status - should be Failed
    let status = check_invoice_status(node1_addr, &hodl_res.invoice).await;
    assert_eq!(status, InvoiceStatus::Failed);
    println!("✓ Invoice is in Failed status after cancellation");

    shutdown(&[node1_addr, node2_addr]).await;
}

#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn hodl_invoice_with_custom_hash() {
    initialize();

    let test_dir_node1 = format!("{TEST_DIR_BASE}custom_hash/node1");
    let (node1_addr, _) = start_node(&test_dir_node1, NODE1_PEER_PORT, false).await;

    fund_and_create_utxos(node1_addr, None).await;

    // Generate a custom payment hash (for testing proxy invoice scenarios)
    let custom_hash = "1111111111111111111111111111111111111111111111111111111111111111";

    // Create HODL invoice with custom hash
    let payload = HodlInvoiceRequest {
        amt_msat: Some(100000),
        expiry_sec: 900,
        payment_hash: Some(custom_hash.to_string()),
    };
    let hodl_res: HodlInvoiceResponse = reqwest::Client::new()
        .post(format!("http://{node1_addr}/hodlinvoice"))
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Verify the returned hash matches what we provided
    assert_eq!(hodl_res.payment_hash, custom_hash);
    println!("✓ HODL invoice created with custom payment hash");

    // Verify invoice was created successfully
    let status = check_invoice_status(node1_addr, &hodl_res.invoice).await;
    assert_eq!(status, InvoiceStatus::Pending);

    shutdown(&[node1_addr]).await;
}

#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn hodl_invoice_error_settle_non_held() {
    initialize();

    let test_dir_node1 = format!("{TEST_DIR_BASE}error_settle/node1");
    let (node1_addr, _) = start_node(&test_dir_node1, NODE1_PEER_PORT, false).await;

    fund_and_create_utxos(node1_addr, None).await;

    // Create HODL invoice with known preimage
    let known_preimage = "0303030303030303030303030303030303030303030303030303030303030303";
    let known_hash = compute_payment_hash(known_preimage);

    let payload = HodlInvoiceRequest {
        amt_msat: Some(100000),
        expiry_sec: 900,
        payment_hash: Some(known_hash.clone()),
    };
    let _hodl_res: HodlInvoiceResponse = reqwest::Client::new()
        .post(format!("http://{node1_addr}/hodlinvoice"))
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Try to settle before payment is received (invoice is still Pending, not Held)
    let settle_payload = SettleInvoiceRequest {
        payment_hash: known_hash,
        payment_preimage: known_preimage.to_string(),
    };
    let res = reqwest::Client::new()
        .post(format!("http://{node1_addr}/settleinvoice"))
        .json(&settle_payload)
        .send()
        .await
        .unwrap();

    // Should fail because invoice is not in Held status
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);
    println!("✓ Correctly rejected settle attempt on non-held invoice");

    shutdown(&[node1_addr]).await;
}

#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn hodl_invoice_error_invalid_preimage() {
    initialize();

    let test_dir_node1 = format!("{TEST_DIR_BASE}invalid_preimage/node1");
    let test_dir_node2 = format!("{TEST_DIR_BASE}invalid_preimage/node2");
    let (node1_addr, _) = start_node(&test_dir_node1, NODE1_PEER_PORT, false).await;
    let (node2_addr, _) = start_node(&test_dir_node2, NODE2_PEER_PORT, false).await;

    fund_and_create_utxos(node1_addr, None).await;
    fund_and_create_utxos(node2_addr, None).await;

    let node2_pubkey = node_info(node2_addr).await.pubkey;

    open_channel(
        node1_addr,
        &node2_pubkey,
        Some(NODE2_PEER_PORT),
        Some(600000),
        Some(400000),
        None,
        None,
    )
    .await;

    // Create HODL invoice with known preimage
    let correct_preimage = "0404040404040404040404040404040404040404040404040404040404040404";
    let correct_hash = compute_payment_hash(correct_preimage);

    let payload = HodlInvoiceRequest {
        amt_msat: Some(100000),
        expiry_sec: 900,
        payment_hash: Some(correct_hash.clone()),
    };
    let hodl_res: HodlInvoiceResponse = reqwest::Client::new()
        .post(format!("http://{node1_addr}/hodlinvoice"))
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Node2 sends payment
    let send_payload = SendPaymentRequest {
        invoice: hodl_res.invoice.clone(),
        amt_msat: None,
    };
    let _send_res = reqwest::Client::new()
        .post(format!("http://{node2_addr}/sendpayment"))
        .json(&send_payload)
        .send()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Try to settle with wrong preimage
    let wrong_preimage = "2222222222222222222222222222222222222222222222222222222222222222";
    let settle_payload = SettleInvoiceRequest {
        payment_hash: correct_hash,
        payment_preimage: wrong_preimage.to_string(),
    };
    let res = reqwest::Client::new()
        .post(format!("http://{node1_addr}/settleinvoice"))
        .json(&settle_payload)
        .send()
        .await
        .unwrap();

    // Should fail because preimage doesn't match hash
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);
    println!("✓ Correctly rejected invalid preimage");

    shutdown(&[node1_addr, node2_addr]).await;
}

#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn hodl_invoice_vs_standard_invoice() {
    initialize();

    let test_dir_node1 = format!("{TEST_DIR_BASE}hodl_vs_standard/node1");
    let test_dir_node2 = format!("{TEST_DIR_BASE}hodl_vs_standard/node2");
    let (node1_addr, _) = start_node(&test_dir_node1, NODE1_PEER_PORT, false).await;
    let (node2_addr, _) = start_node(&test_dir_node2, NODE2_PEER_PORT, false).await;

    fund_and_create_utxos(node1_addr, None).await;
    fund_and_create_utxos(node2_addr, None).await;

    let node2_pubkey = node_info(node2_addr).await.pubkey;

    open_channel(
        node1_addr,
        &node2_pubkey,
        Some(NODE2_PEER_PORT),
        Some(600000),
        Some(400000),
        None,
        None,
    )
    .await;

    // Create a standard invoice
    let standard_payload = LNInvoiceRequest {
        amt_msat: Some(50000),
        expiry_sec: 900,
        asset_id: None,
        asset_amount: None,
    };
    let standard_invoice: LNInvoiceResponse = reqwest::Client::new()
        .post(format!("http://{node1_addr}/lninvoice"))
        .json(&standard_payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Node2 pays standard invoice
    let send_payload = SendPaymentRequest {
        invoice: standard_invoice.invoice.clone(),
        amt_msat: None,
    };
    let _send_res = reqwest::Client::new()
        .post(format!("http://{node2_addr}/sendpayment"))
        .json(&send_payload)
        .send()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Standard invoice should be auto-claimed (Succeeded, not Held)
    let status = check_invoice_status(node1_addr, &standard_invoice.invoice).await;
    assert_eq!(status, InvoiceStatus::Succeeded);
    println!("✓ Standard invoice auto-claimed successfully");

    // Create a HODL invoice
    let hodl_payload = HodlInvoiceRequest {
        amt_msat: Some(50000),
        expiry_sec: 900,
        payment_hash: None,
    };
    let hodl_invoice: HodlInvoiceResponse = reqwest::Client::new()
        .post(format!("http://{node1_addr}/hodlinvoice"))
        .json(&hodl_payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Node2 pays HODL invoice
    let send_payload = SendPaymentRequest {
        invoice: hodl_invoice.invoice.clone(),
        amt_msat: None,
    };
    let _send_res = reqwest::Client::new()
        .post(format!("http://{node2_addr}/sendpayment"))
        .json(&send_payload)
        .send()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // HODL invoice should NOT be auto-claimed (Held, not Succeeded)
    let status = check_invoice_status(node1_addr, &hodl_invoice.invoice).await;
    assert_eq!(status, InvoiceStatus::Held);
    println!("✓ HODL invoice correctly held (not auto-claimed)");

    shutdown(&[node1_addr, node2_addr]).await;
}

// Helper function to check invoice status
async fn check_invoice_status(node_addr: SocketAddr, invoice: &str) -> InvoiceStatus {
    let payload = InvoiceStatusRequest {
        invoice: invoice.to_string(),
    };
    let res: InvoiceStatusResponse = reqwest::Client::new()
        .post(format!("http://{node_addr}/invoicestatus"))
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    res.status
}

// Helper function to compute payment hash from preimage
fn compute_payment_hash(preimage_hex: &str) -> String {
    use bitcoin::hashes::{sha256, Hash};
    use hex::DisplayHex;

    // Decode preimage from hex
    let preimage_bytes = hex_str_to_vec(preimage_hex).expect("Invalid preimage hex");

    // Hash the preimage to get the payment hash
    let hash = sha256::Hash::hash(&preimage_bytes);

    hash.as_byte_array().as_hex().to_string()
}
