#[derive(Debug, serde::Deserialize)]
pub struct CombinedStreamMsg<T> {
    pub data: T,
}

pub fn parse_combined_data<T>(msg: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let wrapper: CombinedStreamMsg<serde_json::Value> = match serde_json::from_str(msg) {
        Ok(wrapper) => wrapper,
        Err(err) => {
            let snippet: String = msg.chars().take(180).collect();
            let suffix = if msg.chars().count() > 180 { "..." } else { "" };
            eprintln!(
                "[parse_combined_data:{}] invalid JSON: {} payload='{}{}'",
                std::any::type_name::<T>(),
                err,
                snippet,
                suffix
            );
            return None;
        }
    };

    serde_json::from_value(wrapper.data).ok()
}

#[cfg(test)]
mod tests {
    use super::parse_combined_data;

    #[derive(Debug, serde::Deserialize)]
    struct AggTradeLike {
        p: String,
    }

    #[test]
    fn parse_combined_data_accepts_matching_payload() {
        let payload = r#"{"stream":"btcusdt@aggTrade","data":{"p":"43000.50"}}"#;
        let parsed: Option<AggTradeLike> = parse_combined_data(payload);

        assert_eq!(parsed.expect("payload should parse").p, "43000.50");
    }

    #[test]
    fn parse_combined_data_rejects_non_matching_data_shape() {
        let payload =
            r#"{"stream":"btcusdt@depth@100ms","data":{"b":[["24100.10","1.20"]],"a":[["24100.20","0.80"]]}}"#;
        let parsed: Option<AggTradeLike> = parse_combined_data(payload);

        assert!(parsed.is_none());
    }
}
