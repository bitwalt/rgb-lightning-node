# Quick Test: Two Nodes via Tor with .onion Address

This is the quick reference for testing two RGB Lightning nodes connecting via Tor using pubkey@onion format and opening a channel.

## Prerequisites
```bash
# Build the project
cargo build --release

# Start Bitcoin Core in regtest
bitcoind -regtest -daemon

# Generate initial blocks
bitcoin-cli -regtest createwallet "test"
ADDRESS=$(bitcoin-cli -regtest getnewaddress)
bitcoin-cli -regtest generatetoaddress 101 $ADDRESS
```

## Test Execution

### Terminal 1: Alice (Node with .onion address)

```bash
# Start Alice with Tor
./target/release/rgb-lightning-node /tmp/alice-node \
    --enable-tor \
    --daemon-listening-port 3001 \
    --ldk-peer-listening-port 9735 \
    --network regtest

# Wait for "Tor client bootstrapped successfully"
```

### Terminal 2: Alice Setup

```bash
# Initialize Alice
curl -X POST http://localhost:3001/init \
    -H "Content-Type: application/json" \
    -d '{
        "password": "test123",
        "mnemonic": "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art"
    }'

# Unlock Alice
curl -X POST http://localhost:3001/unlock \
    -H "Content-Type: application/json" \
    -d '{
        "password": "test123",
        "bitcoind_rpc_host": "localhost",
        "bitcoind_rpc_port": 18443,
        "bitcoind_rpc_username": "test",
        "bitcoind_rpc_password": "test"
    }'

# Get Alice's pubkey
ALICE_PUBKEY=$(curl -s http://localhost:3001/nodeinfo | jq -r '.pubkey')
echo "Alice pubkey: $ALICE_PUBKEY"

# Fund Alice
ALICE_ADDR=$(curl -s http://localhost:3001/btcbalance | jq -r '.address')
bitcoin-cli -regtest sendtoaddress $ALICE_ADDR 1.0
bitcoin-cli -regtest generatetoaddress 6 $ADDRESS
```

### Terminal 3: Bob (Connecting node)

```bash
# Start Bob with Tor
./target/release/rgb-lightning-node /tmp/bob-node \
    --enable-tor \
    --daemon-listening-port 3002 \
    --ldk-peer-listening-port 9736 \
    --network regtest

# Wait for "Tor client bootstrapped successfully"
```

### Terminal 4: Bob Setup and Connection

```bash
# Initialize Bob
curl -X POST http://localhost:3002/init \
    -H "Content-Type: application/json" \
    -d '{
        "password": "test123",
        "mnemonic": "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote"
    }'

# Unlock Bob
curl -X POST http://localhost:3002/unlock \
    -H "Content-Type: application/json" \
    -d '{
        "password": "test123",
        "bitcoind_rpc_host": "localhost",
        "bitcoind_rpc_port": 18443,
        "bitcoind_rpc_username": "test",
        "bitcoind_rpc_password": "test"
    }'

# Connect Bob to Alice
# For testing on same machine, use localhost:
curl -X POST http://localhost:3002/connectpeer \
    -H "Content-Type: application/json" \
    -d "{
        \"peer_pubkey_and_addr\": \"$ALICE_PUBKEY@127.0.0.1:9735\"
    }"

# Expected response: {}

# Verify connection
curl http://localhost:3002/listpeers | jq
# Should show Alice as connected peer

# Verify from Alice's side
curl http://localhost:3001/listpeers | jq
# Should show Bob as connected peer
```

### Open Channel via Tor

```bash
# Open channel from Bob to Alice
curl -X POST http://localhost:3002/openchannel \
    -H "Content-Type: application/json" \
    -d "{
        \"peer_pubkey_and_addr\": \"$ALICE_PUBKEY@127.0.0.1:9735\",
        \"capacity_sat\": 1000000,
        \"push_msat\": 0
    }"

# Mine blocks to confirm
bitcoin-cli -regtest generatetoaddress 6 $ADDRESS

# Wait for confirmations (10 seconds)
sleep 10

# Check channels on both nodes
echo "=== Alice's channels ==="
curl http://localhost:3001/listchannels | jq

echo "=== Bob's channels ==="
curl http://localhost:3002/listchannels | jq
```

