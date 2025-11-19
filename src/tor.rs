use arti_client::{TorClient, TorClientConfig};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tor_rtcompat::PreferredRuntime;
use tracing::{debug, info, warn};

use crate::error::APIError;

/// Tor connection manager that handles Tor client lifecycle and connections
pub struct TorConnectionManager {
    tor_client: Arc<TorClient<PreferredRuntime>>,
    socks_port: Option<u16>,
}

impl TorConnectionManager {
    /// Create a new Tor connection manager
    pub async fn new(
        _tor_data_dir: Option<PathBuf>,
        socks_port: Option<u16>,
    ) -> Result<Self, APIError> {
        info!("Initializing Tor connection manager");

        // Build Tor client with tokio runtime using default config
        let config = TorClientConfig::default();

        let tor_client = TorClient::create_bootstrapped(config)
            .await
            .map_err(|e| {
                APIError::InvalidPeerInfo(format!("Failed to bootstrap Tor client: {}", e))
            })?;

        info!("Tor client bootstrapped successfully");

        Ok(Self {
            tor_client: Arc::new(tor_client),
            socks_port,
        })
    }

    /// Connect to a peer through Tor
    /// Supports both .onion addresses and regular addresses
    pub async fn connect_through_tor(
        &self,
        host: &str,
        port: u16,
    ) -> Result<TcpStream, APIError> {
        debug!("Connecting through Tor to {}:{}", host, port);

        // For .onion addresses, use Arti directly
        if host.ends_with(".onion") {
            self.connect_to_onion(host, port).await
        } else if let Some(socks_port) = self.socks_port {
            // For regular addresses, use SOCKS proxy if configured
            self.connect_via_socks(host, port, socks_port).await
        } else {
            // Use Arti's direct connection for regular addresses too
            self.connect_direct_via_tor(host, port).await
        }
    }

    /// Connect to a .onion hidden service
    async fn connect_to_onion(&self, host: &str, port: u16) -> Result<TcpStream, APIError> {
        debug!("Connecting to onion service: {}:{}", host, port);

        let stream = self
            .tor_client
            .connect((host, port))
            .await
            .map_err(|e| {
                APIError::FailedPeerConnection
            })?;

        // Convert Arti's DataStream to TcpStream
        // Note: Arti's DataStream implements AsyncRead + AsyncWrite
        // We need to wrap it in a way compatible with TcpStream
        self.wrap_tor_stream(stream).await
    }

    /// Connect to a regular address through Tor's circuit
    async fn connect_direct_via_tor(
        &self,
        host: &str,
        port: u16,
    ) -> Result<TcpStream, APIError> {
        debug!("Connecting directly via Tor to {}:{}", host, port);

        let stream = self
            .tor_client
            .connect((host, port))
            .await
            .map_err(|e| {
                warn!("Failed to connect via Tor: {}", e);
                APIError::FailedPeerConnection
            })?;

        self.wrap_tor_stream(stream).await
    }

    /// Connect via SOCKS proxy
    async fn connect_via_socks(
        &self,
        host: &str,
        port: u16,
        socks_port: u16,
    ) -> Result<TcpStream, APIError> {
        debug!(
            "Connecting via SOCKS proxy (port {}) to {}:{}",
            socks_port, host, port
        );

        let proxy_addr = format!("127.0.0.1:{}", socks_port);
        let target_addr = format!("{}:{}", host, port);

        let stream = tokio_socks::tcp::Socks5Stream::connect(proxy_addr.as_str(), target_addr.as_str())
            .await
            .map_err(|e| {
                warn!("Failed to connect via SOCKS proxy: {}", e);
                APIError::FailedPeerConnection
            })?;

        Ok(stream.into_inner())
    }

    /// Wrap Arti's DataStream into a TcpStream-compatible stream
    /// Since Arti's DataStream implements AsyncRead + AsyncWrite, we need to
    /// create a compatible stream for LDK by creating a local TCP loopback
    async fn wrap_tor_stream<S>(&self, mut stream: S) -> Result<TcpStream, APIError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        // Create a loopback TCP listener
        let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|_| APIError::FailedPeerConnection)?;
        let addr = listener.local_addr().map_err(|_| APIError::FailedPeerConnection)?;
        listener.set_nonblocking(true).map_err(|_| APIError::FailedPeerConnection)?;

        // Convert to tokio listener
        let listener = tokio::net::TcpListener::from_std(listener).map_err(|_| APIError::FailedPeerConnection)?;

        // Connect to ourselves
        let client_stream = TcpStream::connect(addr).await.map_err(|_| APIError::FailedPeerConnection)?;

        // Accept the connection
        let (mut server_stream, _) = listener.accept().await.map_err(|_| APIError::FailedPeerConnection)?;

        // Proxy between Tor stream and server side of the loopback
        tokio::spawn(async move {
            if let Err(e) = tokio::io::copy_bidirectional(&mut stream, &mut server_stream).await {
                debug!("Tor proxy ended: {}", e);
            }
        });

        Ok(client_stream)
    }

    /// Proxy data bidirectionally between two streams
    async fn proxy_streams<S1, S2>(mut stream1: S1, mut stream2: S2) -> std::io::Result<()>
    where
        S1: AsyncRead + AsyncWrite + Unpin,
        S2: AsyncRead + AsyncWrite + Unpin,
    {
        tokio::io::copy_bidirectional(&mut stream1, &mut stream2).await?;
        Ok(())
    }

    /// Check if an address is a Tor hidden service (.onion)
    pub fn is_onion_address(host: &str) -> bool {
        host.ends_with(".onion")
    }

    /// Get the Tor client instance
    pub fn client(&self) -> Arc<TorClient<PreferredRuntime>> {
        Arc::clone(&self.tor_client)
    }
}

/// Parse a peer address string that may contain .onion addresses
/// Format: pubkey@host:port
pub fn parse_peer_address(peer_info: &str) -> Result<(String, u16), APIError> {
    let parts: Vec<&str> = peer_info.split('@').collect();
    if parts.len() != 2 {
        return Err(APIError::FailedPeerConnection);
    }

    let addr_parts: Vec<&str> = parts[1].split(':').collect();
    if addr_parts.len() != 2 {
        return Err(APIError::FailedPeerConnection);
    }

    let host = addr_parts[0].to_string();
    let port = addr_parts[1].parse::<u16>().map_err(|_| {
        APIError::InvalidPeerInfo("Invalid port in peer address".to_string())
    })?;

    Ok((host, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_onion_address() {
        assert!(TorConnectionManager::is_onion_address(
            "test123abc.onion"
        ));
        assert!(TorConnectionManager::is_onion_address(
            "alonghiddenserviceaddress123456.onion"
        ));
        assert!(!TorConnectionManager::is_onion_address("example.com"));
        assert!(!TorConnectionManager::is_onion_address("192.168.1.1"));
    }

    #[test]
    fn test_parse_peer_address() {
        let result = parse_peer_address("pubkey@test.onion:9735");
        assert!(result.is_ok());
        let (host, port) = result.unwrap();
        assert_eq!(host, "test.onion");
        assert_eq!(port, 9735);

        let result = parse_peer_address("pubkey@192.168.1.1:9735");
        assert!(result.is_ok());
        let (host, port) = result.unwrap();
        assert_eq!(host, "192.168.1.1");
        assert_eq!(port, 9735);
    }
}
