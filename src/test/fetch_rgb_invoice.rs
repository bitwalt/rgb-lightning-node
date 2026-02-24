use super::*;

const TEST_DIR_BASE: &str = "tmp/fetch_rgb_invoice/";

#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn success() {
    initialize();

    let test_dir_base = format!("{TEST_DIR_BASE}success/");
    let test_dir_node1 = format!("{test_dir_base}node1");
    let (node1_addr, _) = start_node(&test_dir_node1, NODE1_PEER_PORT, false).await;

    fund_and_create_utxos(node1_addr, None).await;

    let invoice_resp = rgb_invoice(node1_addr, None, false).await;
    let batch_transfer_idx = invoice_resp.batch_transfer_idx;

    let fetched = fetch_rgb_invoice(node1_addr, batch_transfer_idx, None).await;
    assert_eq!(fetched.invoice, invoice_resp.invoice);
    assert_eq!(fetched.recipient_id, invoice_resp.recipient_id);
    assert_eq!(fetched.batch_transfer_idx, batch_transfer_idx);
    assert_eq!(fetched.expiration_timestamp, invoice_resp.expiration_timestamp);
}
