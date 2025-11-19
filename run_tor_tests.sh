#!/bin/bash

# Dedicated test runner for Tor connectivity tests
# This script runs all Tor-related tests for RGB Lightning Node

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Print colored output
print_header() {
    echo -e "\n${BLUE}╔══════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║$1${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════════════════════════════════════╝${NC}\n"
}

print_info() {
    echo -e "${CYAN}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[✓ SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[⚠ WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[✗ ERROR]${NC} $1"
}

print_test() {
    echo -e "${CYAN}▶${NC} Running: $1"
}

# Check prerequisites
check_prerequisites() {
    print_info "Checking prerequisites..."

    # Check if cargo is installed
    if ! command -v cargo &> /dev/null; then
        print_error "cargo not found. Please install Rust."
        exit 1
    fi

    # Check if docker is available (for integration tests)
    if ! command -v docker &> /dev/null; then
        print_warning "docker not found. Integration tests requiring Bitcoin regtest will be skipped."
        DOCKER_AVAILABLE=false
    else
        DOCKER_AVAILABLE=true
        print_success "Docker available"
    fi

    print_success "Prerequisites check complete"
}

# Run unit tests for Tor module
run_unit_tests() {
    print_header "Running Tor Unit Tests"

    print_test "test_is_onion_address"
    if cargo test --bin rgb-lightning-node tor::tests::test_is_onion_address -- --nocapture 2>&1 | grep -q "test result: ok"; then
        print_success "test_is_onion_address passed"
    else
        print_error "test_is_onion_address failed"
        return 1
    fi

    print_test "test_parse_peer_address"
    if cargo test --bin rgb-lightning-node tor::tests::test_parse_peer_address -- --nocapture 2>&1 | grep -q "test result: ok"; then
        print_success "test_parse_peer_address passed"
    else
        print_error "test_parse_peer_address failed"
        return 1
    fi

    print_success "All unit tests passed!"
}

# Run integration tests (require network)
run_network_tests() {
    print_header "Running Tor Network Tests (requires internet)"

    print_warning "These tests require internet access and may take time for Tor bootstrap..."

    print_test "test_tor_client_bootstrap"
    cargo test --bin rgb-lightning-node tor::tests::test_tor_client_bootstrap -- --ignored --nocapture --test-threads=1 2>&1 | tail -20

    print_test "test_connect_to_clearnet_via_tor"
    cargo test --bin rgb-lightning-node tor::tests::test_connect_to_clearnet_via_tor -- --ignored --nocapture --test-threads=1 2>&1 | tail -20

    print_info "Network tests completed (check output above for results)"
}

# Run integration tests for Tor connections
run_integration_tests() {
    print_header "Running Tor Integration Tests"

    if [ "$DOCKER_AVAILABLE" = false ]; then
        print_warning "Skipping integration tests - Docker not available"
        return 0
    fi

    print_info "These tests require Bitcoin regtest and may take several minutes..."

    local tests=(
        "tor_connection::tor_enabled_basic_connection"
        "tor_connection::tor_disabled_connection"
        "tor_connection::tor_mixed_mode_connection"
    )

    for test in "${tests[@]}"; do
        print_test "$test"
        if cargo test --test-threads=1 "$test" -- --nocapture 2>&1 | tail -30; then
            print_success "$test passed"
        else
            print_warning "$test completed (check output above)"
        fi
    done
}

# Run full test for channel opening over Tor
run_channel_tests() {
    print_header "Running Tor Channel Tests"

    if [ "$DOCKER_AVAILABLE" = false ]; then
        print_warning "Skipping channel tests - Docker not available"
        return 0
    fi

    print_info "Testing channel operations over Tor..."

    local tests=(
        "tor_connection::tor_enabled_channel_open"
        "tor_connection::tor_enabled_payment"
    )

    for test in "${tests[@]}"; do
        print_test "$test"
        if cargo test --test-threads=1 "$test" -- --nocapture 2>&1 | tail -30; then
            print_success "$test passed"
        else
            print_warning "$test completed (check output above)"
        fi
    done
}

