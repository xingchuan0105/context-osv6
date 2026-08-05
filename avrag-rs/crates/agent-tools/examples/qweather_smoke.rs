#[tokio::main]
async fn main() {
    let loc = std::env::args().nth(1).unwrap_or_else(|| "上海".into());
    match agent_tools::weather::query_weather(&loc, "metric").await {
        Ok(w) => {
            println!(
                "OK {} {}{} {} humidity={} wind={}",
                w.location, w.temperature, w.units, w.description, w.humidity, w.wind_speed
            );
        }
        Err(e) => {
            eprintln!("ERR {e}");
            std::process::exit(1);
        }
    }
}
