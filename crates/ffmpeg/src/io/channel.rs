use std::sync::{Arc, Mutex};

use bytes::{Buf, BytesMut};

/// A wrapper around a channel that implements `std::io::Read` and
/// `std::io::Write`. The wrapper allows for the channel to be used with the
/// `Input` and `Output` structs.
#[derive(Debug, Clone)]
pub struct ChannelCompat<T: Send> {
    /// I am unsure if the mutex is needed here. I do not think it is, but I am
    /// not sure. FFmpeg might require the IO to be synchronized, but I do not
    /// think it does.
    inner: Arc<Mutex<T>>,
    buffer: BytesMut,
}

impl<T: Send> ChannelCompat<T> {
    /// Creates a new `ChannelCompat` instance.
    pub fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
            buffer: BytesMut::new(),
        }
    }
}

/// A trait that represents a channel that can be read from.
pub trait ChannelCompatRecv: Send {
    /// The type of data that the channel can receive.
    type Data: AsRef<[u8]>;

    /// Receives data from the channel.
    fn channel_recv(&mut self) -> Option<Self::Data>;

    /// Tries to receive data from the channel.
    fn try_channel_recv(&mut self) -> Option<Self::Data>;

    /// Converts the channel to a `ChannelCompat` instance.
    fn into_compat(self) -> ChannelCompat<Self>
    where
        Self: Sized,
    {
        ChannelCompat::new(self)
    }
}

/// A trait that represents a channel that can be written to.
pub trait ChannelCompatSend: Send {
    /// The type of data that the channel can send.
    type Data: From<Vec<u8>>;

    /// Sends data to the channel.
    fn channel_send(&mut self, data: Self::Data) -> bool;

    /// Converts the channel to a `ChannelCompat` instance.
    fn into_compat(self) -> ChannelCompat<Self>
    where
        Self: Sized,
    {
        ChannelCompat::new(self)
    }
}

#[cfg(feature = "tokio-channel")]
impl<D: AsRef<[u8]> + Send> ChannelCompatRecv for tokio::sync::mpsc::Receiver<D> {
    type Data = D;

    fn channel_recv(&mut self) -> Option<Self::Data> {
        self.blocking_recv()
    }

    fn try_channel_recv(&mut self) -> Option<Self::Data> {
        self.try_recv().ok()
    }
}

#[cfg(feature = "tokio-channel")]
impl<D: From<Vec<u8>> + Send> ChannelCompatSend for tokio::sync::mpsc::Sender<D> {
    type Data = D;

    fn channel_send(&mut self, data: Self::Data) -> bool {
        self.blocking_send(data).is_ok()
    }
}

#[cfg(feature = "tokio-channel")]
impl<D: AsRef<[u8]> + Send> ChannelCompatRecv for tokio::sync::mpsc::UnboundedReceiver<D> {
    type Data = D;

    fn channel_recv(&mut self) -> Option<Self::Data> {
        self.blocking_recv()
    }

    fn try_channel_recv(&mut self) -> Option<Self::Data> {
        self.try_recv().ok()
    }
}

#[cfg(feature = "tokio-channel")]
impl<D: From<Vec<u8>> + Send> ChannelCompatSend for tokio::sync::mpsc::UnboundedSender<D> {
    type Data = D;

    fn channel_send(&mut self, data: Self::Data) -> bool {
        self.send(data).is_ok()
    }
}

#[cfg(feature = "tokio-channel")]
impl<D: AsRef<[u8]> + Clone + Send> ChannelCompatRecv for tokio::sync::broadcast::Receiver<D> {
    type Data = D;

    fn channel_recv(&mut self) -> Option<Self::Data> {
        self.blocking_recv().ok()
    }

    fn try_channel_recv(&mut self) -> Option<Self::Data> {
        self.try_recv().ok()
    }
}

#[cfg(feature = "tokio-channel")]
impl<D: From<Vec<u8>> + Clone + Send> ChannelCompatSend for tokio::sync::broadcast::Sender<D> {
    type Data = D;

    fn channel_send(&mut self, data: Self::Data) -> bool {
        self.send(data).is_ok()
    }
}

