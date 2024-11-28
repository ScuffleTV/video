use crate::FlvDemuxerError;

#[test]
fn test_error_display() {
	let error = FlvDemuxerError::InvalidFrameType(0);
	assert_eq!(error.to_string(), "invalid frame type: 0");

	let error = FlvDemuxerError::IO(std::io::Error::new(std::io::ErrorKind::Other, "test"));
	assert_eq!(error.to_string(), "io error: test");

	let error = FlvDemuxerError::Amf0Read(amf0::Amf0ReadError::UnknownMarker(0));
	assert_eq!(error.to_string(), "amf0 read error: unknown marker: 0");

	let error = FlvDemuxerError::InvalidFlvHeader;
	assert_eq!(error.to_string(), "invalid flv header");

	let error = FlvDemuxerError::InvalidScriptDataName;
	assert_eq!(error.to_string(), "invalid script data name");

	let error = FlvDemuxerError::InvalidEnhancedPacketType(0);
	assert_eq!(error.to_string(), "invalid enhanced packet type: 0");

	let error = FlvDemuxerError::InvalidSoundRate(0);
	assert_eq!(error.to_string(), "invalid sound rate: 0");

	let error = FlvDemuxerError::InvalidSoundSize(0);
	assert_eq!(error.to_string(), "invalid sound size: 0");

	let error = FlvDemuxerError::InvalidSoundType(0);
	assert_eq!(error.to_string(), "invalid sound type: 0");
}
