use std::net::SocketAddr;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Config {
    /// The configuration for HTTP/1 and HTTP/2
    pub http: HttpConfig,
    /// The address to bind to for HTTP/3
    /// 
    /// If set, this will enable QUIC with HTTP/3
    pub http3_bind: Option<SocketAddr>,
}

/// The configuration for HTTP/1 and HTTP/2
#[derive(Debug, Clone, serde::Deserialize)]
pub struct HttpConfig {
    /// Whether to enable HTTP/1
    pub http1_enabled: bool,
    /// Whether to enable HTTP/2
    pub http2_enabled: bool,
    /// The address to bind to for insecure HTTP/1 and HTTP/2
    /// 
    /// If set, this will enable cleartext HTTP/1 and HTTP/2
    pub insecure_bind: Option<SocketAddr>,
    /// The address to bind to for secure HTTP/1 and HTTP/2
    /// 
    /// If set, this will enable HTTPS with HTTP/1 and HTTP/2
    pub secure_bind: Option<SocketAddr>,
}

impl Config {
    pub fn insecure_only(bind: SocketAddr) -> Self {
        Self {
            http: HttpConfig {
                http1_enabled: true,
                http2_enabled: true,
                insecure_bind: Some(bind),
                secure_bind: None,
            },
            http3_bind: None,
        }
    }

    pub fn secure_only(bind: SocketAddr) -> Self {
        Self {
            http: HttpConfig {
                http1_enabled: true,
                http2_enabled: true,
                insecure_bind: None,
                secure_bind: Some(bind),
            },
            http3_bind: Some(bind),
        }
    }

    pub fn all(insecure_bind: SocketAddr, secure_bind: SocketAddr) -> Self {
        Self {
            http: HttpConfig {
                http1_enabled: true,
                http2_enabled: true,
                insecure_bind: Some(insecure_bind),
                secure_bind: Some(secure_bind),
            },
            http3_bind: Some(secure_bind),
        }
    }

    pub fn alpn_protocols(&self) -> Vec<Vec<u8>> {
        // https://www.iana.org/assignments/tls-extensiontype-values/tls-extensiontype-values.xhtml#alpn-protocol-ids
        let mut protocols = Vec::new();

        if self.http.http1_enabled {
            // HTTP/1.0 and HTTP/1.1
            protocols.push(b"http/1.0".to_vec());
            protocols.push(b"http/1.1".to_vec());
        }

        if self.http.http2_enabled {
            if self.http.insecure_bind.is_some() {
                // HTTP/2 over cleartext TCP
                protocols.push(b"h2c".to_vec());
            }
            if self.http.secure_bind.is_some() {
                // HTTP/2 over TLS
                protocols.push(b"h2".to_vec());
            }
        }

        if self.http3_bind.is_some() {
            // HTTP/3
            protocols.push(b"h3".to_vec());
        }

        protocols
    }

    pub fn alt_svc(&self) -> http::HeaderValue {
        // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Alt-Svc
        let mut values = Vec::new();

        if self.http.http2_enabled {
            if let Some(addr) = self.http.insecure_bind {
                values.push(format!("h2c=\":{}\"; ma=3600", addr.port()));
            }
            if let Some(addr) = self.http.secure_bind {
                values.push(format!("h2=\":{}\"; ma=3600", addr.port()));
            }
        }

        if let Some(addr) = self.http3_bind {
            values.push(format!("h3=\":{}\"; ma=3600", addr.port()));
        }

        http::HeaderValue::from_str(&values.join(", ")).unwrap()
    }
}
