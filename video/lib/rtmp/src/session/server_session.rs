use std::collections::HashMap;
use std::time::Duration;

use amf0::Amf0Value;
use bytes::Bytes;
use bytesio::bytes_writer::BytesWriter;
use bytesio::bytesio::{AsyncReadWrite, BytesIO};
use bytesio::bytesio_errors::BytesIOError;
use scuffle_utils::prelude::FutureTimeout;
use tokio::sync::oneshot;

use super::define::RtmpCommand;
use super::errors::SessionError;
use crate::channels::{ChannelData, DataProducer, PublishRequest, UniqueID};
use crate::chunk::{ChunkDecoder, ChunkEncoder, CHUNK_SIZE};
use crate::handshake::{HandshakeServer, ServerHandshakeState};
use crate::messages::{MessageParser, RtmpMessageData};
use crate::netconnection::NetConnection;
use crate::netstream::NetStreamWriter;
use crate::protocol_control_messages::ProtocolControlMessagesWriter;
use crate::user_control_messages::EventMessagesWriter;
use crate::{handshake, PublishProducer};

pub struct Session<S: AsyncReadWrite> {
	/// When you connect via rtmp, you specify the app name in the url
	/// For example: rtmp://localhost:1935/live/xyz
	/// The app name is "live"
	/// The next part of the url is the stream name (or the stream key) "xyz"
	/// However the stream key is not required to be the same for each stream
	/// you publish / play Traditionally we only publish a single stream per
	/// RTMP connection, However we can publish multiple streams per RTMP
	/// connection (using different stream keys) and or play multiple streams
	/// per RTMP connection (using different stream keys) as per the RTMP spec.
	app_name: Option<String>,

	/// This is a unique id for this session
	/// This is issued when the client connects to the server
	uid: Option<UniqueID>,

	/// Used to read and write data
	io: BytesIO<S>,

	/// Sometimes when doing the handshake we read too much data,
	/// this flag is used to indicate that we have data ready to parse and we
	/// should not read more data from the stream
	skip_read: bool,

	/// This is used to read the data from the stream and convert it into rtmp
	/// messages
	chunk_decoder: ChunkDecoder,
	/// This is used to convert rtmp messages into chunks
	chunk_encoder: ChunkEncoder,

	/// StreamID
	stream_id: u32,

	/// Data Producer
	data_producer: DataProducer,

	/// Is Publishing
	is_publishing: bool,

	/// when the publisher connects and tries to publish a stream, we need to
	/// send a publish request to the server
	publish_request_producer: PublishProducer,
}

impl<S: AsyncReadWrite> Session<S> {
	pub fn new(stream: S, data_producer: DataProducer, publish_request_producer: PublishProducer) -> Self {
		let io = BytesIO::new(stream);

		Self {
			uid: None,
			app_name: None,
			io,
			skip_read: false,
			chunk_decoder: ChunkDecoder::default(),
			chunk_encoder: ChunkEncoder::default(),
			data_producer,
			stream_id: 0,
			is_publishing: false,
			publish_request_producer,
		}
	}

	pub fn uid(&self) -> Option<UniqueID> {
		self.uid
	}

	/// Run the session to completion
	/// The result of the return value will be true if all publishers have
	/// disconnected If any publishers are still connected, the result will be
	/// false This can be used to detect non-graceful disconnects (ie. the
	/// client crashed)
	pub async fn run(&mut self) -> Result<bool, SessionError> {
		let mut handshaker = HandshakeServer::default();
		// Run the handshake to completion
		while !self.do_handshake(&mut handshaker).await? {}

		// Drop the handshaker, we don't need it anymore
		// We can get rid of the memory that was allocated for it
		drop(handshaker);

		tracing::debug!("Handshake complete");

		// Run the session to completion
		while match self.do_ready().await {
			Ok(v) => v,
			Err(SessionError::BytesIO(BytesIOError::ClientClosed)) => {
				// The client closed the connection
				// We are done with the session
				tracing::debug!("Client closed the connection");
				false
			}
			Err(e) => {
				return Err(e);
			}
		} {}

		// We should technically check the stream_map here
		// However most clients just disconnect without cleanly stopping the subscrition
		// streams (play streams) So we just check that all publishers have disconnected
		// cleanly
		Ok(!self.is_publishing)
	}

