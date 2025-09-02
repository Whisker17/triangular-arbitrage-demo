use std::collections::HashMap;
use alloy::primitives::Address;
use alloy::providers::Provider;
use futures::future::join_all;
use crate::types::PoolReserves;
use crate::blockchain::fetch_all_reserves_with_retry;

/// Batch fetcher for pool reserves with parallel processing
pub struct BatchReservesFetcher {
    pool_addresses: Vec<Address>,
    max_retries: usize,
    batch_size: usize,
}

impl BatchReservesFetcher {
    /// Create new batch fetcher
    pub fn new(max_retries: usize) -> Self {
        Self {
            pool_addresses: Vec::new(),
            max_retries,
            batch_size: 50, // Process 50 pools per batch
        }
    }

    /// Load pool addresses from CSV
    pub fn load_pool_addresses_from_csv(&mut self, csv_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut reader = csv::Reader::from_path(csv_path)?;
        
        for result in reader.records() {
            let record = result?;
            if record.len() >= 3 {
                if let Ok(pool_addr) = record[2].parse::<Address>() {
                    self.pool_addresses.push(pool_addr);
                }
            }
        }
        
        println!("📊 Loaded {} pool addresses from CSV", self.pool_addresses.len());
        Ok(())
    }

    /// Get all pool addresses
    pub fn get_pool_addresses(&self) -> &[Address] {
        &self.pool_addresses
    }

    /// Add individual pool address
    pub fn add_pool_address(&mut self, address: Address) {
        if !self.pool_addresses.contains(&address) {
            self.pool_addresses.push(address);
        }
    }

