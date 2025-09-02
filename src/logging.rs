use std::error::Error;
use std::fs::OpenOptions;
use std::path::Path;
use csv::Writer;
use chrono::{DateTime, Utc};
use crate::types::{ArbitrageRecord, PoolReserves, ArbitrageOpportunity};
use crate::config::Config;

/// Initialize CSV file with headers if it doesn't exist
pub fn init_csv_file(csv_file_path: &str) -> Result<(), Box<dyn Error>> {
    if !Path::new(csv_file_path).exists() {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(csv_file_path)?;
        
        let mut writer = Writer::from_writer(file);
        
        // Write header if file is new
        writer.write_record(&[
            "timestamp", "block_number", "optimal_input_wmnt", "final_output_wmnt", 
            "gross_profit_wmnt", "net_profit_wmnt", "profit_percentage", "gas_cost_mnt", "search_method",
            "moe_wmnt_reserve0", "moe_wmnt_reserve1", 
            "joe_moe_reserve0", "joe_moe_reserve1",
            "joe_wmnt_reserve0", "joe_wmnt_reserve1", "fetch_time_ms"
        ])?;
        writer.flush()?;
    }
    Ok(())
}

/// Write arbitrage opportunity to CSV
pub fn write_arbitrage_to_csv(
    timestamp: DateTime<Utc>,
    block_number: u64,
    opportunity: &ArbitrageOpportunity,
    moe_wmnt_reserves: &PoolReserves,
    joe_moe_reserves: &PoolReserves,
    joe_wmnt_reserves: &PoolReserves,
    fetch_time_ms: u64,
    config: &Config,
) -> Result<(), Box<dyn Error>> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&config.csv_file_path)?;
    
    let mut writer = Writer::from_writer(file);
    
    let record = ArbitrageRecord {
        timestamp: timestamp.format("%Y-%m-%d %H:%M:%S%.3f UTC").to_string(),
        block_number,
        optimal_input_wmnt: opportunity.optimal_input,
        final_output_wmnt: opportunity.final_output,
        gross_profit_wmnt: opportunity.gross_profit,
        net_profit_wmnt: opportunity.net_profit,
        profit_percentage: opportunity.profit_percentage,
        gas_cost_mnt: opportunity.gas_cost(config.gas_price_gwei),
        search_method: opportunity.search_method.clone(),
        moe_wmnt_reserve0: moe_wmnt_reserves.reserve_a.to_string(),
        moe_wmnt_reserve1: moe_wmnt_reserves.reserve_b.to_string(),
        joe_moe_reserve0: joe_moe_reserves.reserve_a.to_string(),
        joe_moe_reserve1: joe_moe_reserves.reserve_b.to_string(),
        joe_wmnt_reserve0: joe_wmnt_reserves.reserve_a.to_string(),
        joe_wmnt_reserve1: joe_wmnt_reserves.reserve_b.to_string(),
        fetch_time_ms,
    };
    
    writer.serialize(&record)?;
    writer.flush()?;
    Ok(())
}

/// Log profitable arbitrage opportunity to console and CSV
pub fn log_profitable_arbitrage(
    opportunity: &ArbitrageOpportunity,
    fetch_duration: std::time::Duration,
    config: &Config,
) {
    println!("💎 OPTIMAL ARBITRAGE OPPORTUNITY FOUND!");
    println!("   🎯 Optimal Input: {:.6} WMNT (via {})", opportunity.optimal_input, opportunity.search_method);
    println!("   📈 Final Output: {:.6} WMNT", opportunity.final_output);
    println!("   💰 Gross Profit: {:.6} WMNT", opportunity.gross_profit);
    println!("   🎯 Net Profit: {:.6} WMNT ({:.2}%)", opportunity.net_profit, opportunity.profit_percentage);
    let gas_cost = if opportunity.hop_count() > 0 {
        opportunity.gas_cost(config.gas_price_gwei)
    } else {
        config.calculate_gas_cost(crate::constants::GAS_UNITS_3_HOPS) // Default to 3-hops for legacy
    };
    println!("   ⛽ After {:.6} MNT gas cost", gas_cost);
    println!("   🔍 Search iterations: {}", config.ternary_search_iterations);
    println!("   ⚡ Analysis time: {:?}", fetch_duration);
}

