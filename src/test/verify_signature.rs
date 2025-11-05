use super::*;

const TEST_DIR_BASE: &str = "tmp/verify_signature/";

#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn verify_signature_endpoint() {
    initialize();

    let test_dir_node = format!("{TEST_DIR_BASE}node");
    let (node_addr, _) = start_node(&test_dir_node, NODE1_PEER_PORT, false).await;

    let node_info = node_info(node_addr).await;
    let message = "rgb-lightning";
    let signed_message = sign_message_api(node_addr, message).await;

    assert!(verify_signature_api(node_addr, message, &node_info.pubkey, &signed_message).await);

    // Verification should fail if the message changes
    assert!(
        !verify_signature_api(
            node_addr,
            "different message",
            &node_info.pubkey,
            &signed_message,
        )
        .await
    );
}