    /// Fetch all reserves in parallel batches
    pub async fn fetch_all_reserves<P: Provider>(
        &self,
        provider: &P,
        current_block: u64,
    ) -> Result<HashMap<Address, PoolReserves>, Box<dyn std::error::Error>> {
        let mut all_reserves = HashMap::new();
        
        // Process pools in batches to avoid overwhelming the RPC
        for chunk in self.pool_addresses.chunks(self.batch_size) {
            let chunk_vec: Vec<Address> = chunk.to_vec();
            let batch_result = fetch_all_reserves_with_retry(provider, &chunk_vec, current_block, self.max_retries as u32).await;
            
            match batch_result {
                Ok(reserves_map) => {
                    // Merge all results from this batch
                    for (addr, reserves) in reserves_map {
                        all_reserves.insert(addr, reserves);
                    }
                }
                Err(e) => {
                    eprintln!("⚠️ Failed to fetch reserves for batch: {}", e);
                }
            }
            
            // Small delay between batches to be gentle on RPC
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        println!("✅ Successfully fetched reserves for {}/{} pools", 
                all_reserves.len(), self.pool_addresses.len());
        
        Ok(all_reserves)
    }

    /// Filter pools by minimum liquidity threshold
    pub fn filter_by_liquidity(&self, reserves_map: &HashMap<Address, PoolReserves>, min_liquidity_usd: f64) -> Vec<Address> {
        reserves_map
            .iter()
            .filter_map(|(&addr, reserves)| {
                // Estimate liquidity in USD (simplified calculation)
                let reserve_a_f64 = crate::math::u256_to_f64(reserves.reserve_a);
                let reserve_b_f64 = crate::math::u256_to_f64(reserves.reserve_b);
                
                // Rough estimate: assume both tokens have similar value
                let estimated_liquidity = (reserve_a_f64 + reserve_b_f64) * 0.5;
                
                if estimated_liquidity >= min_liquidity_usd {
                    Some(addr)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get pool count
    pub fn pool_count(&self) -> usize {
        self.pool_addresses.len()
    }

    /// Set batch size for parallel processing
    pub fn set_batch_size(&mut self, batch_size: usize) {
        self.batch_size = batch_size.max(1);
    }
}

/// Pool liquidity analyzer
pub struct LiquidityAnalyzer;

impl LiquidityAnalyzer {
    /// Analyze liquidity distribution across pools
    pub fn analyze_liquidity_distribution(reserves_map: &HashMap<Address, PoolReserves>) -> LiquidityStats {
        let mut total_liquidity = 0.0;
        let mut pool_liquidities = Vec::new();
        
        for reserves in reserves_map.values() {
            let reserve_a = crate::math::u256_to_f64(reserves.reserve_a);
            let reserve_b = crate::math::u256_to_f64(reserves.reserve_b);
            
            // Simple liquidity calculation (sum of reserves)
            let liquidity = reserve_a + reserve_b;
            pool_liquidities.push(liquidity);
            total_liquidity += liquidity;
        }
        
        pool_liquidities.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        
        let count = pool_liquidities.len();
        let mean = if count > 0 { total_liquidity / count as f64 } else { 0.0 };
        let median = if count > 0 {
            if count % 2 == 0 {
                (pool_liquidities[count / 2 - 1] + pool_liquidities[count / 2]) / 2.0
            } else {
                pool_liquidities[count / 2]
            }
        } else {
            0.0
        };
        
        let max_liquidity = pool_liquidities.first().copied().unwrap_or(0.0);
        let min_liquidity = pool_liquidities.last().copied().unwrap_or(0.0);

        LiquidityStats {
            total_pools: count,
            total_liquidity,
            mean_liquidity: mean,
            median_liquidity: median,
            max_liquidity,
            min_liquidity,
            top_10_pools: pool_liquidities.iter().take(10).copied().collect(),
        }
    }

    /// Get pools with sufficient liquidity for arbitrage
    pub fn get_arbitrage_ready_pools(
        reserves_map: &HashMap<Address, PoolReserves>,
        min_liquidity: f64,
    ) -> Vec<Address> {
        reserves_map
            .iter()
            .filter_map(|(&addr, reserves)| {
                let reserve_a = crate::math::u256_to_f64(reserves.reserve_a);
                let reserve_b = crate::math::u256_to_f64(reserves.reserve_b);
                
                // Check if both reserves meet minimum threshold
                if reserve_a >= min_liquidity && reserve_b >= min_liquidity {
                    Some(addr)
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Liquidity statistics
#[derive(Debug, Clone)]
pub struct LiquidityStats {
    pub total_pools: usize,
    pub total_liquidity: f64,
    pub mean_liquidity: f64,
    pub median_liquidity: f64,
    pub max_liquidity: f64,
    pub min_liquidity: f64,
    pub top_10_pools: Vec<f64>,
}

impl LiquidityStats {
    /// Print liquidity analysis
    pub fn print_analysis(&self) {
        println!("\n📊 Liquidity Analysis:");
        println!("├─ Total Pools: {}", self.total_pools);
        println!("├─ Total Liquidity: {:.2}", self.total_liquidity);
        println!("├─ Mean Liquidity: {:.2}", self.mean_liquidity);
        println!("├─ Median Liquidity: {:.2}", self.median_liquidity);
        println!("├─ Max Liquidity: {:.2}", self.max_liquidity);
        println!("├─ Min Liquidity: {:.2}", self.min_liquidity);
        println!("└─ Top 10 Pools by Liquidity:");
        
        for (i, liquidity) in self.top_10_pools.iter().enumerate() {
            let prefix = if i == self.top_10_pools.len() - 1 { "   └─" } else { "   ├─" };
            println!("{}  #{}: {:.2}", prefix, i + 1, liquidity);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_fetcher_creation() {
        let fetcher = BatchReservesFetcher::new(3);
        assert_eq!(fetcher.pool_count(), 0);
        assert_eq!(fetcher.max_retries, 3);
    }

    #[test]
    fn test_pool_address_management() {
        let mut fetcher = BatchReservesFetcher::new(3);
        
        let addr1 = Address::from([1u8; 20]);
        let addr2 = Address::from([2u8; 20]);
        
        fetcher.add_pool_address(addr1);
        fetcher.add_pool_address(addr2);
        fetcher.add_pool_address(addr1); // Duplicate
        
        assert_eq!(fetcher.pool_count(), 2);
        assert!(fetcher.get_pool_addresses().contains(&addr1));
        assert!(fetcher.get_pool_addresses().contains(&addr2));
    }

    #[test]
    fn test_batch_size_setting() {
        let mut fetcher = BatchReservesFetcher::new(3);
        
        fetcher.set_batch_size(100);
        assert_eq!(fetcher.batch_size, 100);
        
        fetcher.set_batch_size(0); // Should be clamped to minimum 1
        assert_eq!(fetcher.batch_size, 1);
    }
}
