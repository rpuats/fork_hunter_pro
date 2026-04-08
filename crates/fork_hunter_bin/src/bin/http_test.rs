use std::time::Instant;

#[tokio::main]
async fn main() {
    println!("Starting HTTP test...");
    let start = Instant::now();
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();
    
    println!("Client built in {:?}", start.elapsed());
    
    let url = "https://line51.tf39be-resources.com/events/list?lang=ru&scopeMarket=3000";
    println!("Fetching {}", url);
    
    match client.get(url)
        .header("User-Agent", "Mozilla/5.0")
        .header("Accept", "application/json")
        .send()
        .await {
        Ok(resp) => {
            println!("Response received in {:?}, status: {}", start.elapsed(), resp.status());
            match resp.bytes().await {
                Ok(bytes) => {
                    println!("Bytes received in {:?}, size: {} bytes", start.elapsed(), bytes.len());
                }
                Err(e) => {
                    println!("Bytes error: {}", e);
                }
            }
        }
        Err(e) => {
            println!("Request error: {}", e);
        }
    }
    
    println!("Total time: {:?}", start.elapsed());
}