#[cfg(feature = "crossbeam-channel")]
impl<D: AsRef<[u8]> + Send> ChannelCompatRecv for crossbeam_channel::Receiver<D> {
    type Data = D;

    fn channel_recv(&mut self) -> Option<Self::Data> {
        self.recv().ok()
    }

    fn try_channel_recv(&mut self) -> Option<Self::Data> {
        self.try_recv().ok()
    }
}

#[cfg(feature = "crossbeam-channel")]
impl<D: From<Vec<u8>> + Send> ChannelCompatSend for crossbeam_channel::Sender<D> {
    type Data = D;

    fn channel_send(&mut self, data: Self::Data) -> bool {
        self.send(data).is_ok()
    }
}

impl<D: AsRef<[u8]> + Send> ChannelCompatRecv for std::sync::mpsc::Receiver<D> {
    type Data = D;

    fn channel_recv(&mut self) -> Option<Self::Data> {
        self.recv().ok()
    }

    fn try_channel_recv(&mut self) -> Option<Self::Data> {
        self.try_recv().ok()
    }
}

impl<D: From<Vec<u8>> + Send> ChannelCompatSend for std::sync::mpsc::Sender<D> {
    type Data = D;

    fn channel_send(&mut self, data: Self::Data) -> bool {
        self.send(data).is_ok()
    }
}

impl<D: From<Vec<u8>> + Send> ChannelCompatSend for std::sync::mpsc::SyncSender<D> {
    type Data = D;

    fn channel_send(&mut self, data: Self::Data) -> bool {
        self.send(data).is_ok()
    }
}

impl<T: ChannelCompatRecv> std::io::Read for ChannelCompat<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.buffer.len() >= buf.len() {
            buf.copy_from_slice(&self.buffer[..buf.len()]);
            self.buffer.advance(buf.len());
            return Ok(buf.len());
        }

        let mut inner = self.inner.lock().unwrap();

        let mut total_read = 0;
        if self.buffer.is_empty() {
            let Some(data) = inner.channel_recv() else {
                return Ok(0);
            };

            let data = data.as_ref();
            let min = data.len().min(buf.len());

            buf.copy_from_slice(&data[..min]);
            self.buffer.extend_from_slice(&data[min..]);
            total_read += min;
        } else {
            buf[..self.buffer.len()].copy_from_slice(&self.buffer);
            total_read += self.buffer.len();
            self.buffer.clear();
        }

        while let Some(Some(data)) = (total_read < buf.len()).then(|| inner.try_channel_recv()) {
            let data = data.as_ref();
            let min = data.len().min(buf.len() - total_read);
            buf[total_read..total_read + min].copy_from_slice(&data[..min]);
            self.buffer.extend_from_slice(&data[min..]);
            total_read += min;
        }

        Ok(total_read)
    }
}

impl<T: ChannelCompatSend> std::io::Write for ChannelCompat<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if !self.inner.lock().unwrap().channel_send(buf.to_vec().into()) {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Unexpected EOF"));
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::io::{Read, Write};

    use rand::distributions::Standard;
    use rand::{thread_rng, Rng};

    use crate::io::channel::{ChannelCompat, ChannelCompatRecv, ChannelCompatSend};

