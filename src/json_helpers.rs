
#[derive(Debug, serde::Deserialize)]
pub struct CombinedStreamMsg<T> {
    pub data: T,
}

pub fn parse_combined_data<T>(msg: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let wrapper: CombinedStreamMsg<T> = serde_json::from_str(msg).ok()?;
    Some(wrapper.data)
}
