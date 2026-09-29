//! The status codes the BLE stack returns besides the Bluetooth Core's,
//! read from the `BLE_STATUS_*` definitions of `ble_defs.h`.

use crate::cube::{BLE_CORE_DIR, CubeTag};

pub fn extract(tag: &CubeTag<'_>) -> Result<Vec<(String, u8)>, String> {
    let path = format!("{BLE_CORE_DIR}/ble_defs.h");
    parse(&tag.read_text(&path)?).map_err(|error| format!("{} {path}: {error}", tag.tag))
}

/// Each `#define BLE_STATUS_<NAME> 0x<NN>U` of `header`, in order. Any other
/// definition of a `BLE_STATUS_` name is an error, so a change in how ST
/// writes them is noticed rather than skipped.
pub fn parse(header: &str) -> Result<Vec<(String, u8)>, String> {
    let mut statuses = Vec::new();
    for line in header.lines() {
        let mut words = line.split_whitespace();
        if words.next() != Some("#define") {
            continue;
        }
        let Some(name) = words.next().filter(|name| name.starts_with("BLE_STATUS_")) else {
            continue;
        };
        let value = words
            .next()
            .filter(|_| words.next().is_none())
            .and_then(|value| value.strip_prefix("0x"))
            .and_then(|value| value.strip_suffix('U'))
            .filter(|digits| digits.len() == 2)
            .and_then(|digits| u8::from_str_radix(digits, 16).ok())
            .ok_or_else(|| format!("cannot read the definition {:?}", line.trim()))?;
        if !name
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(format!("{name} is not a C constant name"));
        }
        statuses.push((name.to_owned(), value));
    }
    if statuses.is_empty() {
        return Err("defines no BLE_STATUS_ codes".to_owned());
    }
    Ok(statuses)
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn definitions_are_read_in_order() {
        let header = "\
/* Status codes */
#define BLE_STATUS_SUCCESS                              0x00U
#define ACI_GAP_LIMITED_DISCOVERABLE_VSEVT_CODE         0x0400U
/* The Host failed. */
#define BLE_STATUS_FAILED                               0x91U
";
        assert_eq!(
            parse(header).unwrap(),
            [
                ("BLE_STATUS_SUCCESS".to_owned(), 0x00),
                ("BLE_STATUS_FAILED".to_owned(), 0x91)
            ]
        );
    }

    #[test]
    fn unreadable_definitions_are_errors() {
        assert!(
            parse("#define BLE_STATUS_FAILED 0x91").is_err(),
            "no suffix"
        );
        assert!(parse("#define BLE_STATUS_FAILED (0x91U)").is_err());
        assert!(
            parse("#define BLE_STATUS_FAILED 0x0091U").is_err(),
            "not a byte"
        );
        assert!(parse("#define OTHER 0x01U").is_err(), "no status codes");
    }
}
