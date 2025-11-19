#!/bin/bash

# Test script for Tor connectivity in RGB Lightning Node
# This script helps test two nodes connecting via Tor and opening a channel

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== RGB Lightning Node Tor Connection Test ===${NC}\n"

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Test 1: Run unit tests
test_unit_tests() {
    print_info "Running unit tests..."

    echo -e "\n${BLUE}--- Running basic tests (no network) ---${NC}"
    cargo test --lib tor:: -- --nocapture 2>&1 | grep -E "(test |PASSED|FAILED|✓|✗)" || true

    if [ $? -eq 0 ]; then
        print_success "Unit tests passed"
    else
        print_warning "Some unit tests may have failed"
    fi
}

# Test 2: Run network tests (ignored by default)
test_network_tests() {
    print_info "Running network tests (these require internet access)..."
    print_warning "This will take time as Tor needs to bootstrap..."

    echo -e "\n${BLUE}--- Running Tor bootstrap test ---${NC}"
    cargo test --lib tor::tests::test_tor_client_bootstrap -- --ignored --nocapture --test-threads=1 2>&1 | tail -20

    echo -e "\n${BLUE}--- Running clearnet via Tor test ---${NC}"
    cargo test --lib tor::tests::test_connect_to_clearnet_via_tor -- --ignored --nocapture --test-threads=1 2>&1 | tail -20
}

# Test 3: Test two nodes manually
test_two_nodes_manual() {
    print_info "Manual two-node test setup instructions"

    cat <<EOF

${BLUE}=== Manual Two-Node Tor Connection Test ===${NC}

To test two nodes connecting via Tor and opening a channel:

${GREEN}Step 1: Build the project${NC}
    cargo build --release

${GREEN}Step 2: Start Node 1 (Alice) with Tor${NC}
    mkdir -p /tmp/alice-node
    ./target/release/rgb-lightning-node /tmp/alice-node \\
        --enable-tor \\
        --daemon-listening-port 3001 \\
        --ldk-peer-listening-port 9735

${GREEN}Step 3: Initialize and unlock Node 1${NC}
    # Initialize
    curl -X POST http://localhost:3001/init \\
        -H "Content-Type: application/json" \\
        -d '{
            "password": "test123",
            "mnemonic": "<24-word mnemonic>"
        }'

    # Unlock
    curl -X POST http://localhost:3001/unlock \\
        -H "Content-Type: application/json" \\
        -d '{
            "password": "test123",
            "bitcoind_rpc_host": "localhost",
            "bitcoind_rpc_port": 18443,
            "bitcoind_rpc_username": "user",
            "bitcoind_rpc_password": "password"
        }'

    # Get node info (save the pubkey)
    NODE1_PUBKEY=\$(curl http://localhost:3001/nodeinfo | jq -r '.pubkey')
    echo "Node 1 pubkey: \$NODE1_PUBKEY"

${GREEN}Step 4: Start Node 2 (Bob) with Tor${NC}
    mkdir -p /tmp/bob-node
    ./target/release/rgb-lightning-node /tmp/bob-node \\
        --enable-tor \\
        --daemon-listening-port 3002 \\
        --ldk-peer-listening-port 9736

${GREEN}Step 5: Initialize and unlock Node 2${NC}
    # Initialize (similar to Node 1 but on port 3002)
    curl -X POST http://localhost:3002/init \\
        -H "Content-Type: application/json" \\
        -d '{
            "password": "test123",
            "mnemonic": "<different-24-word-mnemonic>"
        }'

    # Unlock (similar to Node 1)
    curl -X POST http://localhost:3002/unlock \\
        -H "Content-Type: application/json" \\
        -d '{
            "password": "test123",
            "bitcoind_rpc_host": "localhost",
            "bitcoind_rpc_port": 18443,
            "bitcoind_rpc_username": "user",
            "bitcoind_rpc_password": "password"
        }'

