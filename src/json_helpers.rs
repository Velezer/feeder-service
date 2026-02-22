use simd_json::serde::from_slice;

#[derive(Debug, serde::Deserialize)]
pub struct CombinedStreamMsg<T> {
    pub data: T,
}

pub fn parse_combined_data<T>(msg: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let mut bytes = msg.as_bytes().to_vec();
    let wrapper: CombinedStreamMsg<T> = from_slice(&mut bytes).ok()?;
    Some(wrapper.data)
}
