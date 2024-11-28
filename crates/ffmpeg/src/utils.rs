use ffmpeg_sys_next::*;

pub fn check_i64(val: i64) -> Option<i64> {
	if val == AV_NOPTS_VALUE {
		None
	} else {
		Some(val)
	}
}
