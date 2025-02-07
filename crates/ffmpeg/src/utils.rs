use ffmpeg_sys_next::*;

/// Checks if a value is AV_NOPTS_VALUE and returns None if it is.
pub const fn check_i64(val: i64) -> Option<i64> {
    if val == AV_NOPTS_VALUE {
        None
    } else {
        Some(val)
    }
}

/// Returns the value if it is Some, otherwise returns AV_NOPTS_VALUE.
pub const fn or_nopts(val: Option<i64>) -> i64 {
    if let Some(val) = val {
        val
    } else {
        AV_NOPTS_VALUE
    }
}
