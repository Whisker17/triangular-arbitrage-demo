use alloy::primitives::U256;
use crate::types::{Token, PoolReserves, ArbitrageOpportunity};
use crate::config::Config;
use crate::math::{u256_to_f64, find_best_input, get_amount_out};
use crate::constants::{GAS_UNITS_3_HOPS, DEFAULT_GAS_PRICE_GWEI, GWEI_TO_MNT_MULTIPLIER};

/// Extract and normalize pool reserves for ternary search algorithm
fn prepare_pools_for_search(
    moe_wmnt: &PoolReserves,
    joe_moe: &PoolReserves,
    joe_wmnt: &PoolReserves,
) -> Option<Vec<(f64, f64)>> {
    // Pool 1: WMNT -> MOE in MOE-WMNT pool
    let (wmnt_reserve1, moe_reserve1) = match (moe_wmnt.token_a, moe_wmnt.token_b) {
        (Token::WMNT(_), Token::MOE(_)) => (u256_to_f64(moe_wmnt.reserve_a), u256_to_f64(moe_wmnt.reserve_b)),
        (Token::MOE(_), Token::WMNT(_)) => (u256_to_f64(moe_wmnt.reserve_b), u256_to_f64(moe_wmnt.reserve_a)),
        _ => return None,
    };

    // Pool 2: MOE -> JOE in JOE-MOE pool
    let (moe_reserve2, joe_reserve2) = match (joe_moe.token_a, joe_moe.token_b) {
        (Token::MOE(_), Token::JOE(_)) => (u256_to_f64(joe_moe.reserve_a), u256_to_f64(joe_moe.reserve_b)),
        (Token::JOE(_), Token::MOE(_)) => (u256_to_f64(joe_moe.reserve_b), u256_to_f64(joe_moe.reserve_a)),
        _ => return None,
    };

    // Pool 3: JOE -> WMNT in JOE-WMNT pool
    let (joe_reserve3, wmnt_reserve3) = match (joe_wmnt.token_a, joe_wmnt.token_b) {
        (Token::JOE(_), Token::WMNT(_)) => (u256_to_f64(joe_wmnt.reserve_a), u256_to_f64(joe_wmnt.reserve_b)),
        (Token::WMNT(_), Token::JOE(_)) => (u256_to_f64(joe_wmnt.reserve_b), u256_to_f64(joe_wmnt.reserve_a)),
        _ => return None,
    };

    Some(vec![
        (wmnt_reserve1, moe_reserve1),    // Pool 1: WMNT -> MOE
        (moe_reserve2, joe_reserve2),     // Pool 2: MOE -> JOE
        (joe_reserve3, wmnt_reserve3),    // Pool 3: JOE -> WMNT
    ])
}

/// New optimized arbitrage function using ternary search
pub fn find_optimal_arbitrage(
    moe_wmnt: &PoolReserves,
    joe_moe: &PoolReserves,
    joe_wmnt: &PoolReserves,
    config: &Config,
) -> Option<ArbitrageOpportunity> {
    let pools = prepare_pools_for_search(moe_wmnt, joe_moe, joe_wmnt)?;
    
    let (best_input, gross_profit) = find_best_input(&pools, config.dex_fee, config.ternary_search_iterations);
    
    // Calculate final output amount
    let final_output = best_input + gross_profit;
    
    // Calculate net profit after precise gas costs (3-hops for triangular arbitrage)
    let gas_cost = config.calculate_gas_cost(GAS_UNITS_3_HOPS);
    let net_profit = gross_profit - gas_cost;
    
    // Calculate profit percentage
    let profit_percentage = if best_input > 0.0 { 
        (net_profit / best_input) * 100.0 
    } else { 
        0.0 
    };
    
            Some(ArbitrageOpportunity {
            optimal_input: best_input,
            final_output,
            gross_profit,
            net_profit,
            profit_percentage,
            search_method: "ternary_search".to_string(),
            path: None, // Legacy triangular arbitrage doesn't use path structure
        })
}

