use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::bytesio::BytesIO;

#[tokio::test]
async fn test_bytes_io() {
	let (pipe1, mut pipe2) = tokio::io::duplex(1024);
	let mut bytesio = BytesIO::new(Box::new(pipe1));

	bytesio.write(Bytes::from_static(b"hello world")).await.unwrap();

	let mut buf = vec![0; 11];
	pipe2.read_exact(&mut buf).await.unwrap();
	assert_eq!(buf, b"hello world".to_vec());

	pipe2.write_all(b"hello bytesio").await.unwrap();

	let buf = bytesio.read().await.unwrap();
	assert_eq!(buf.to_vec(), b"hello bytesio".to_vec());
}