	/// This is the first stage of the session
	/// It is used to do the handshake with the client
	/// The handshake is the first thing that happens when you connect to an
	/// rtmp server
	async fn do_handshake(&mut self, handshaker: &mut HandshakeServer) -> Result<bool, SessionError> {
		let mut bytes_len = 0;

		while bytes_len < handshake::RTMP_HANDSHAKE_SIZE {
			let buf = self.io.read_timeout(Duration::from_millis(2500)).await?;
			bytes_len += buf.len();
			handshaker.extend_data(&buf[..]);
		}

		let mut writer = BytesWriter::default();
		handshaker.handshake(&mut writer)?;
		self.write_data(writer.dispose()).await?;

		if handshaker.state() == ServerHandshakeState::Finish {
			let over_read = handshaker.extract_remaining_bytes();

			if !over_read.is_empty() {
				self.skip_read = true;
				self.chunk_decoder.extend_data(&over_read[..]);
			}

			self.send_set_chunk_size().await?;

			// We are done with the handshake
			// This causes the loop to exit
			// And move onto the next stage of the session
			Ok(true)
		} else {
			// We are not done with the handshake yet
			// We need to read more data from the stream
			// This causes the loop to continue
			Ok(false)
		}
	}

	/// This is the second stage of the session
	/// It is used to read data from the stream and parse it into rtmp messages
	/// We also send data to the client if they are playing a stream
	async fn do_ready(&mut self) -> Result<bool, SessionError> {
		// If we have data ready to parse, parse it
		if self.skip_read {
			self.skip_read = false;
		} else {
			let data = self.io.read_timeout(Duration::from_millis(2500)).await?;
			self.chunk_decoder.extend_data(&data[..]);
		}

		self.parse_chunks().await?;

		Ok(true)
	}

	/// Parse data from the client into rtmp messages and process them
	async fn parse_chunks(&mut self) -> Result<(), SessionError> {
		while let Some(chunk) = self.chunk_decoder.read_chunk()? {
			let timestamp = chunk.message_header.timestamp;
			let msg_stream_id = chunk.message_header.msg_stream_id;

			if let Some(msg) = MessageParser::parse(chunk)? {
				self.process_messages(msg, msg_stream_id, timestamp).await?;
			}
		}

		Ok(())
	}

	/// Process rtmp messages
	async fn process_messages(
		&mut self,
		rtmp_msg: RtmpMessageData,
		stream_id: u32,
		timestamp: u32,
	) -> Result<(), SessionError> {
		match rtmp_msg {
			RtmpMessageData::Amf0Command {
				command_name,
				transaction_id,
				command_object,
				others,
			} => {
				self.on_amf0_command_message(stream_id, command_name, transaction_id, command_object, others)
					.await?
			}
			RtmpMessageData::SetChunkSize { chunk_size } => {
				self.on_set_chunk_size(chunk_size as usize)?;
			}
			RtmpMessageData::AudioData { data } => {
				self.on_data(stream_id, ChannelData::Audio { timestamp, data }).await?;
			}
			RtmpMessageData::VideoData { data } => {
				self.on_data(stream_id, ChannelData::Video { timestamp, data }).await?;
			}
			RtmpMessageData::AmfData { data } => {
				self.on_data(stream_id, ChannelData::Metadata { timestamp, data }).await?;
			}
		}

		Ok(())
	}

	/// Set the server chunk size to the client
	async fn send_set_chunk_size(&mut self) -> Result<(), SessionError> {
		let mut writer = BytesWriter::default();
		ProtocolControlMessagesWriter::write_set_chunk_size(&self.chunk_encoder, &mut writer, CHUNK_SIZE as u32)?;
		self.chunk_encoder.set_chunk_size(CHUNK_SIZE);
		self.write_data(writer.dispose()).await?;

		Ok(())
	}

	/// on_data is called when we receive a data message from the client (a
	/// published_stream) Such as audio, video, or metadata
	/// We then forward the data to the specified publisher
	async fn on_data(&self, stream_id: u32, data: ChannelData) -> Result<(), SessionError> {
		if stream_id != self.stream_id || !self.is_publishing {
			return Err(SessionError::UnknownStreamID(stream_id));
		};

		if matches!(
			self.data_producer.send(data).timeout(Duration::from_secs(2)).await,
			Err(_) | Ok(Err(_))
		) {
			tracing::debug!("Publisher dropped");
			return Err(SessionError::PublisherDropped);
		}

		Ok(())
	}

