use kiro_runtime::{HostResult, KiroError, RuntimeVal};

pub async fn empty(args: Vec<RuntimeVal>) -> HostResult {
    expect_arity(&args, 0, "empty")?;
    Ok(RuntimeVal::bytes([]))
}

pub async fn from_str(args: Vec<RuntimeVal>) -> HostResult {
    let value = RuntimeVal::expect_arg(&args, 0, "from_str")?.as_str()?;
    Ok(RuntimeVal::bytes(value.as_bytes()))
}

pub async fn to_str(args: Vec<RuntimeVal>) -> HostResult {
    let value = RuntimeVal::expect_arg(&args, 0, "to_str")?.as_bytes()?;
    let text = std::str::from_utf8(value)
        .map_err(|error| KiroError::message("InvalidUtf8", error.to_string()))?;
    Ok(RuntimeVal::from(text))
}

pub async fn from_hex(args: Vec<RuntimeVal>) -> HostResult {
    let value = RuntimeVal::expect_arg(&args, 0, "from_hex")?.as_str()?;
    if value.len() % 2 != 0 {
        return Err(KiroError::message(
            "InvalidHex",
            "hex text must contain an even number of digits",
        ));
    }

    let mut decoded = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high = hex_digit(pair[0])?;
        let low = hex_digit(pair[1])?;
        decoded.push((high << 4) | low);
    }
    Ok(RuntimeVal::bytes(decoded))
}

pub async fn to_hex(args: Vec<RuntimeVal>) -> HostResult {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let value = RuntimeVal::expect_arg(&args, 0, "to_hex")?.as_bytes()?;
    let mut encoded = String::with_capacity(value.len() * 2);
    for byte in value {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    Ok(RuntimeVal::from(encoded))
}

pub async fn slice(args: Vec<RuntimeVal>) -> HostResult {
    let value = RuntimeVal::expect_arg(&args, 0, "slice")?.as_bytes()?;
    let start = byte_index(RuntimeVal::expect_arg(&args, 1, "slice")?, "start")?;
    let end = byte_index(RuntimeVal::expect_arg(&args, 2, "slice")?, "end")?;
    let selected = value.get(start..end).ok_or_else(|| {
        KiroError::message(
            "InvalidRange",
            format!(
                "byte range {start}..{end} is outside length {}",
                value.len()
            ),
        )
    })?;
    Ok(RuntimeVal::bytes(selected))
}

pub async fn concat(args: Vec<RuntimeVal>) -> HostResult {
    let left = RuntimeVal::expect_arg(&args, 0, "concat")?.as_bytes()?;
    let right = RuntimeVal::expect_arg(&args, 1, "concat")?.as_bytes()?;
    let mut joined = Vec::with_capacity(left.len() + right.len());
    joined.extend_from_slice(left);
    joined.extend_from_slice(right);
    Ok(RuntimeVal::bytes(joined))
}

fn expect_arity(args: &[RuntimeVal], expected: usize, function: &str) -> Result<(), KiroError> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(KiroError::message(
            "ArityError",
            format!(
                "{function} expected {expected} arguments, got {}",
                args.len()
            ),
        ))
    }
}

fn hex_digit(value: u8) -> Result<u8, KiroError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(KiroError::message(
            "InvalidHex",
            format!("invalid hexadecimal digit '{}'", char::from(value)),
        )),
    }
}

fn byte_index(value: &RuntimeVal, name: &str) -> Result<usize, KiroError> {
    let value = value.as_num()?;
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
        return Err(KiroError::message(
            "InvalidRange",
            format!("{name} must be a non-negative integer"),
        ));
    }
    Ok(value as usize)
}