/// Legacy arbitrage function (kept for potential comparison/debugging)
pub fn check_arbitrage_legacy(
    moe_wmnt: &PoolReserves,
    joe_moe: &PoolReserves,
    joe_wmnt: &PoolReserves,
    start_amount: U256,
) -> (bool, U256, U256) {
    // Path: WMNT -> MOE -> JOE -> WMNT

    // Step 1: WMNT -> MOE in MOE-WMNT pool
    let (wmnt_reserve, moe_reserve) = match (moe_wmnt.token_a, moe_wmnt.token_b) {
        (Token::WMNT(_), Token::MOE(_)) => (moe_wmnt.reserve_a, moe_wmnt.reserve_b),
        (Token::MOE(_), Token::WMNT(_)) => (moe_wmnt.reserve_b, moe_wmnt.reserve_a),
        _ => return (false, U256::ZERO, U256::ZERO),
    };
    let moe_out = get_amount_out(start_amount, wmnt_reserve, moe_reserve);

    // Step 2: MOE -> JOE in JOE-MOE pool
    let (moe_reserve, joe_reserve) = match (joe_moe.token_a, joe_moe.token_b) {
        (Token::MOE(_), Token::JOE(_)) => (joe_moe.reserve_a, joe_moe.reserve_b),
        (Token::JOE(_), Token::MOE(_)) => (joe_moe.reserve_b, joe_moe.reserve_a),
        _ => return (false, U256::ZERO, U256::ZERO),
    };
    let joe_out = get_amount_out(moe_out, moe_reserve, joe_reserve);

    // Step 3: JOE -> WMNT in JOE-WMNT pool
    let (joe_reserve, wmnt_reserve) = match (joe_wmnt.token_a, joe_wmnt.token_b) {
        (Token::JOE(_), Token::WMNT(_)) => (joe_wmnt.reserve_a, joe_wmnt.reserve_b),
        (Token::WMNT(_), Token::JOE(_)) => (joe_wmnt.reserve_b, joe_wmnt.reserve_a),
        _ => return (false, U256::ZERO, U256::ZERO),
    };
    let wmnt_out = get_amount_out(joe_out, joe_reserve, wmnt_reserve);

    // Calculate precise gas cost in wei (3-hops for legacy triangular arbitrage)
    let gas_cost_mnt = GAS_UNITS_3_HOPS as f64 * DEFAULT_GAS_PRICE_GWEI * GWEI_TO_MNT_MULTIPLIER;
    let tx_cost = U256::from((gas_cost_mnt * 1e18) as u128);
    
    // Calculate net profit after transaction costs
    let net_profit = if wmnt_out > start_amount + tx_cost {
        wmnt_out - start_amount - tx_cost
    } else {
        U256::ZERO
    };
    
    // Check if profitable after considering transaction costs
    let profitable = net_profit > U256::ZERO;
    (profitable, wmnt_out, net_profit)
}

/// Generic arbitrage path finder (for future expansion to support multiple paths)
pub trait ArbitragePath {
    fn calculate_output(&self, input_amount: f64, reserves: &[&PoolReserves]) -> Option<f64>;
    fn get_path_description(&self) -> String;
}

/// Standard 3-pool triangular arbitrage path
pub struct TriangularPath {
    pub path: Vec<Token>,
}

impl ArbitragePath for TriangularPath {
    fn calculate_output(&self, _input_amount: f64, _reserves: &[&PoolReserves]) -> Option<f64> {
        // Implementation would go here for generic path calculation
        // This is a placeholder for future expansion
        None
    }

    fn get_path_description(&self) -> String {
        self.path
            .iter()
            .map(|t| t.symbol())
            .collect::<Vec<_>>()
            .join(" -> ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Token;
    use alloy::primitives::Address;
    use chrono::Utc;

    fn create_test_reserves(token_a: Token, reserve_a: u128, token_b: Token, reserve_b: u128) -> PoolReserves {
        PoolReserves {
            token_a,
            reserve_a: U256::from(reserve_a * 1_000_000_000_000_000_000u128), // Convert to wei
            token_b,
            reserve_b: U256::from(reserve_b * 1_000_000_000_000_000_000u128),
            block_number: 1,
            timestamp: Utc::now(),
            pool_address: Address::ZERO,
        }
    }

    #[test]
    fn test_prepare_pools_for_search() {
        let wmnt_addr = Address::ZERO;
        let moe_addr = Address::from([1u8; 20]);
        let joe_addr = Address::from([2u8; 20]);

        let moe_wmnt = create_test_reserves(Token::WMNT(wmnt_addr), 1000, Token::MOE(moe_addr), 1000);
        let joe_moe = create_test_reserves(Token::MOE(moe_addr), 1000, Token::JOE(joe_addr), 1000);
        let joe_wmnt = create_test_reserves(Token::JOE(joe_addr), 1000, Token::WMNT(wmnt_addr), 1000);

        let pools = prepare_pools_for_search(&moe_wmnt, &joe_moe, &joe_wmnt);
        assert!(pools.is_some());
        
        let pools = pools.unwrap();
        assert_eq!(pools.len(), 3);
        assert_eq!(pools[0], (1000.0, 1000.0)); // WMNT -> MOE
        assert_eq!(pools[1], (1000.0, 1000.0)); // MOE -> JOE
        assert_eq!(pools[2], (1000.0, 1000.0)); // JOE -> WMNT
    }

    #[test]
    fn test_triangular_path() {
        let wmnt_addr = Address::ZERO;
        let moe_addr = Address::from([1u8; 20]);
        let joe_addr = Address::from([2u8; 20]);

        let path = TriangularPath {
            path: vec![Token::WMNT(wmnt_addr), Token::MOE(moe_addr), Token::JOE(joe_addr)],
        };

        assert_eq!(path.get_path_description(), "WMNT -> MOE -> JOE");
    }
}
