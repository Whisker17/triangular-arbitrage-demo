mod types;
mod constants;
mod config;
mod cache;
mod math;
mod blockchain;
mod arbitrage;
mod logging;
mod display;
mod pools;

use std::error::Error;
use alloy::providers::ProviderBuilder;

use tokio::runtime::Runtime;
use tokio::time::{sleep, Duration, Instant};
use chrono::Utc;

use config::Config;
use cache::ReservesCache;
use blockchain::{fetch_all_reserves_with_retry, get_current_block};
use arbitrage::find_optimal_arbitrage;
use logging::{
    init_csv_file, write_arbitrage_to_csv, log_profitable_arbitrage, 
    log_no_profit, log_analysis_failure, log_csv_success, log_csv_failure
};
use display::{print_startup_banner, format_pool_reserves, format_block_info};
use pools::moe::MoeProtocol;

/// Main application entry point
fn main() -> Result<(), Box<dyn Error>> {
    // Load configuration from environment variables
    let config = Config::load().map_err(|e| {
        eprintln!("Configuration Error: {}", e);
        eprintln!("Please set the RPC_URL environment variable.");
        eprintln!("Example: export RPC_URL=https://rpc.mantle.xyz");
        eprintln!("Or create a .env file with: RPC_URL=your_rpc_endpoint");
        e
    })?;

    let rt = Runtime::new()?;

    rt.block_on(async {
        run_arbitrage_monitor(config).await
    })
}

/// Main arbitrage monitoring loop
async fn run_arbitrage_monitor(config: Config) -> Result<(), Box<dyn Error>> {
    // Set up provider
    let provider = ProviderBuilder::new().connect_http(config.rpc_url.parse()?);

    // Initialize MOE protocol
    let moe_protocol = MoeProtocol::new();
    
    // Validate triangular arbitrage setup
    moe_protocol.validate_triangular_setup()
        .map_err(|e| format!("Triangular arbitrage setup validation failed: {}", e))?;

    // Get pool addresses
    let (moe_wmnt_addr, joe_moe_addr, joe_wmnt_addr) = moe_protocol.get_main_triangular_pools();
    let pool_addresses = vec![moe_wmnt_addr, joe_moe_addr, joe_wmnt_addr];

    // Initialize CSV file
    if let Err(e) = init_csv_file(&config.csv_file_path) {
        println!("⚠️ Warning: Failed to initialize CSV file: {}", e);
    } else {
        println!("📝 CSV logging initialized: {}", config.csv_file_path);
    }

    // Initialize cache
    let mut cache = ReservesCache::new();
    
    // Print startup information
    print_startup_banner();
    config.print_summary();
    println!();

    // Block-based monitoring loop
    loop {
        let start_time = Instant::now();
        
        // Get current block number
        let current_block = match get_current_block(&provider).await {
            Ok(block) => block,
            Err(e) => {
                println!("❌ Error getting block number: {}", e);
                sleep(Duration::from_secs(config.block_time_seconds)).await;
                continue;
            }
        };

        // Only fetch and process if block has changed
        if cache.has_changed(current_block) {
            // Fetch all reserves in parallel
            match fetch_all_reserves_with_retry(&provider, &pool_addresses, current_block, config.max_retries).await {
                Ok(reserves_map) => {
                    // Check if reserves have actually changed
                    if cache.reserves_changed(&reserves_map) {
                        let timestamp = Utc::now();
                        let fetch_duration = start_time.elapsed();
                        
                        // Print log with reserves information only when reserves change
                        println!("🔄 Reserves changed at {}", format_block_info(current_block, timestamp));
                        println!("{}", format_pool_reserves(moe_wmnt_addr, joe_moe_addr, joe_wmnt_addr, &reserves_map));

                        // Update cache
                        for (addr, reserves) in &reserves_map {
                            cache.update(*addr, reserves.clone());
                        }

                        // Use ternary search to find optimal arbitrage opportunity
                        let moe_wmnt_reserves = &reserves_map[&moe_wmnt_addr];
                        let joe_moe_reserves = &reserves_map[&joe_moe_addr];
                        let joe_wmnt_reserves = &reserves_map[&joe_wmnt_addr];

                        // Find optimal arbitrage using ternary search
                        match find_optimal_arbitrage(moe_wmnt_reserves, joe_moe_reserves, joe_wmnt_reserves, &config) {
                            Some(opportunity) => {
                                if opportunity.is_profitable() {
                                    log_profitable_arbitrage(&opportunity, fetch_duration, &config);

                                    // Write to CSV
                                    match write_arbitrage_to_csv(
                                        timestamp,
                                        current_block,
                                        &opportunity,
                                        moe_wmnt_reserves,
                                        joe_moe_reserves,
                                        joe_wmnt_reserves,
                                        fetch_duration.as_millis() as u64,
                                        &config,
                                    ) {
                                        Ok(_) => log_csv_success(&config.csv_file_path),
                                        Err(e) => log_csv_failure(e.as_ref()),
                                    }
                                } else {
                                    log_no_profit(opportunity.gross_profit, opportunity.net_profit, fetch_duration);
                                }
                            }
                            None => {
                                log_analysis_failure(fetch_duration);
                            }
                        }
                        println!(); // Add blank line for readability
                    } else {
                        // Update cache block number even if reserves didn't change
                        cache.update_block_number(current_block);
                    }
                }
                Err(e) => {
                    println!("❌ Block {}: Failed to fetch reserves: {}", current_block, e);
                }
            }
        }

        // Sleep until next expected block (with a small buffer)
        sleep(Duration::from_millis((config.block_time_seconds * 1000) - 200)).await;
    }
}