## Testing with Real .onion Address

To test with an actual .onion hidden service:

### Setup Tor Hidden Service

```bash
# Install Tor
sudo apt-get install tor

# Edit /etc/tor/torrc
sudo nano /etc/tor/torrc

# Add:
HiddenServiceDir /var/lib/tor/alice-lightning/
HiddenServicePort 9735 127.0.0.1:9735

# Restart Tor
sudo systemctl restart tor

# Get Alice's .onion address
ALICE_ONION=$(sudo cat /var/lib/tor/alice-lightning/hostname)
echo "Alice's onion address: $ALICE_ONION"
```

### Connect Using .onion

```bash
# Connect Bob to Alice using .onion address
curl -X POST http://localhost:3002/connectpeer \
    -H "Content-Type: application/json" \
    -d "{
        \"peer_pubkey_and_addr\": \"$ALICE_PUBKEY@$ALICE_ONION:9735\"
    }"

# This will connect through Tor circuits using Arti
```

## Expected Results

### ✅ Success Indicators

1. **Tor Bootstrap:**
```
[INFO] Initializing Tor support...
[INFO] Tor client bootstrapped successfully
```

2. **Peer Connection:**
```bash
$ curl http://localhost:3002/listpeers | jq
[
  {
    "pubkey": "<alice_pubkey>",
    "is_connected": true,
    "is_persisted": false
  }
]
```

3. **Channel Opening:**
```bash
$ curl http://localhost:3002/listchannels | jq
[
  {
    "channel_id": "...",
    "counterparty_node_id": "<alice_pubkey>",
    "funding_txo": "...",
    "is_usable": true,
    "is_public": false,
    "balance_msat": 1000000000,
    ...
  }
]
```

## Verification Checklist

- [ ] Alice's node shows: "Tor client bootstrapped successfully"
- [ ] Bob's node shows: "Tor client bootstrapped successfully"
- [ ] Bob successfully connects to Alice
- [ ] Both nodes show each other in `/listpeers`
- [ ] Channel opens without errors
- [ ] After confirmations, channel shows in `/listchannels` on both nodes
- [ ] Channel state is "is_usable": true

## Test Logs to Monitor

**Alice's logs:**
```
Initializing Tor support...
Tor client bootstrapped successfully
```

**Bob's logs:**
```
Initializing Tor support...
Tor client bootstrapped successfully
Connecting to peer via Tor: 127.0.0.1:9735
Successfully connected to peer via Tor: 127.0.0.1:9735
```

## Troubleshooting

### Issue: "Failed to bootstrap Tor client"
**Solution:** Wait 60-90 seconds for Tor to bootstrap. Check network connectivity.

### Issue: "FailedPeerConnection"
**Solution:**
- Verify Alice's node is running
- Check port 9735 is not blocked
- Ensure both nodes have Tor enabled

### Issue: Channel open fails
**Solution:**
- Verify Alice has sufficient on-chain funds
- Ensure Bitcoin node is synced
- Check both nodes are unlocked

## Performance Notes

- **Tor Bootstrap:** 30-90 seconds first time
- **Peer Connection:** 1-5 seconds via Tor
- **Channel Open:** Immediate (pending Bitcoin confirmations)
- **Confirmations:** 6 blocks required (instant in regtest)

## Running Unit Tests

```bash
# Test .onion address detection
cargo test --bin rgb-lightning-node tor::tests::test_is_onion_address -- --nocapture

# Test peer address parsing
cargo test --bin rgb-lightning-node tor::tests::test_parse_peer_address -- --nocapture

# All Tor unit tests
cargo test --bin rgb-lightning-node tor::tests
```

**Expected output:**
```
running 2 tests
test tor::tests::test_is_onion_address ... ok
test tor::tests::test_parse_peer_address ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured
```

## Summary

This test demonstrates:
1. ✅ Two nodes can connect via Tor using `pubkey@address` format
2. ✅ Supports both IP addresses and .onion addresses
3. ✅ Channels can be opened over Tor connections
4. ✅ Tor connectivity is transparent to the Lightning protocol
5. ✅ All unit tests pass successfully

For full documentation, see [TOR_TESTING.md](TOR_TESTING.md)