	/// on_amf0_command_message is called when we receive an AMF0 command
	/// message from the client We then handle the command message
	async fn on_amf0_command_message(
		&mut self,
		stream_id: u32,
		command_name: Amf0Value,
		transaction_id: Amf0Value,
		command_object: Amf0Value,
		others: Vec<Amf0Value>,
	) -> Result<(), SessionError> {
		let cmd = RtmpCommand::from(match command_name {
			Amf0Value::String(ref s) => s,
			_ => "",
		});

		let transaction_id = match transaction_id {
			Amf0Value::Number(number) => number,
			_ => 0.0,
		};

		let obj = match command_object {
			Amf0Value::Object(obj) => obj,
			_ => HashMap::new(),
		};

		match cmd {
			RtmpCommand::Connect => {
				self.on_command_connect(transaction_id, stream_id, obj, others).await?;
			}
			RtmpCommand::CreateStream => {
				self.on_command_create_stream(transaction_id, stream_id, obj, others).await?;
			}
			RtmpCommand::DeleteStream => {
				self.on_command_delete_stream(transaction_id, stream_id, obj, others).await?;
			}
			RtmpCommand::Play => {
				return Err(SessionError::PlayNotSupported);
			}
			RtmpCommand::Publish => {
				self.on_command_publish(transaction_id, stream_id, obj, others).await?;
			}
			RtmpCommand::CloseStream | RtmpCommand::ReleaseStream => {
				// Not sure what this is for
			}
			RtmpCommand::Unknown(_) => {}
		}

		Ok(())
	}

	/// on_set_chunk_size is called when we receive a set chunk size message
	/// from the client We then update the chunk size of the unpacketizer
	fn on_set_chunk_size(&mut self, chunk_size: usize) -> Result<(), SessionError> {
		if self.chunk_decoder.update_max_chunk_size(chunk_size) {
			Ok(())
		} else {
			Err(SessionError::InvalidChunkSize(chunk_size))
		}
	}

	/// on_command_connect is called when we receive a amf0 command message with
	/// the name "connect" We then handle the connect message
	/// This is called when the client first connects to the server
	async fn on_command_connect(
		&mut self,
		transaction_id: f64,
		_stream_id: u32,
		command_obj: HashMap<String, Amf0Value>,
		_others: Vec<Amf0Value>,
	) -> Result<(), SessionError> {
		let mut writer = BytesWriter::default();

		ProtocolControlMessagesWriter::write_window_acknowledgement_size(
			&self.chunk_encoder,
			&mut writer,
			CHUNK_SIZE as u32,
		)?;

		ProtocolControlMessagesWriter::write_set_peer_bandwidth(
			&self.chunk_encoder,
			&mut writer,
			CHUNK_SIZE as u32,
			2, // 2 = dynamic
		)?;

		let app_name = command_obj.get("app");
		let app_name = match app_name {
			Some(Amf0Value::String(app)) => app,
			_ => {
				return Err(SessionError::NoAppName);
			}
		};

		self.app_name = Some(app_name.to_owned());

		// The only AMF encoding supported by this server is AMF0
		// So we ignore the objectEncoding value sent by the client
		// and always use AMF0
		// - OBS does not support AMF3 (https://github.com/obsproject/obs-studio/blob/1be1f51635ac85b3ad768a88b3265b192bd0bf18/plugins/obs-outputs/librtmp/rtmp.c#L1737)
		// - Ffmpeg does not support AMF3 either (https://github.com/FFmpeg/FFmpeg/blob/c125860892e931d9b10f88ace73c91484815c3a8/libavformat/rtmpproto.c#L569)
		// - NginxRTMP does not support AMF3 (https://github.com/arut/nginx-rtmp-module/issues/313)
		// - SRS does not support AMF3 (https://github.com/ossrs/srs/blob/dcd02fe69cdbd7f401a7b8d139d95b522deb55b1/trunk/src/protocol/srs_protocol_rtmp_stack.cpp#L599)
		// However, the new enhanced-rtmp-v1 spec from YouTube does encourage the use of AMF3 over AMF0 (https://github.com/veovera/enhanced-rtmp)
		// We will eventually support this spec but for now we will stick to AMF0
		NetConnection::write_connect_response(
			&self.chunk_encoder,
			&mut writer,
			transaction_id,
			"FMS/3,0,1,123", // flash version (this value is used by other media servers as well)
			31.0,            // No idea what this is, but it is used by other media servers as well
			"NetConnection.Connect.Success",
			"status", // Again not sure what this is but other media servers use it.
			"Connection Succeeded.",
			0.0,
		)?;

		self.write_data(writer.dispose()).await?;

		Ok(())
	}

