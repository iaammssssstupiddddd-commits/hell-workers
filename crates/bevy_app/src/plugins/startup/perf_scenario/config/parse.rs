use super::*;

pub(super) fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

pub(super) fn value_from_args_or_env(
    args: &[String],
    flag: &str,
    env_key: &str,
) -> Result<Option<String>, PerfScenarioConfigError> {
    value_from_args(args, flag).map(|value| value.or_else(|| env::var(env_key).ok()))
}

pub(super) fn parse_value_or_default<T>(
    value: Option<String>,
    flag: &str,
    allowed: &str,
    parse: impl FnOnce(&str) -> Option<T>,
    default: T,
) -> Result<T, PerfScenarioConfigError> {
    match value {
        Some(value) => parse(&value).ok_or_else(|| {
            PerfScenarioConfigError(format!("{flag} must be one of {allowed}; got '{value}'"))
        }),
        None => Ok(default),
    }
}

pub(super) fn parse_u32_value_or_default(
    value: Option<String>,
    flag: &str,
    default: u32,
) -> Result<u32, PerfScenarioConfigError> {
    match value {
        Some(value) => value.parse().map_err(|_| {
            PerfScenarioConfigError(format!("{flag} must be an unsigned integer; got '{value}'"))
        }),
        None => Ok(default),
    }
}

pub(super) fn parse_u64_value_or_random(
    value: Option<String>,
    flag: &str,
) -> Result<u64, PerfScenarioConfigError> {
    match value {
        Some(value) => value.parse().map_err(|_| {
            PerfScenarioConfigError(format!("{flag} must be an unsigned integer; got '{value}'"))
        }),
        None => Ok(rand::random()),
    }
}

pub(super) fn parse_u64_value_or_default(
    value: Option<String>,
    flag: &str,
    default: u64,
) -> Result<u64, PerfScenarioConfigError> {
    match value {
        Some(value) => value.parse().map_err(|_| {
            PerfScenarioConfigError(format!("{flag} must be an unsigned integer; got '{value}'"))
        }),
        None => Ok(default),
    }
}

pub(super) fn parse_duration_secs(
    value: Option<String>,
    flag: &str,
    default: f32,
    allow_zero: bool,
) -> Result<f32, PerfScenarioConfigError> {
    let Some(value) = value else {
        return Ok(default);
    };
    let parsed = value.parse::<f32>().map_err(|_| {
        PerfScenarioConfigError(format!(
            "{flag} must be a finite number of seconds; got '{value}'"
        ))
    })?;
    let is_valid = parsed.is_finite() && (parsed >= 0.0) && (allow_zero || parsed > 0.0);
    if !is_valid {
        let constraint = if allow_zero {
            "at least 0"
        } else {
            "greater than 0"
        };
        return Err(PerfScenarioConfigError(format!(
            "{flag} must be finite and {constraint}; got '{value}'"
        )));
    }
    Ok(parsed)
}

pub(super) fn value_from_args(
    args: &[String],
    flag: &str,
) -> Result<Option<String>, PerfScenarioConfigError> {
    let Some(index) = args.iter().position(|arg| arg == flag) else {
        return Ok(None);
    };
    args.get(index + 1)
        .cloned()
        .map(Some)
        .ok_or_else(|| PerfScenarioConfigError(format!("{flag} requires a value")))
}

pub(super) const fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut mixed = value;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}
