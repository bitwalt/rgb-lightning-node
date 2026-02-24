use super::*;

const TEST_DIR_BASE: &str = "tmp/bolt12/";

#[serial_test::serial]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[traced_test]
async fn create_offer() {
    initialize();

    let test_dir_node1 = format!("{TEST_DIR_BASE}node1");
    let (node1_addr, _) = start_node(&test_dir_node1, NODE1_PEER_PORT, false).await;

    // create an offer with no parameters should succeed
    let payload = CreateOfferRequest {
        amt_msat: None,
        description: None,
        expiry_sec: None,
    };
    let res = reqwest::Client::new()
        .post(format!("http://{node1_addr}/createoffer"))
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json::<CreateOfferResponse>()
        .await;
    assert!(res.is_ok());
    let offer = res.unwrap().offer;
    assert!(offer.starts_with("lno1"));

    // create an offer with an amount should succeed
    let payload = CreateOfferRequest {
        amt_msat: Some(100_000),
        description: None,
        expiry_sec: None,
    };
    let res = reqwest::Client::new()
        .post(format!("http://{node1_addr}/createoffer"))
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json::<CreateOfferResponse>()
        .await;
    assert!(res.is_ok());
    let offer = res.unwrap().offer;
    assert!(offer.starts_with("lno1"));

    // create an offer with description and expiry should succeed
    let payload = CreateOfferRequest {
        amt_msat: Some(50_000),
        description: Some("test offer".to_string()),
        expiry_sec: Some(3600),
    };
    let res = reqwest::Client::new()
        .post(format!("http://{node1_addr}/createoffer"))
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json::<CreateOfferResponse>()
        .await;
    assert!(res.is_ok());
    let offer = res.unwrap().offer;
    assert!(offer.starts_with("lno1"));
}
