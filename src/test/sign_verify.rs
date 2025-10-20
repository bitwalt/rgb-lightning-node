use super::*;

#[tokio::test]
#[traced_test]
async fn sign_verify() {
    initialize();

    let node1_test_dir = "tmp/test_name/node1";
    let (node1_address, _) = start_node(node1_test_dir, NODE1_PEER_PORT, false).await;

    let node_info = node_info(node1_address).await;
    let pubkey = node_info.pubkey;

    let message = "test message to sign";
    let signed_message = sign_message(node1_address, message).await;
    assert!(!signed_message.is_empty());

    let verified = verify_message(node1_address, message, &signed_message, &pubkey).await;
    assert!(verified);

    let verified = verify_message(node1_address, "wrong message", &signed_message, &pubkey).await;
    assert!(!verified);

    shutdown(&[node1_address]).await;
}