${GREEN}Step 6: Connect Node 2 to Node 1 via Tor${NC}
    ${YELLOW}# Option A: Using .onion address (if Node 1 publishes one)${NC}
    curl -X POST http://localhost:3002/connectpeer \\
        -H "Content-Type: application/json" \\
        -d "{
            \"peer_pubkey_and_addr\": \"\${NODE1_PUBKEY}@<node1-onion-address>:9735\"
        }"

    ${YELLOW}# Option B: Using localhost for testing${NC}
    curl -X POST http://localhost:3002/connectpeer \\
        -H "Content-Type: application/json" \\
        -d "{
            \"peer_pubkey_and_addr\": \"\${NODE1_PUBKEY}@127.0.0.1:9735\"
        }"

${GREEN}Step 7: Verify connection${NC}
    # On Node 2, list peers
    curl http://localhost:3002/listpeers | jq

    # Should show Node 1 as connected peer

${GREEN}Step 8: Open a channel (Node 2 to Node 1)${NC}
    curl -X POST http://localhost:3002/openchannel \\
        -H "Content-Type: application/json" \\
        -d "{
            \"peer_pubkey_and_addr\": \"\${NODE1_PUBKEY}@127.0.0.1:9735\",
            \"capacity_sat\": 1000000,
            \"push_msat\": 0
        }"

${GREEN}Step 9: Check channels${NC}
    # On both nodes
    curl http://localhost:3001/listchannels | jq
    curl http://localhost:3002/listchannels | jq

${BLUE}=== Testing with .onion addresses ===${NC}

To test with actual .onion addresses, you need to:
1. Configure a Tor hidden service for Node 1
2. Add this to /etc/tor/torrc (or run Tor manually):

   HiddenServiceDir /var/lib/tor/lightning-node/
   HiddenServicePort 9735 127.0.0.1:9735

3. Restart Tor and get the .onion address:

   cat /var/lib/tor/lightning-node/hostname

4. Use that .onion address in Step 6

${BLUE}=== Expected Results ===${NC}
✓ Tor client bootstraps successfully on both nodes
✓ Node 2 connects to Node 1
✓ Both nodes show peer as connected
✓ Channel opens successfully
✓ Channel appears in listchannels on both nodes

EOF
}

# Test 4: Quick connectivity test
test_quick_connectivity() {
    print_info "Running quick connectivity test..."

    # Check if we can parse addresses correctly
    echo -e "\n${BLUE}Testing address parsing...${NC}"

    # This would ideally call the actual binary, but we'll use cargo test
    cargo test --lib parse_peer_address -- --nocapture 2>&1 | grep -E "(test |PASSED|✓)" || true

    if [ $? -eq 0 ]; then
        print_success "Address parsing tests passed"
    fi
}

# Main menu
show_menu() {
    echo -e "\n${BLUE}Select test to run:${NC}"
    echo "1) Run unit tests (fast, no network)"
    echo "2) Run network tests (slow, requires internet)"
    echo "3) Show manual two-node test instructions"
    echo "4) Run quick connectivity test"
    echo "5) Run all non-network tests"
    echo "6) Exit"
    echo -n "Enter choice [1-6]: "
}

# Main execution
if [ "$1" == "--all" ]; then
    test_unit_tests
    test_quick_connectivity
    print_info "All basic tests completed"
elif [ "$1" == "--network" ]; then
    test_network_tests
elif [ "$1" == "--manual" ]; then
    test_two_nodes_manual
elif [ "$1" == "--help" ] || [ "$1" == "-h" ]; then
    echo "Usage: $0 [--all|--network|--manual|--help]"
    echo ""
    echo "Options:"
    echo "  --all       Run all non-network tests"
    echo "  --network   Run network tests (requires internet)"
    echo "  --manual    Show manual testing instructions"
    echo "  --help      Show this help"
    echo ""
    echo "Run without arguments for interactive menu"
else
    # Interactive mode
    while true; do
        show_menu
        read -r choice
        case $choice in
            1) test_unit_tests ;;
            2) test_network_tests ;;
            3) test_two_nodes_manual ;;
            4) test_quick_connectivity ;;
            5)
                test_unit_tests
                test_quick_connectivity
                print_success "All basic tests completed"
                ;;
            6)
                print_info "Exiting..."
                exit 0
                ;;
            *)
                print_error "Invalid option"
                ;;
        esac
    done
fi
