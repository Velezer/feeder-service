use reqwest::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize)]
struct AggTrade {
    a: u64,    // trade_id
    p: String, // price
    q: String, // quantity
    T: u64,    // timestamp (ms)
    m: bool,   // is_buyer_maker
}

fn get_last_trade_id(conn: &Connection, symbol: &str) -> rusqlite::Result<Option<u64>> {
    conn.query_row(
        "SELECT MAX(trade_id) FROM agg_trades WHERE symbol = ?1",
        [symbol],
        |row| row.get(0),
    )
    .optional()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let symbol = "BTCUSDT";

    let mut conn = Connection::open("trades.db")?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS agg_trades (
            trade_id INTEGER PRIMARY KEY,
            symbol TEXT NOT NULL,
            price REAL NOT NULL,
            qty REAL NOT NULL,
            timestamp INTEGER NOT NULL,
            is_buyer_maker INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_symbol_time
        ON agg_trades(symbol, timestamp);
        ",
    )?;

    let client = Client::new();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis() as u64;

    let one_day_ms = 24 * 60 * 60 * 1000;
    let cutoff_time = now - one_day_ms;

    println!("Starting resumable 1-day aggTrade ingestion...");

    // Determine starting from_id
    let mut from_id: u64 = if let Some(last_id) = get_last_trade_id(&conn, symbol)? {
        println!("Resuming from trade_id {}", last_id);
        last_id + 1
    } else {
        println!("No existing data. Bootstrapping from 1-day cutoff.");

        let url = format!(
            "https://api.binance.com/api/v3/aggTrades?symbol={}&startTime={}&limit=1",
            symbol, cutoff_time
        );

        let trades: Vec<AggTrade> = client.get(&url).send().await?.json().await?;

        if trades.is_empty() {
            println!("No trades found.");
            return Ok(());
        }

        trades[0].a
    };

    loop {
        let url = format!(
            "https://api.binance.com/api/v3/aggTrades?symbol={}&fromId={}&limit=1000",
            symbol, from_id
        );

        let trades: Vec<AggTrade> = client.get(&url).send().await?.json().await?;

        if trades.is_empty() {
            break;
        }

        let tx = conn.transaction()?;
        let mut last_id = from_id;
        let mut reached_now = false;

        for t in &trades {
            if t.T > now {
                reached_now = true;
                break;
            }

            tx.execute(
                "INSERT OR IGNORE INTO agg_trades
                 (trade_id, symbol, price, qty, timestamp, is_buyer_maker)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    t.a,
                    symbol,
                    t.p.parse::<f64>()?,
                    t.q.parse::<f64>()?,
                    t.T,
                    if t.m { 
                        1 // seller 
                    } else {
                         0 // buyer 
                    }
                ],
            )?;

            last_id = t.a;
        }

        tx.commit()?;

        println!("Inserted up to trade_id {}", last_id);

        if reached_now || trades.len() < 1000 {
            break;
        }

        from_id = last_id + 1;
    }

    println!("Done.");

    Ok(())
}