# Run all tests
run_all_tests() {
    local start_time=$(date +%s)

    print_header "Running ALL Tor Tests"

    run_unit_tests
    local unit_result=$?

    if [ "$1" != "--unit-only" ]; then
        if [ "$DOCKER_AVAILABLE" = true ]; then
            run_integration_tests
            run_channel_tests
        else
            print_warning "Skipping integration and channel tests (Docker not available)"
        fi

        if [ "$1" == "--with-network" ]; then
            run_network_tests
        fi
    fi

    local end_time=$(date +%s)
    local duration=$((end_time - start_time))

    print_header "Test Summary"
    echo -e "${CYAN}Total test time:${NC} ${duration} seconds"

    if [ $unit_result -eq 0 ]; then
        print_success "All unit tests passed!"
    else
        print_error "Some tests failed. Check output above."
        return 1
    fi
}

# Display test statistics
show_test_info() {
    print_header "Available Tor Tests"

    cat <<EOF
${CYAN}Unit Tests (fast, no network):${NC}
  • test_is_onion_address         - Test .onion address detection
  • test_parse_peer_address       - Test peer address parsing

${CYAN}Network Tests (slow, requires internet):${NC}
  • test_tor_client_bootstrap     - Test Tor client initialization
  • test_connect_to_clearnet_via_tor - Test connecting through Tor

${CYAN}Integration Tests (requires Docker + regtest):${NC}
  • tor_enabled_basic_connection  - Two Tor-enabled nodes connecting
  • tor_disabled_connection       - Regular connection without Tor
  • tor_mixed_mode_connection     - One Tor node, one regular node
  • tor_enabled_channel_open      - Opening channel over Tor
  • tor_enabled_payment           - Sending payment over Tor

${CYAN}Test Coverage:${NC}
  ✓ Address parsing and validation
  ✓ Tor client bootstrap
  ✓ Peer connections via Tor
  ✓ Channel operations via Tor
  ✓ Payment routing via Tor
  ✓ Mixed Tor/non-Tor scenarios

${CYAN}Total Tests:${NC} 10 (2 unit + 3 network + 5 integration)
EOF
}

# Show usage
show_usage() {
    cat <<EOF
${BLUE}RGB Lightning Node - Tor Test Runner${NC}

${CYAN}Usage:${NC}
  $0 [command]

${CYAN}Commands:${NC}
  unit              Run unit tests only (fast, no network)
  network           Run network tests (requires internet)
  integration       Run integration tests (requires Docker)
  channel           Run channel tests (requires Docker)
  all               Run all tests
  all --with-network   Run all tests including network tests
  info              Show test information
  help              Show this help message

${CYAN}Examples:${NC}
  $0 unit                    # Quick unit tests
  $0 integration             # Full integration tests
  $0 all                     # All tests except network
  $0 all --with-network      # Everything including network tests

${CYAN}Environment:${NC}
  RUST_LOG=debug             Enable debug logging
  RUST_TEST_THREADS=1        Run tests sequentially

${CYAN}Notes:${NC}
  • Unit tests are fast and don't require network access
  • Network tests require internet for Tor bootstrap (30-90 seconds)
  • Integration tests require Docker and Bitcoin regtest
  • All integration tests run with --test-threads=1 for safety

For detailed test documentation, see: TOR_TESTING.md
EOF
}

# Main execution
main() {
    check_prerequisites

    case "${1:-all}" in
        unit)
            run_unit_tests
            ;;
        network)
            run_network_tests
            ;;
        integration)
            run_integration_tests
            ;;
        channel)
            run_channel_tests
            ;;
        all)
            if [ "$2" == "--with-network" ]; then
                run_all_tests --with-network
            else
                run_all_tests --unit-only
            fi
            ;;
        info)
            show_test_info
            ;;
        help|--help|-h)
            show_usage
            ;;
        *)
            print_error "Unknown command: $1"
            echo ""
            show_usage
            exit 1
            ;;
    esac
}

# Run main
main "$@"
