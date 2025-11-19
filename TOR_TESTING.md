# Tor Connection Testing Guide

This guide explains how to test Tor connectivity in the RGB Lightning Node, including connecting two nodes via Tor and opening channels.

## Table of Contents
- [Prerequisites](#prerequisites)
- [Quick Tests](#quick-tests)
- [Manual Two-Node Testing](#manual-two-node-testing)
- [Testing with .onion Addresses](#testing-with-onion-addresses)
- [Test Results](#test-results)
- [Troubleshooting](#troubleshooting)

## Prerequisites

### Build the Project
```bash
cargo build --release
```

### Requirements for Full Testing
- Bitcoin Core (regtest mode) or connection to a Bitcoin node
- Network access for Tor bootstrap
- (Optional) Tor daemon for hidden service setup

## Quick Tests

### 1. Run Unit Tests
These tests verify basic functionality without requiring network access:

```bash
# Test .onion address detection
cargo test --bin rgb-lightning-node tor::tests::test_is_onion_address

# Test peer address parsing
cargo test --bin rgb-lightning-node tor::tests::test_parse_peer_address

# Run all non-network Tor tests
cargo test --bin rgb-lightning-node tor::tests -- --test-threads=1
```

**Expected Output:**
```
running 2 tests
test tor::tests::test_is_onion_address ... ok
test tor::tests::test_parse_peer_address ... ok

test result: ok. 2 passed; 0 failed; 0 ignored
```

### 2. Run Network Tests (Ignored by Default)
These tests require internet access and Tor bootstrap:

```bash
# Test Tor client bootstrap (takes time)
cargo test --bin rgb-lightning-node tor::tests::test_tor_client_bootstrap -- --ignored --nocapture

# Test clearnet connection via Tor
cargo test --bin rgb-lightning-node tor::tests::test_connect_to_clearnet_via_tor -- --ignored --nocapture
```

### 3. Use the Test Script
```bash
# Interactive menu
./test_tor_connection.sh

# Run all basic tests
./test_tor_connection.sh --all

# Run network tests
./test_tor_connection.sh --network

# Show manual testing instructions
./test_tor_connection.sh --manual
```

## Manual Two-Node Testing

This section describes how to test two actual RGB Lightning Nodes connecting via Tor and opening a channel.

### Step 1: Prepare Test Environment

```bash
# Create directories
mkdir -p /tmp/alice-node
mkdir -p /tmp/bob-node

# Ensure Bitcoin Core is running in regtest mode
bitcoind -regtest -daemon

# Generate some blocks if needed
bitcoin-cli -regtest generatetoaddress 101 <address>
```

### Step 2: Start Node 1 (Alice) with Tor

```bash
./target/release/rgb-lightning-node /tmp/alice-node \
    --enable-tor \
    --daemon-listening-port 3001 \
    --ldk-peer-listening-port 9735 \
    --network regtest
```

**What happens:**
- Node starts and listens for HTTP requests on port 3001
- Tor client bootstraps (this may take 30-60 seconds)
- LDK peer listener starts on port 9735
- Logs will show: "Tor client bootstrapped successfully"

### Step 3: Initialize and Unlock Alice

```bash
# Generate a mnemonic or use an existing one
ALICE_MNEMONIC="word1 word2 ... word24"

# Initialize the node
curl -X POST http://localhost:3001/init \
    -H "Content-Type: application/json" \
    -d "{
        \"password\": \"test123\",
        \"mnemonic\": \"${ALICE_MNEMONIC}\"
    }"

# Unlock the node
curl -X POST http://localhost:3001/unlock \
    -H "Content-Type: application/json" \
    -d '{
        \"password\": \"test123\",
        \"bitcoind_rpc_host\": \"localhost\",
        \"bitcoind_rpc_port\": 18443,
        \"bitcoind_rpc_username\": \"user\",
        \"bitcoind_rpc_password\": \"password\"
    }'

# Get Alice's node info
curl http://localhost:3001/nodeinfo | jq
```

**Save Alice's pubkey:**
```bash
ALICE_PUBKEY=$(curl -s http://localhost:3001/nodeinfo | jq -r '.pubkey')
echo "Alice's pubkey: $ALICE_PUBKEY"
```

### Step 4: Start Node 2 (Bob) with Tor

```bash
./target/release/rgb-lightning-node /tmp/bob-node \
    --enable-tor \
    --daemon-listening-port 3002 \
    --ldk-peer-listening-port 9736 \
    --network regtest
```

### Step 5: Initialize and Unlock Bob

```bash
# Use a different mnemonic
BOB_MNEMONIC="different1 different2 ... different24"

# Initialize
curl -X POST http://localhost:3002/init \
    -H "Content-Type: application/json" \
    -d "{
        \"password\": \"test123\",
        \"mnemonic\": \"${BOB_MNEMONIC}\"
    }"

# Unlock
curl -X POST http://localhost:3002/unlock \
    -H "Content-Type: application/json" \
    -d '{
        \"password\": \"test123\",
        \"bitcoind_rpc_host\": \"localhost\",
        \"bitcoind_rpc_port\": 18443,
        \"bitcoind_rpc_username\": \"user\",
        \"bitcoind_rpc_password\": \"password\"
    }'
```

### Step 6: Connect Bob to Alice

```bash
# Connect via localhost (both nodes on same machine)
curl -X POST http://localhost:3002/connectpeer \
    -H "Content-Type: application/json" \
    -d "{
        \"peer_pubkey_and_addr\": \"${ALICE_PUBKEY}@127.0.0.1:9735\"
    }"
```

**Expected Response:**
```json
{}
```

If Tor is working and the nodes are on different machines, you can use Alice's .onion address (see next section).

### Step 7: Verify Connection

```bash
# Check peers on Bob's node
curl http://localhost:3002/listpeers | jq

# Check peers on Alice's node
curl http://localhost:3001/listpeers | jq
```

**Expected Output:**
Both nodes should show the other as a connected peer.

```json
[
  {
    "pubkey": "<peer_pubkey>",
    "is_connected": true,
    "is_persisted": false
  }
]
```

### Step 8: Open a Channel

Before opening a channel, ensure both nodes have on-chain funds:

```bash
# Get Alice's on-chain address
ALICE_ADDR=$(curl -s http://localhost:3001/btcbalance | jq -r '.address')

# Send funds to Alice
bitcoin-cli -regtest sendtoaddress $ALICE_ADDR 1.0

# Mine blocks to confirm
bitcoin-cli -regtest generatetoaddress 6 <address>

# Wait for sync
sleep 5

# Check Alice's balance
curl http://localhost:3001/btcbalance | jq
```

Now open a channel from Alice to Bob:

```bash
curl -X POST http://localhost:3001/openchannel \
    -H "Content-Type: application/json" \
    -d "{
        \"peer_pubkey_and_addr\": \"$(curl -s http://localhost:3002/nodeinfo | jq -r '.pubkey')@127.0.0.1:9736\",
        \"capacity_sat\": 1000000,
        \"push_msat\": 0
    }"
```

### Step 9: Verify Channel

```bash
# Mine blocks to confirm channel
bitcoin-cli -regtest generatetoaddress 6 <address>

# Wait for confirmations
sleep 10

# Check channels on both nodes
curl http://localhost:3001/listchannels | jq
curl http://localhost:3002/listchannels | jq
```

**Expected Output:**
Both nodes should show the channel as active after confirmations.

## Testing with .onion Addresses

To test actual .onion hidden service connections, you need to configure a Tor hidden service.

### Option 1: Using Tor Daemon

1. Install Tor:
```bash
sudo apt-get install tor  # Debian/Ubuntu
brew install tor          # macOS
```

2. Configure hidden service in `/etc/tor/torrc`:
```
HiddenServiceDir /var/lib/tor/lightning-alice/
HiddenServicePort 9735 127.0.0.1:9735
```

3. Restart Tor:
```bash
sudo systemctl restart tor
```

4. Get the .onion address:
```bash
sudo cat /var/lib/tor/lightning-alice/hostname
```

5. Connect Bob to Alice using .onion address:
```bash
curl -X POST http://localhost:3002/connectpeer \
    -H "Content-Type: application/json" \
    -d "{
        \"peer_pubkey_and_addr\": \"${ALICE_PUBKEY}@<alice-onion-address>:9735\"
    }"
```

### Option 2: Using Arti Directly

With `--enable-tor`, the RGB Lightning Node uses Arti internally. To connect to .onion addresses:

1. Start Alice with Tor:
```bash
./target/release/rgb-lightning-node /tmp/alice-node \
    --enable-tor \
    --ldk-peer-listening-port 9735
```

2. If Alice publishes an .onion address (via announce addresses), Bob can connect directly:
```bash
curl -X POST http://localhost:3002/connectpeer \
    -H "Content-Type: application/json" \
    -d "{
        \"peer_pubkey_and_addr\": \"${ALICE_PUBKEY}@<onion-address>:9735\"
    }"
```

## Test Results

### ✅ Successful Test Indicators

1. **Tor Bootstrap:**
   - Log message: "Tor client bootstrapped successfully"
   - No errors in node startup

2. **Peer Connection:**
   - `/listpeers` shows peer with `is_connected: true`
   - No connection errors in logs

3. **Channel Opening:**
   - `/openchannel` returns channel ID
   - After confirmations, `/listchannels` shows active channel
   - Channel state transitions: Funded → Confirmed → Active

### ❌ Common Issues

| Issue | Symptom | Solution |
|-------|---------|----------|
| Tor bootstrap timeout | "Failed to bootstrap Tor client" | Check network connectivity, wait longer |
| Connection refused | "FailedPeerConnection" | Verify peer is listening, check firewall |
| Invalid .onion address | "Failed to resolve address" | Verify address format, ensure Tor is enabled |
| Channel open fails | "Insufficient funds" | Ensure node has on-chain balance |

## Performance Expectations

- **Tor Bootstrap:** 30-90 seconds on first start
- **Peer Connection:** 1-5 seconds
- **Channel Opening:** Immediate (pending confirmations)
- **Channel Confirmation:** 6 blocks (~1 hour on Bitcoin mainnet, instant in regtest)

## Advanced Testing

### Testing SOCKS Proxy

If you have a Tor SOCKS proxy running on port 9050:

```bash
./target/release/rgb-lightning-node /tmp/node \
    --enable-tor \
    --tor-socks-port 9050
```

This will use the external SOCKS proxy instead of Arti's internal circuits.

### Testing Multiple Concurrent Connections

```bash
# Start multiple nodes
for i in {1..3}; do
    PORT=$((3000 + i))
    PEER_PORT=$((9734 + i))
    ./target/release/rgb-lightning-node /tmp/node$i \
        --enable-tor \
        --daemon-listening-port $PORT \
        --ldk-peer-listening-port $PEER_PORT &
done

# Connect them in a mesh
# (Initialize, unlock, and connect each node to others)
```

### Testing Network Resilience

```bash
# Connect two nodes
# Kill Tor process
# Observe reconnection behavior
# Verify channel state maintained
```

## Debugging

### Enable Verbose Logging

Set environment variable before starting:
```bash
RUST_LOG=debug ./target/release/rgb-lightning-node /tmp/node --enable-tor
```

### Check Tor Logs

Logs will include:
- "Initializing Tor support..."
- "Tor client bootstrapped successfully"
- "Connecting to peer via Tor: <host>:<port>"
- "Successfully connected to peer via Tor"

### Common Debug Commands

```bash
# Check node status
curl http://localhost:3001/nodeinfo

# Check peers
curl http://localhost:3001/listpeers

# Check channels
curl http://localhost:3001/listchannels

# Check on-chain balance
curl http://localhost:3001/btcbalance

# Check Tor is enabled
# (Look for tor_manager initialization in logs)
```

## Automated Testing

For CI/CD integration:

```bash
# Run only non-network tests
cargo test --bin rgb-lightning-node tor::tests -- --test-threads=1

# Run with network tests (if allowed)
cargo test --bin rgb-lightning-node tor::tests -- --ignored --test-threads=1

# Use the test script
./test_tor_connection.sh --all
```

## Security Considerations

When testing with Tor:
- Use test mnemonics, not real funds
- Test environment should be isolated
- .onion addresses provide location privacy
- Always verify peer public keys before connecting
- Monitor for unexpected connections

## Further Reading

- [Arti Documentation](https://docs.rs/arti-client/)
- [Tor Project](https://www.torproject.org/)
- [Lightning Network Specification](https://github.com/lightning/bolts)
- [RGB Protocol Documentation](https://rgb.tech/)

## Support

For issues or questions:
- Check logs for detailed error messages
- Verify Bitcoin node connectivity
- Ensure Tor network is accessible
- Report issues at: https://github.com/RGB-Tools/rgb-lightning-node/issues
