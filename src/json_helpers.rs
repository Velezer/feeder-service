#[derive(Debug, serde::Deserialize)]
pub struct CombinedStreamMsg<T> {
    pub data: T,
}

pub fn parse_combined_data<T>(msg: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let wrapper: CombinedStreamMsg<T> = match serde_json::from_str(msg) {
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
    Some(wrapper.data)
}
