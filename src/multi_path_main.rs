use std::error::Error;
use alloy::providers::ProviderBuilder;
use hex;
use alloy::primitives::Address;
use tokio::time::{sleep, Duration, Instant};
use chrono::Utc;

use crate::config::Config;
use crate::types::{Token, MultiPathOpportunity};
use crate::multi_path::{MultiPathAnalyzer, OptimizationStrategy, StrategySelector};
use crate::batch_fetcher::{BatchReservesFetcher, LiquidityAnalyzer};
use crate::blockchain::get_current_block;
use crate::constants::WMNT_ADDRESS;
use crate::logging::{init_csv_file, log_csv_success, log_csv_failure};
use crate::display::print_startup_banner;

/// Multi-path arbitrage monitoring system
pub async fn run_multi_path_arbitrage(config: Config) -> Result<(), Box<dyn Error>> {
    // Set up provider
    let provider = ProviderBuilder::new().connect_http(config.rpc_url.parse()?);

    // Create WMNT token for graph root
    let wmnt_token = Token::WMNT(Address::from_slice(
        &hex::decode(WMNT_ADDRESS.trim_start_matches("0x"))?
    ));

    // Initialize multi-path analyzer
    let mut analyzer = MultiPathAnalyzer::new(wmnt_token, &config);

    // Initialize batch fetcher
    let mut batch_fetcher = BatchReservesFetcher::new(config.max_retries as usize);

    // Load pools from CSV data
    let csv_path = "data/selected.csv";
    println!("🔄 Loading pools from: {}", csv_path);
    
    match batch_fetcher.load_pool_addresses_from_csv(csv_path) {
        Ok(()) => {
            println!("✅ Successfully loaded pool addresses");
        }
        Err(e) => {
            println!("❌ Failed to load pool addresses: {}", e);
            return Err(e);
        }
    }

    // Load pools into analyzer
    match analyzer.load_pools_from_csv(csv_path) {
        Ok(()) => {
            let (nodes, edges) = analyzer.get_graph_stats();
            println!("✅ Graph initialized: {} tokens, {} pools", nodes, edges);
        }
        Err(e) => {
            println!("❌ Failed to initialize graph: {}", e);
            return Err(e);
        }
    }

    // Initialize CSV logging
    if let Err(e) = init_csv_file(&config.csv_file_path) {
        println!("⚠️ Warning: Failed to initialize CSV file: {}", e);
    } else {
        println!("📝 CSV logging initialized: {}", config.csv_file_path);
    }

    // Print startup information
    print_startup_banner();
    config.print_summary();
    println!("\n🚀 Multi-Path Arbitrage Monitor Started");
    println!("├─ Pool Count: {}", batch_fetcher.pool_count());
    let all_paths = analyzer.get_all_paths();
    println!("├─ Available Paths: {}", all_paths.len());
    println!("│  ├─ 3-hop paths: {}", all_paths.iter().filter(|p| p.tokens.len() == 4).count());
    println!("│  └─ 4-hop paths: {}", all_paths.iter().filter(|p| p.tokens.len() == 5).count());
    println!("├─ Optimization Strategy: MaxProfit");
    println!("└─ Update Interval: {}s", config.block_time_seconds);
    println!();

    // Main monitoring loop
    let mut last_block = 0u64;
    let mut iteration_count = 0u64;

    loop {
        let start_time = Instant::now();
        iteration_count += 1;

        // Get current block number
        let current_block = match get_current_block(&provider).await {
            Ok(block) => block,
            Err(e) => {
                println!("❌ Error getting block number: {}", e);
                sleep(Duration::from_secs(config.block_time_seconds)).await;
                continue;
            }
        };

        // Only process if block has changed or it's the first iteration
        if current_block != last_block || iteration_count == 1 {
            last_block = current_block;

            println!("🔄 Block {} - Fetching reserves for {} pools...", current_block, batch_fetcher.pool_count());

            // Fetch all reserves in parallel
            match batch_fetcher.fetch_all_reserves(&provider, current_block).await {
                Ok(reserves_map) => {
                    let fetch_duration = start_time.elapsed();
                    println!("✅ Fetched {} pools in {:?}", reserves_map.len(), fetch_duration);

                    // Analyze liquidity
                    let _liquidity_stats = LiquidityAnalyzer::analyze_liquidity_distribution(&reserves_map);
                    
                    // Filter pools with sufficient liquidity
                    let min_liquidity = 1000.0; // Minimum liquidity threshold
                    let liquid_pools = LiquidityAnalyzer::get_arbitrage_ready_pools(&reserves_map, min_liquidity);
                    
                    println!("📊 Liquidity Analysis: {}/{} pools above ${} threshold", 
                            liquid_pools.len(), reserves_map.len(), min_liquidity);

                    // Update analyzer with new reserves
                    analyzer.update_pool_reserves(&reserves_map);

                    // Find all arbitrage opportunities
                    let analysis_start = Instant::now();
                    let multi_opportunity = analyzer.find_all_opportunities(
                        (100.0, 10000.0), // Input range in WMNT
                        config.ternary_search_iterations
                    );
                    let analysis_duration = analysis_start.elapsed();

                    // Process results
                    process_multi_path_results(
                        &multi_opportunity,
                        current_block,
                        fetch_duration,
                        analysis_duration,
                        &config,
                    ).await;

                }
                Err(e) => {
                    println!("❌ Block {}: Failed to fetch reserves: {}", current_block, e);
                }
            }
        }

        // Sleep until next iteration
        sleep(Duration::from_secs(config.block_time_seconds)).await;
    }
}

