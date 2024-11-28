pub async fn true_bind(addr: std::net::SocketAddr) -> std::io::Result<std::net::SocketAddr> {
	if addr.port() == 0 {
		let bind = tokio::net::TcpListener::bind(addr).await?;
		bind.local_addr()
	} else {
		Ok(addr)
	}
}