	/// on_command_create_stream is called when we receive a amf0 command
	/// message with the name "createStream" We then handle the createStream
	/// message This is called when the client wants to create a stream
	/// A NetStream is used to start publishing or playing a stream
	async fn on_command_create_stream(
		&mut self,
		transaction_id: f64,
		_stream_id: u32,
		_command_obj: HashMap<String, Amf0Value>,
		_others: Vec<Amf0Value>,
	) -> Result<(), SessionError> {
		let mut writer = BytesWriter::default();
		// 1.0 is the Stream ID of the stream we are creating
		NetConnection::write_create_stream_response(&self.chunk_encoder, &mut writer, transaction_id, 1.0)?;
		self.write_data(writer.dispose()).await?;

		Ok(())
	}

	/// A delete stream message is unrelated to the NetConnection close method.
	/// Delete stream is basically a way to tell the server that you are done
	/// publishing or playing a stream. The server will then remove the stream
	/// from its list of streams.
	async fn on_command_delete_stream(
		&mut self,
		transaction_id: f64,
		_stream_id: u32,
		_command_obj: HashMap<String, Amf0Value>,
		others: Vec<Amf0Value>,
	) -> Result<(), SessionError> {
		let mut writer = BytesWriter::default();

		let stream_id = match others.first() {
			Some(Amf0Value::Number(stream_id)) => *stream_id,
			_ => 0.0,
		} as u32;

		if self.stream_id == stream_id && self.is_publishing {
			self.stream_id = 0;
			self.is_publishing = false;
		}

		NetStreamWriter::write_on_status(
			&self.chunk_encoder,
			&mut writer,
			transaction_id,
			"status",
			"NetStream.DeleteStream.Suceess",
			"",
		)?;

		self.write_data(writer.dispose()).await?;

		Ok(())
	}

	/// on_command_publish is called when we receive a amf0 command message with
	/// the name "publish" publish commands are used to publish a stream to the
	/// server ie. the user wants to start streaming to the server
	async fn on_command_publish(
		&mut self,
		transaction_id: f64,
		stream_id: u32,
		_command_obj: HashMap<String, Amf0Value>,
		others: Vec<Amf0Value>,
	) -> Result<(), SessionError> {
		let stream_name = match others.first() {
			Some(Amf0Value::String(val)) => val,
			_ => {
				return Err(SessionError::NoStreamName);
			}
		};

		let Some(app_name) = &self.app_name else {
			return Err(SessionError::NoAppName);
		};

		let (response, waiter) = oneshot::channel();

		if self
			.publish_request_producer
			.send(PublishRequest {
				app_name: app_name.clone(),
				stream_name: stream_name.clone(),
				response,
			})
			.await
			.is_err()
		{
			return Err(SessionError::PublishRequestDenied);
		}

		let Ok(uid) = waiter.await else {
			return Err(SessionError::PublishRequestDenied);
		};

		self.uid = Some(uid);

		self.is_publishing = true;
		self.stream_id = stream_id;

		let mut writer = BytesWriter::default();
		EventMessagesWriter::write_stream_begin(&self.chunk_encoder, &mut writer, stream_id)?;

		NetStreamWriter::write_on_status(
			&self.chunk_encoder,
			&mut writer,
			transaction_id,
			"status",
			"NetStream.Publish.Start",
			"",
		)?;

		self.write_data(writer.dispose()).await?;

		Ok(())
	}

	/// write_data is a helper function to write data to the underlying
	/// connection. If the data is empty, it will not write anything.
	/// This is to avoid writing empty bytes to the underlying connection.
	async fn write_data(&mut self, data: Bytes) -> Result<(), SessionError> {
		if !data.is_empty() {
			self.io.write_timeout(data, Duration::from_secs(2)).await?;
		}

		Ok(())
	}
}