    macro_rules! make_test {
        (
            $(
                $(
                    #[variant($name:ident, $channel:expr$(, cfg($($cfg_meta:meta)*))?)]
                )*
                |$tx:ident, $rx:ident| $body:block
            )*
        ) => {
            $(
                $(
                    #[test]
                    $(#[cfg($($cfg_meta)*)])?
                    fn $name() {
                        let ($tx, $rx) = $channel;
                        $body
                    }
                )*
            )*
        };
    }

    // test 1000 byte read
    make_test! {
        #[variant(
            test_read_std_mpsc,
            std::sync::mpsc::channel::<Vec<u8>>()
        )]
        #[variant(
            test_read_std_sync_mpsc,
            std::sync::mpsc::sync_channel::<Vec<u8>>(1)
        )]
        #[variant(
            test_read_tokio_mpsc,
            tokio::sync::mpsc::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_tokio_unbounded,
            tokio::sync::mpsc::unbounded_channel::<Vec<u8>>(),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_tokio_broadcast,
            tokio::sync::broadcast::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_crossbeam_unbounded,
            crossbeam_channel::unbounded::<Vec<u8>>(),
            cfg(feature = "crossbeam-channel")
        )]
        |tx, rx| {
            let mut reader = rx.into_compat();

            // generate 1000 bytes of random data
            let mut rng = thread_rng();
            let data: Vec<u8> = (0..1000).map(|_| rng.sample(Standard)).collect();

            let mut tx = tx;
            let write_result = tx.channel_send(data.clone());
            assert!(write_result);

            // read 1000 bytes
            let mut buffer = vec![0u8; 1000];
            let read_result = reader.read(&mut buffer);
            assert!(read_result.is_ok());
            assert_eq!(read_result.unwrap(), data.len());

            // data read must match data written
            assert_eq!(buffer, data);
        }
    }

    // test 1000 byte write
    make_test! {
        #[variant(
            test_write_std_mpsc,
            std::sync::mpsc::channel::<Vec<u8>>()
        )]
        #[variant(
            test_write_std_sync_mpsc,
            std::sync::mpsc::sync_channel::<Vec<u8>>(1)
        )]
        #[variant(
            test_write_tokio_mpsc,
            tokio::sync::mpsc::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_write_tokio_unbounded,
            tokio::sync::mpsc::unbounded_channel::<Vec<u8>>(),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_write_tokio_broadcast,
            tokio::sync::broadcast::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_write_crossbeam_unbounded,
            crossbeam_channel::unbounded::<Vec<u8>>(),
            cfg(feature = "crossbeam-channel")
        )]
        |tx, rx| {
            let mut writer = tx.into_compat();

        // generate 1000 bytes of random data
        let mut rng = thread_rng();
        let data: Vec<u8> = (0..1000).map(|_| rng.sample(Standard)).collect();

        let write_result = writer.write(&data);
        assert!(write_result.is_ok(), "Failed to write data to the channel");
        assert_eq!(write_result.unwrap(), data.len(), "Written byte count mismatch");

        // read 1000 bytes
        let mut rx = rx;
        let read_result = rx.channel_recv();
        assert!(read_result.is_some(), "No data received from the channel");

        let received_data = read_result.unwrap();
        assert_eq!(received_data.len(), data.len(), "Received byte count mismatch");

        // data read must match data written
        assert_eq!(
            received_data, data,
            "Mismatch between written data and received data"
        );
        }
    }

    // test read with smaller buffer than data
    make_test! {
        #[variant(
            test_read_smaller_buffer_than_data_std_mpsc,
            std::sync::mpsc::channel::<Vec<u8>>()
        )]
        #[variant(
            test_read_smaller_buffer_than_data_std_sync_mpsc,
            std::sync::mpsc::sync_channel::<Vec<u8>>(1)
        )]
        #[variant(
            test_read_smaller_buffer_than_data_tokio_mpsc,
            tokio::sync::mpsc::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_smaller_buffer_than_data_tokio_unbounded,
            tokio::sync::mpsc::unbounded_channel::<Vec<u8>>(),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_smaller_buffer_than_data_tokio_broadcast,
            tokio::sync::broadcast::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_smaller_buffer_than_data_crossbeam_unbounded,
            crossbeam_channel::unbounded::<Vec<u8>>(),
            cfg(feature = "crossbeam-channel")
        )]
        |tx, rx| {
            let mut reader = ChannelCompat::new(rx);
            let data = b"PartialReadTest".to_vec();
            let mut tx = tx;
            let send_result = tx.channel_send(data);
            assert!(send_result);

            let mut buffer = vec![0u8; 7]; // buffer.len() < data.len()
            let read_result = reader.read(&mut buffer);
            assert!(read_result.is_ok());
            assert_eq!(read_result.unwrap(), buffer.len());
            assert_eq!(&buffer, b"Partial");

            // Read the remaining part of the data
            let mut buffer = vec![0u8; 8];
            let read_result = reader.read(&mut buffer);
            assert!(read_result.is_ok());
            assert_eq!(read_result.unwrap(), buffer.len());
            assert_eq!(&buffer, b"ReadTest");
        }
    }

    // test read with no data
    make_test! {
        #[variant(
            test_read_no_data_std_mpsc,
            std::sync::mpsc::channel::<Vec<u8>>()
        )]
        #[variant(
            test_read_no_data_std_sync_mpsc,
            std::sync::mpsc::sync_channel::<Vec<u8>>(1)
        )]
        #[variant(
            test_read_no_data_tokio_mpsc,
            tokio::sync::mpsc::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_no_data_tokio_unbounded,
            tokio::sync::mpsc::unbounded_channel::<Vec<u8>>(),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_no_data_tokio_broadcast,
            tokio::sync::broadcast::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_no_data_crossbeam_unbounded,
            crossbeam_channel::unbounded::<Vec<u8>>(),
            cfg(feature = "crossbeam-channel")
        )]
        |tx, rx| {
            let mut reader = ChannelCompat::new(rx);

            // no data is sent to the channel + drop tx to prevent it from blocking
            drop(tx);
            let mut buffer = vec![0u8; 10];
            let read_result = reader.read(&mut buffer);

            assert!(read_result.is_ok());
            assert_eq!(read_result.unwrap(), 0);
        }
    }

    // test read non-empty buffer after initial read to catch else
    make_test! {
        #[variant(
            test_read_else_case_std_mpsc,
            std::sync::mpsc::channel::<Vec<u8>>()
        )]
        #[variant(
            test_read_else_case_std_sync_mpsc,
            std::sync::mpsc::sync_channel::<Vec<u8>>(1)
        )]
        #[variant(
            test_read_else_case_tokio_mpsc,
            tokio::sync::mpsc::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_else_case_tokio_unbounded,
            tokio::sync::mpsc::unbounded_channel::<Vec<u8>>(),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_else_case_tokio_broadcast,
            tokio::sync::broadcast::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_else_case_crossbeam_unbounded,
            crossbeam_channel::unbounded::<Vec<u8>>(),
            cfg(feature = "crossbeam-channel")
        )]
        |tx, rx| {
            let mut reader = ChannelCompat::new(rx);
            let mut tx = tx;

            let data1 = b"FirstChunk".to_vec();
            let write_result1 = tx.channel_send(data1);
            assert!(write_result1, "Failed to send data1");

            // read the first part of the data ("First")
            let mut buffer = vec![0u8; 5];
            let read_result = reader.read(&mut buffer);
            assert!(read_result.is_ok(), "Failed to read the first chunk");
            let bytes_read = read_result.unwrap();
            assert_eq!(bytes_read, buffer.len(), "Mismatch in first chunk read size");
            assert_eq!(&buffer, b"First", "Buffer content mismatch for first part of FirstChunk");

            // read the remaining part of data1 ("Chunk") and part of data2 which hasn't been written yet ("Secon")
            let mut buffer = vec![0u8; 10];
            let read_result = reader.read(&mut buffer);
            assert!(read_result.is_ok(), "Failed to read the next 10 bytes");
            let bytes_read = read_result.unwrap();

            // validate that the buffer contains "Chunk" at this point
            assert_eq!(bytes_read, 5, "Unexpected read size for the next part");
            assert_eq!(&buffer[..bytes_read], b"Chunk", "Buffer content mismatch for combined reads");

            // Write second chunk of data ("SecondChunk")
            let data2 = b"SecondChunk".to_vec();
            let write_result2 = tx.channel_send(data2);
            assert!(write_result2, "Failed to send data2");

            // verify that there's leftover data from data2
            let mut buffer = vec![0u8; 5];
            let read_result = reader.read(&mut buffer);
            assert!(read_result.is_ok(), "Failed to read leftover data from data2");
            let bytes_read = read_result.unwrap();
            assert!(bytes_read > 0, "No leftover data from data2 was available");
        }
    }

    // test read to hit the while loop
    make_test! {
        #[variant(
            test_read_while_case_std_mpsc,
            std::sync::mpsc::channel::<Vec<u8>>()
        )]
        #[variant(
            test_read_while_case_std_sync_mpsc,
            std::sync::mpsc::sync_channel::<Vec<u8>>(1)
        )]
        #[variant(
            test_read_while_case_tokio_mpsc,
            tokio::sync::mpsc::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_while_case_tokio_unbounded,
            tokio::sync::mpsc::unbounded_channel::<Vec<u8>>(),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_while_case_tokio_broadcast,
            tokio::sync::broadcast::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_read_while_case_crossbeam_unbounded,
            crossbeam_channel::unbounded::<Vec<u8>>(),
            cfg(feature = "crossbeam-channel")
        )]
        |tx, rx| {
            let mut reader = ChannelCompat::new(rx);
            let mut tx = tx;

            let data1 = b"FirstChunk".to_vec();
            let write_result1 = tx.channel_send(data1);
            assert!(write_result1, "Failed to send data1");

            // read "First"
            let mut buffer = vec![0u8; 5];
            let read_result = reader.read(&mut buffer);
            assert!(read_result.is_ok(), "Failed to read the first chunk");
            let bytes_read = read_result.unwrap();
            assert_eq!(bytes_read, buffer.len(), "Mismatch in first chunk read size");
            assert_eq!(&buffer, b"First", "Buffer content mismatch for first part of FirstChunk");

            // write "SecondChunk"
            let data2 = b"SecondChunk".to_vec();
            let write_result2 = tx.channel_send(data2);
            assert!(write_result2, "Failed to send data2");

            // read "ChunkSecon"
            let mut buffer = vec![0u8; 10];
            let read_result = reader.read(&mut buffer);
            assert!(read_result.is_ok(), "Failed to read the next chunk of data");
            let bytes_read = read_result.unwrap();
            assert!(bytes_read > 0, "No data was read");
            assert_eq!(&buffer[..bytes_read], b"ChunkSecon", "Buffer content mismatch");

            // continue reading to enter the while loop
            let mut buffer = vec![0u8; 6];
            let read_result = reader.read(&mut buffer);
            assert!(read_result.is_ok(), "Failed to read remaining data");
            let bytes_read = read_result.unwrap();
            assert!(bytes_read > 0, "No additional data was read");
            assert_eq!(&buffer[..bytes_read], b"dChunk", "Buffer content mismatch for remaining data");
        }
    }

    // test write return ErrorKind::UnexpectedEof
    make_test! {
        #[variant(
            test_write_eof_error_std_mpsc,
            std::sync::mpsc::channel::<Vec<u8>>()
        )]
        #[variant(
            test_write_eof_error_std_sync_mpsc,
            std::sync::mpsc::sync_channel::<Vec<u8>>(1)
        )]
        #[variant(
            test_write_eof_error_tokio_mpsc,
            tokio::sync::mpsc::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_write_eof_error_tokio_unbounded,
            tokio::sync::mpsc::unbounded_channel::<Vec<u8>>(),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_write_eof_error_tokio_broadcast,
            tokio::sync::broadcast::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_write_eof_error_crossbeam_unbounded,
            crossbeam_channel::unbounded::<Vec<u8>>(),
            cfg(feature = "crossbeam-channel")
        )]
        |tx, rx| {
            let mut writer = ChannelCompat::new(tx);

            // simulate sending failure by dropping the receiver
            drop(rx);

            let data = vec![42u8; 100];
            let write_result = writer.write(&data);
            assert!(write_result.is_err());
            assert_eq!(write_result.unwrap_err().kind(), std::io::ErrorKind::UnexpectedEof);
        }
    }

    // test write flush
    make_test! {
        #[variant(
            test_flush_std_mpsc,
            std::sync::mpsc::channel::<Vec<u8>>()
        )]
        #[variant(
            test_flush_std_sync_mpsc,
            std::sync::mpsc::sync_channel::<Vec<u8>>(1)
        )]
        #[variant(
            test_flush_tokio_mpsc,
            tokio::sync::mpsc::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_flush_tokio_unbounded,
            tokio::sync::mpsc::unbounded_channel::<Vec<u8>>(),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_flush_tokio_broadcast,
            tokio::sync::broadcast::channel::<Vec<u8>>(1),
            cfg(feature = "tokio-channel")
        )]
        #[variant(
            test_flush_crossbeam_unbounded,
            crossbeam_channel::unbounded::<Vec<u8>>(),
            cfg(feature = "crossbeam-channel")
        )]
        |tx, _rx| {
            let mut writer = ChannelCompat::new(tx);

            let flush_result = writer.flush();
            assert!(flush_result.is_ok());
        }
    }
}
