use crate::news::types::NewsItem;

const SYMBOL_ALIASES: [(&str, &str); 12] = [
    ("btc", "btcusdt"),
    ("bitcoin", "btcusdt"),
    ("eth", "ethusdt"),
    ("ethereum", "ethusdt"),
    ("sol", "solusdt"),
    ("solana", "solusdt"),
    ("bnb", "bnbusdt"),
    ("binance coin", "bnbusdt"),
    ("xrp", "xrpusdt"),
    ("ripple", "xrpusdt"),
    ("doge", "dogeusdt"),
    ("dogecoin", "dogeusdt"),
];

pub fn tag_symbols(item: &mut NewsItem) {
    let haystack = format!("{} {}", item.title, item.summary).to_lowercase();
    let mut symbols: Vec<String> = SYMBOL_ALIASES
        .iter()
        .filter_map(|(alias, canonical)| haystack.contains(alias).then_some(*canonical))
        .map(str::to_string)
        .collect();

    symbols.sort();
    symbols.dedup();
    item.symbols = symbols;
}
