use alloy::primitives::Address;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

const SDN_XML_URLS: [&str; 2] = [
    "https://www.treasury.gov/ofac/downloads/sdn.xml",
    "https://sanctions.ofac.treas.gov/downloads/sdn.xml",
];

const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 86_400;

pub async fn load_sanctioned_evm_addresses() -> HashSet<Address> {
    let client = reqwest::Client::new();
    match fetch_sdn_xml(&client).await {
        Ok(xml) => parse_sanctioned_evm_addresses_from_xml(&xml),
        Err(err) => {
            warn!("Failed to fetch OFAC SDN XML: {err}");
            HashSet::new()
        }
    }
}

pub async fn refresh_sanctioned_evm_addresses(
    sanctioned: Arc<RwLock<HashSet<Address>>>,
    interval_secs: Option<u64>,
) {
    let interval_secs = interval_secs.unwrap_or(DEFAULT_REFRESH_INTERVAL_SECS);
    loop {
        sleep(Duration::from_secs(interval_secs)).await;
        let latest = load_sanctioned_evm_addresses().await;
        let count = latest.len();
        {
            let mut guard = sanctioned.write().await;
            *guard = latest;
        }
        info!("OFAC SDN set refreshed with {count} sanctioned EVM addresses");
    }
}

async fn fetch_sdn_xml(client: &reqwest::Client) -> anyhow::Result<String> {
    let mut last_err: Option<anyhow::Error> = None;
    for url in SDN_XML_URLS {
        match client.get(url).send().await {
            Ok(resp) => match resp.error_for_status() {
                Ok(ok) => return Ok(ok.text().await?),
                Err(err) => last_err = Some(err.into()),
            },
            Err(err) => last_err = Some(err.into()),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("No SDN XML URL succeeded")))
}

pub fn parse_sanctioned_evm_addresses_from_xml(xml: &str) -> HashSet<Address> {
    let mut out = HashSet::new();
    for block in extract_tag_blocks(xml, "id") {
        let attr_type = extract_attribute(block, "type");
        let id_type = if attr_type.is_empty() {
            extract_first_tag_value(block, "idType")
        } else {
            attr_type
        };
        if !is_supported_id_type(&id_type) {
            continue;
        }
        let id_number = extract_first_tag_value(block, "idNumber");
        if let Some(addr) = extract_first_evm_address(&id_number) {
            out.insert(addr);
        }
    }
    out
}

fn is_supported_id_type(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed == "Digital Currency Address - ETH" || trimmed == "Digital Currency Address - EVM"
}

fn extract_first_evm_address(input: &str) -> Option<Address> {
    let s = input.trim();
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i + 42 <= bytes.len() {
        if bytes[i] == b'0' && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
            let candidate = &s[i..i + 42];
            if candidate[2..].bytes().all(|b| b.is_ascii_hexdigit()) {
                if let Ok(addr) = candidate.parse::<Address>() {
                    return Some(addr);
                }
            }
        }
        i += 1;
    }
    None
}

fn extract_tag_blocks<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut from = 0usize;
    while let Some(open_rel) = xml[from..].find(&open) {
        let start = from + open_rel;
        let after_open = match xml[start..].find('>') {
            Some(i) => start + i + 1,
            None => break,
        };
        let close_start = match xml[after_open..].find(&close) {
            Some(i) => after_open + i,
            None => break,
        };
        let end = close_start + close.len();
        out.push(&xml[start..end]);
        from = end;
    }
    out
}

fn extract_first_tag_value(xml: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let Some(start_rel) = xml.find(&open) else {
        return String::new();
    };
    let start = start_rel + open.len();
    let Some(end_rel) = xml[start..].find(&close) else {
        return String::new();
    };
    xml[start..start + end_rel].trim().to_string()
}

fn extract_attribute(tag_block: &str, key: &str) -> String {
    let header_end = tag_block.find('>').unwrap_or(tag_block.len());
    let header = &tag_block[..header_end];
    let patterns = [format!(r#"{key}=""#), format!(r#"{key}='"#)];
    for pattern in patterns {
        if let Some(idx) = header.find(&pattern) {
            let value_start = idx + pattern.len();
            let quote = header.as_bytes()[value_start - 1] as char;
            if let Some(end_rel) = header[value_start..].find(quote) {
                return header[value_start..value_start + end_rel]
                    .trim()
                    .to_string();
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use alloy::primitives::Address;

    use super::parse_sanctioned_evm_addresses_from_xml;

    #[test]
    fn parses_known_tornado_cash_address() {
        let xml = r#"
<sdnList>
  <sdnEntry>
    <idList>
      <id>
        <idType>Digital Currency Address - ETH</idType>
        <idNumber>0x8589427373D6D84E98730D7795D8f6f8731FDA16</idNumber>
      </id>
      <id>
        <idType>Passport</idType>
        <idNumber>AB12345</idNumber>
      </id>
    </idList>
  </sdnEntry>
</sdnList>
"#;
        let out = parse_sanctioned_evm_addresses_from_xml(xml);
        let expected: Address = "0x8589427373D6D84E98730D7795D8f6f8731FDA16"
            .parse()
            .expect("valid address");
        assert!(out.contains(&expected));
    }

    #[test]
    fn parses_id_type_attribute_variant() {
        let xml = r#"
<sdnList>
  <id type="Digital Currency Address - EVM">
    <idNumber>0x1111111111111111111111111111111111111111</idNumber>
  </id>
</sdnList>
"#;
        let out = parse_sanctioned_evm_addresses_from_xml(xml);
        let expected: Address = "0x1111111111111111111111111111111111111111"
            .parse()
            .expect("valid address");
        assert!(out.contains(&expected));
    }
}