/// Log when no profitable opportunity is found
pub fn log_no_profit(
    gross_profit: f64,
    net_profit: f64,
    fetch_duration: std::time::Duration,
) {
    println!("   📊 No profitable opportunity after costs. Gross: {:.6}, Net: {:.6} WMNT, Time: {:?}", 
        gross_profit, net_profit, fetch_duration);
}

/// Log analysis failure
pub fn log_analysis_failure(fetch_duration: std::time::Duration) {
    println!("   ❌ Failed to analyze pools. Analysis time: {:?}", fetch_duration);
}

/// Log successful CSV write
pub fn log_csv_success(csv_file_path: &str) {
    println!("   ✅ Logged to CSV: {}", csv_file_path);
}

/// Log CSV write failure
pub fn log_csv_failure(error: &dyn Error) {
    println!("   ⚠️ Failed to write to CSV: {}", error);
}

/// Generic logger trait for future extensibility
pub trait ArbitrageLogger {
    fn log_opportunity(&self, opportunity: &ArbitrageOpportunity) -> Result<(), Box<dyn Error>>;
    fn log_reserves_change(&self, block_number: u64, timestamp: DateTime<Utc>);
    fn log_error(&self, error: &str);
    fn log_info(&self, message: &str);
}

/// Console logger implementation
pub struct ConsoleLogger;

impl ArbitrageLogger for ConsoleLogger {
    fn log_opportunity(&self, opportunity: &ArbitrageOpportunity) -> Result<(), Box<dyn Error>> {
        if opportunity.is_profitable() {
            println!("💎 Profitable opportunity: {:.6} WMNT profit ({:.2}%)", 
                opportunity.net_profit, opportunity.profit_percentage);
        } else {
            println!("📊 No profit after costs: {:.6} WMNT", opportunity.net_profit);
        }
        Ok(())
    }

    fn log_reserves_change(&self, block_number: u64, timestamp: DateTime<Utc>) {
        println!("🔄 Reserves changed at block {} ({})", block_number, timestamp.format("%H:%M:%S%.3f"));
    }

    fn log_error(&self, error: &str) {
        println!("❌ {}", error);
    }

    fn log_info(&self, message: &str) {
        println!("ℹ️ {}", message);
    }
}

/// File logger implementation
pub struct FileLogger {
    file_path: String,
}

impl FileLogger {
    pub fn new(file_path: String) -> Self {
        Self { file_path }
    }
}

impl ArbitrageLogger for FileLogger {
    fn log_opportunity(&self, opportunity: &ArbitrageOpportunity) -> Result<(), Box<dyn Error>> {
        use std::io::Write;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        
        writeln!(file, "{}: Opportunity - Profit: {:.6} WMNT ({:.2}%)", 
            Utc::now().format("%Y-%m-%d %H:%M:%S%.3f UTC"),
            opportunity.net_profit, 
            opportunity.profit_percentage)?;
        
        Ok(())
    }

    fn log_reserves_change(&self, block_number: u64, timestamp: DateTime<Utc>) {
        // Implementation for file logging
        let _ = self.log_info(&format!("Reserves changed at block {} ({})", 
            block_number, timestamp.format("%H:%M:%S%.3f")));
    }

    fn log_error(&self, error: &str) {
        let _ = self.log_info(&format!("ERROR: {}", error));
    }

    fn log_info(&self, message: &str) {
        use std::io::Write;
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path) {
            let _ = writeln!(file, "{}: {}", 
                Utc::now().format("%Y-%m-%d %H:%M:%S%.3f UTC"), message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_init_csv_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();
        
        // Remove the file so we can test creation
        std::fs::remove_file(path).unwrap();
        
        let result = init_csv_file(path);
        assert!(result.is_ok());
        assert!(Path::new(path).exists());
    }

    #[test]
    fn test_console_logger() {
        let logger = ConsoleLogger;
        let opportunity = ArbitrageOpportunity {
            optimal_input: 100.0,
            final_output: 105.0,
            gross_profit: 5.0,
            net_profit: 3.0,
            profit_percentage: 3.0,
            search_method: "test".to_string(),
            path: None,
        };

        let result = logger.log_opportunity(&opportunity);
        assert!(result.is_ok());
    }
}
