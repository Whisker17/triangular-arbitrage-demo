use std::collections::HashMap;
use alloy::primitives::{Address, U256};
use rayon::prelude::*;
use hex;
use crate::types::{
    Token, PoolReserves, ArbitrageOpportunity, ArbitragePath, MultiPathOpportunity
};
use crate::graph::TokenGraph;
use crate::math::find_best_input;
use crate::config::Config;

/// Multi-path arbitrage analyzer
pub struct MultiPathAnalyzer {
    graph: TokenGraph,
    wmnt_token: Token,
    dex_fee: f64,
    gas_price_gwei: f64,
}

impl MultiPathAnalyzer {
    /// Create a new multi-path analyzer
    pub fn new(wmnt_token: Token, config: &Config) -> Self {
        Self {
            graph: TokenGraph::new(wmnt_token),
            wmnt_token,
            dex_fee: config.dex_fee,
            gas_price_gwei: config.gas_price_gwei,
        }
    }

    /// Load pools from CSV data file
    pub fn load_pools_from_csv(&mut self, csv_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut reader = csv::Reader::from_path(csv_path)?;
        
        for result in reader.records() {
            let record = result?;
            
            // Parse CSV record: Protocol,Pair Name,Pair Address,TokenA Reserves,TokenB Reserves
            if record.len() >= 5 {
                let pair_address = record[2].parse::<Address>();
                if let Ok(pool_addr) = pair_address {
                    // Parse token symbols from pair name
                    let pair_name = &record[1];
                    if let Some((token_a, token_b)) = self.parse_token_pair(pair_name) {
                        let reserve_a_str = &record[3];
                        let reserve_b_str = &record[4];
                        
                        if let (Ok(reserve_a), Ok(reserve_b)) = (
                            reserve_a_str.parse::<f64>(),
                            reserve_b_str.parse::<f64>()
                        ) {
                            // Convert to wei (assuming reserves are in token units)
                            let reserve_a_wei = U256::from((reserve_a * 1e18) as u128);
                            let reserve_b_wei = U256::from((reserve_b * 1e18) as u128);
                            
                            let pool_reserves = PoolReserves::new(
                                token_a,
                                reserve_a_wei,
                                token_b,
                                reserve_b_wei,
                                0, // block number will be updated later
                                pool_addr,
                            );
                            
                            self.graph.add_pool(&pool_reserves, self.dex_fee);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Parse token pair from pair name (e.g., "MOE-WMNT" -> (MOE, WMNT))
    fn parse_token_pair(&self, pair_name: &str) -> Option<(Token, Token)> {
        let parts: Vec<&str> = pair_name.split('-').collect();
        if parts.len() != 2 {
            return None;
        }
        
        let token_a = self.symbol_to_token(parts[0])?;
        let token_b = self.symbol_to_token(parts[1])?;
        
        Some((token_a, token_b))
    }

    /// Convert symbol string to Token enum
    fn symbol_to_token(&self, symbol: &str) -> Option<Token> {
        match symbol {
            "WMNT" => Some(Token::WMNT(Address::from_slice(&hex::decode(crate::constants::WMNT_ADDRESS.trim_start_matches("0x")).ok()?))),
            "MOE" => Some(Token::MOE(Address::from_slice(&hex::decode(crate::constants::MOE_ADDRESS.trim_start_matches("0x")).ok()?))),
            "JOE" => Some(Token::JOE(Address::from_slice(&hex::decode(crate::constants::JOE_ADDRESS.trim_start_matches("0x")).ok()?))),
            // Add other tokens as needed based on CSV data
            "mETH" | "PUFF" | "MINU" | "LEND" => {
                // For now, skip unknown tokens
                None
            }
            _ => None,
        }
    }

    /// Update pool reserves with new data
    pub fn update_pool_reserves(&mut self, reserves_map: &HashMap<Address, PoolReserves>) {
        for pool_reserves in reserves_map.values() {
            self.graph.update_pool(pool_reserves);
        }
    }

    /// Find all arbitrage opportunities across multiple paths
    pub fn find_all_opportunities(&self, input_range: (f64, f64), iterations: usize) -> MultiPathOpportunity {
        let start_time = std::time::Instant::now();
        
        // Find all arbitrage cycles (3-hops and 4-hops)
        let cycles = self.graph.find_arbitrage_cycles(4);
        
        // Analyze each cycle in parallel for maximum performance
        let opportunities: Vec<ArbitrageOpportunity> = cycles
            .par_iter()
            .filter_map(|cycle| self.analyze_cycle(cycle, input_range, iterations))
            .collect();
        
        let analysis_time_ms = start_time.elapsed().as_millis() as u64;
        
        MultiPathOpportunity::new(opportunities, analysis_time_ms)
    }

    /// Analyze a specific arbitrage cycle for profitability
    fn analyze_cycle(
        &self,
        cycle: &ArbitragePath,
        _input_range: (f64, f64),
        iterations: usize,
    ) -> Option<ArbitrageOpportunity> {
        // Convert cycle to pool format for ternary search
        let pools = self.cycle_to_pools(cycle)?;
        
        // Use ternary search to find optimal input amount
        let (optimal_input, gross_profit) = find_best_input(&pools, self.dex_fee, iterations);
        
        // Calculate final output
        let final_output = optimal_input + gross_profit;
        
        // Calculate gas cost based on path type
        let gas_cost = self.calculate_gas_cost(cycle);
        
        // Calculate net profit after gas costs
        let net_profit = gross_profit - gas_cost;
        
        // Calculate profit percentage
        let profit_percentage = if optimal_input > 0.0 {
            (net_profit / optimal_input) * 100.0
        } else {
            0.0
        };
        
        Some(ArbitrageOpportunity {
            optimal_input,
            final_output,
            gross_profit,
            net_profit,
            profit_percentage,
            search_method: "multi_path_ternary".to_string(),
            path: Some(cycle.clone()),
        })
    }

    /// Convert arbitrage cycle to pools format for mathematical analysis
    fn cycle_to_pools(&self, cycle: &ArbitragePath) -> Option<Vec<(f64, f64)>> {
        let mut pools = Vec::new();
        
        for i in 0..cycle.tokens.len() - 1 {
            let token_in = cycle.tokens[i];
            let token_out = cycle.tokens[i + 1];
            
            let pool_info = self.graph.get_pool_info(token_in, token_out)?;
            
            let (reserve_in, reserve_out) = if pool_info.token_a == token_in {
                (pool_info.reserves_a, pool_info.reserves_b)
            } else {
                (pool_info.reserves_b, pool_info.reserves_a)
            };
            
            pools.push((reserve_in, reserve_out));
        }
        
        // Add the closing pool (back to WMNT)
        if let Some(last_token) = cycle.tokens.last() {
            if *last_token != self.wmnt_token {
                let pool_info = self.graph.get_pool_info(*last_token, self.wmnt_token)?;
                
                let (reserve_in, reserve_out) = if pool_info.token_a == *last_token {
                    (pool_info.reserves_a, pool_info.reserves_b)
                } else {
                    (pool_info.reserves_b, pool_info.reserves_a)
                };
                
                pools.push((reserve_in, reserve_out));
            }
        }
        
        Some(pools)
    }

    /// Calculate gas cost for a specific cycle (result in MNT)
    fn calculate_gas_cost(&self, cycle: &ArbitragePath) -> f64 {
        let gas_units = cycle.expected_gas_units();
        
        // Direct calculation: gas_units * gas_price_gwei * gwei_to_mnt_multiplier
        use crate::constants::GWEI_TO_MNT_MULTIPLIER;
        gas_units as f64 * self.gas_price_gwei * GWEI_TO_MNT_MULTIPLIER
    }

    /// Get graph statistics
    pub fn get_graph_stats(&self) -> (usize, usize) {
        (self.graph.node_count(), self.graph.edge_count())
    }

    /// Get all available arbitrage paths
    pub fn get_all_paths(&self) -> Vec<ArbitragePath> {
        self.graph.find_arbitrage_cycles(4)
    }
}

/// Batch reserves fetcher for all pools
pub struct BatchReservesFetcher {
    pool_addresses: Vec<Address>,
}

impl BatchReservesFetcher {
    /// Create new batch fetcher
    pub fn new() -> Self {
        Self {
            pool_addresses: Vec::new(),
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
}

/// Strategy for selecting optimal arbitrage opportunity
pub enum OptimizationStrategy {
    MaxProfit,
    MaxProfitPercent,
    MinRisk,
    BalancedRiskReturn,
}

/// Multi-path strategy selector
pub struct StrategySelector;

impl StrategySelector {
    /// Select best opportunity based on strategy
    pub fn select_best(
        opportunities: &[ArbitrageOpportunity],
        strategy: OptimizationStrategy,
    ) -> Option<&ArbitrageOpportunity> {
        if opportunities.is_empty() {
            return None;
        }

        match strategy {
            OptimizationStrategy::MaxProfit => {
                opportunities
                    .iter()
                    .filter(|opp| opp.is_profitable())
                    .max_by(|a, b| a.net_profit.partial_cmp(&b.net_profit).unwrap_or(std::cmp::Ordering::Equal))
            }
            OptimizationStrategy::MaxProfitPercent => {
                opportunities
                    .iter()
                    .filter(|opp| opp.is_profitable())
                    .max_by(|a, b| a.profit_percentage.partial_cmp(&b.profit_percentage).unwrap_or(std::cmp::Ordering::Equal))
            }
            OptimizationStrategy::MinRisk => {
                // Lower hop count = lower risk
                opportunities
                    .iter()
                    .filter(|opp| opp.is_profitable())
                    .min_by(|a, b| a.hop_count().cmp(&b.hop_count()))
            }
            OptimizationStrategy::BalancedRiskReturn => {
                // Weighted score: profit / (hop_count^2)
                opportunities
                    .iter()
                    .filter(|opp| opp.is_profitable())
                    .max_by(|a, b| {
                        let score_a = a.net_profit / (a.hop_count() as f64).powi(2);
                        let score_b = b.net_profit / (b.hop_count() as f64).powi(2);
                        score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
                    })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Token;
    use alloy::primitives::Address;

    fn create_test_config() -> Config {
        Config {
            rpc_url: "test".to_string(),
            csv_file_path: "test.csv".to_string(),
            dex_fee: 0.003,
            ternary_search_iterations: 100,
            gas_price_gwei: 0.02,
            block_time_seconds: 2,
            max_retries: 3,
        }
    }

    #[test]
    fn test_multi_path_analyzer_creation() {
        let wmnt = Token::WMNT(Address::ZERO);
        let config = create_test_config();
        let analyzer = MultiPathAnalyzer::new(wmnt, &config);
        
        let (nodes, edges) = analyzer.get_graph_stats();
        assert_eq!(nodes, 0); // No tokens added initially 
        assert_eq!(edges, 0);
    }

    #[test]
    fn test_batch_reserves_fetcher() {
        let mut fetcher = BatchReservesFetcher::new();
        
        let addr1 = Address::from([1u8; 20]);
        let addr2 = Address::from([2u8; 20]);
        
        fetcher.add_pool_address(addr1);
        fetcher.add_pool_address(addr2);
        fetcher.add_pool_address(addr1); // Duplicate should be ignored
        
        assert_eq!(fetcher.get_pool_addresses().len(), 2);
    }

    #[test]
    fn test_strategy_selector() {
        let opportunities = vec![
            ArbitrageOpportunity {
                optimal_input: 100.0,
                final_output: 105.0,
                gross_profit: 5.0,
                net_profit: 4.0,
                profit_percentage: 4.0,
                search_method: "test".to_string(),
                path: None,
            },
            ArbitrageOpportunity {
                optimal_input: 200.0,
                final_output: 210.0,
                gross_profit: 10.0,
                net_profit: 8.0,
                profit_percentage: 4.0,
                search_method: "test".to_string(),
                path: None,
            },
        ];

        let best = StrategySelector::select_best(&opportunities, OptimizationStrategy::MaxProfit);
        assert!(best.is_some());
        assert_eq!(best.unwrap().net_profit, 8.0);
    }
}