/// Process and display multi-path arbitrage results
async fn process_multi_path_results(
    multi_opportunity: &MultiPathOpportunity,
    block_number: u64,
    fetch_duration: Duration,
    analysis_duration: Duration,
    config: &Config,
) {
    let timestamp = Utc::now();
    
    println!("\n📈 Multi-Path Analysis Results (Block {}):", block_number);
    println!("├─ Fetch Time: {:?}", fetch_duration);
    println!("├─ Analysis Time: {:?}", analysis_duration);
    println!("├─ Total Opportunities: {}", multi_opportunity.opportunities.len());
    println!("├─ Profitable Opportunities: {}", multi_opportunity.profitable_count());

    if multi_opportunity.has_profitable_opportunities() {
        println!("└─ 💰 PROFITABLE OPPORTUNITIES FOUND!");
        
        // Select best opportunity using strategy
        let profitable_ops = multi_opportunity.profitable_opportunities();
        
        if let Some(best_opportunity) = StrategySelector::select_best(
            &profitable_ops.iter().cloned().cloned().collect::<Vec<_>>(),
            OptimizationStrategy::MaxProfit
        ) {
            println!("\n🎯 BEST OPPORTUNITY:");
            print_opportunity_details(best_opportunity);
            
            // Log to CSV if configured
            match write_multi_path_opportunity_to_csv(
                timestamp,
                block_number,
                best_opportunity,
                fetch_duration.as_millis() as u64,
                analysis_duration.as_millis() as u64,
                config,
            ) {
                Ok(_) => log_csv_success(&config.csv_file_path),
                Err(e) => log_csv_failure(e.as_ref()),
            }
        }

        // Show top 5 opportunities
        let mut sorted_ops = profitable_ops;
        sorted_ops.sort_by(|a, b| b.net_profit.partial_cmp(&a.net_profit).unwrap_or(std::cmp::Ordering::Equal));
        
        println!("\n📊 TOP 5 OPPORTUNITIES:");
        for (i, opportunity) in sorted_ops.iter().take(5).enumerate() {
            println!("{}. {} | Profit: {:.4} WMNT ({:.2}%) | Path: {}", 
                    i + 1,
                    opportunity.path.as_ref()
                        .map(|p| format!("{}-hop", p.tokens.len() - 1))
                        .unwrap_or_else(|| "Unknown".to_string()),
                    opportunity.net_profit,
                    opportunity.profit_percentage,
                    opportunity.path.as_ref()
                        .map(|p| p.description())
                        .unwrap_or_else(|| "Unknown path".to_string())
            );
        }
        
    } else {
        println!("└─ ❌ No profitable opportunities found");
        
        if !multi_opportunity.opportunities.is_empty() {
            // Show best non-profitable opportunity for analysis
            if let Some(best_attempt) = multi_opportunity.opportunities
                .iter()
                .max_by(|a, b| a.net_profit.partial_cmp(&b.net_profit).unwrap_or(std::cmp::Ordering::Equal))
            {
                println!("\n📊 Best Attempt (Non-profitable):");
                print_opportunity_details(best_attempt);
            }
        }
    }
    
    println!(); // Add spacing for readability
}

/// Print detailed opportunity information
fn print_opportunity_details(opportunity: &crate::types::ArbitrageOpportunity) {
    println!("├─ Input Amount: {:.4} WMNT", opportunity.optimal_input);
    println!("├─ Output Amount: {:.4} WMNT", opportunity.final_output);
    println!("├─ Gross Profit: {:.4} WMNT", opportunity.gross_profit);
    println!("├─ Net Profit: {:.4} WMNT", opportunity.net_profit);
    println!("├─ Profit %: {:.2}%", opportunity.profit_percentage);
    
    if let Some(path) = &opportunity.path {
        println!("├─ Path Type: {}-hop", path.tokens.len() - 1);
        println!("├─ Gas Units: {}", path.expected_gas_units());
        println!("└─ Route: {}", path.description());
    } else {
        println!("└─ Route: Legacy triangular");
    }
}

/// Write multi-path opportunity to CSV
fn write_multi_path_opportunity_to_csv(
    timestamp: chrono::DateTime<Utc>,
    block_number: u64,
    opportunity: &crate::types::ArbitrageOpportunity,
    fetch_time_ms: u64,
    analysis_time_ms: u64,
    config: &Config,
) -> Result<(), Box<dyn Error>> {
    use std::fs::OpenOptions;

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.csv_file_path)?;

    let mut writer = csv::Writer::from_writer(file);
    
    // Create enhanced record for multi-path arbitrage
    let record = crate::types::MultiPathArbitrageRecord {
        timestamp: timestamp.to_rfc3339(),
        block_number,
        optimal_input_wmnt: opportunity.optimal_input,
        final_output_wmnt: opportunity.final_output,
        gross_profit_wmnt: opportunity.gross_profit,
        net_profit_wmnt: opportunity.net_profit,
        profit_percentage: opportunity.profit_percentage,
        search_method: opportunity.search_method.clone(),
        path_type: opportunity.path.as_ref()
            .map(|p| format!("{}-hop", p.tokens.len() - 1))
            .unwrap_or_else(|| "legacy".to_string()),
        path_description: opportunity.path.as_ref()
            .map(|p| p.description())
            .unwrap_or_else(|| "WMNT -> MOE -> JOE -> WMNT".to_string()),
        gas_units: opportunity.path.as_ref()
            .map(|p| p.expected_gas_units())
            .unwrap_or(700_000),
        fetch_time_ms,
        analysis_time_ms,
    };

    writer.serialize(record)?;
    writer.flush()?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_path_main_functions_exist() {
        // Test that the main functions are properly defined
        // This is a compilation test
        assert!(true);
    }
}
