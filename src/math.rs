use alloy::primitives::U256;

/// More accurate swap function using constant product formula (x*y=k)
pub fn swap(x_reserve: f64, y_reserve: f64, dx: f64, fee: f64) -> f64 {
    if dx <= 0.0 || x_reserve <= 0.0 || y_reserve <= 0.0 {
        return 0.0;
    }
    let dx_after_fee = dx * (1.0 - fee);
    (y_reserve * dx_after_fee) / (x_reserve + dx_after_fee)
}

/// Calculate arbitrage profit for a given input amount
pub fn arbitrage_profit(
    dx: f64,
    pools: &[(f64, f64)], // [(x,y), (x,y), (x,y)] - 3 pools in the arbitrage path
    fee: f64,
) -> f64 {
    if pools.len() != 3 {
        return -1.0; // Invalid input
    }
    
    // pool1: token0 -> token1
    let dy1 = swap(pools[0].0, pools[0].1, dx, fee);
    // pool2: token1 -> token2  
    let dy2 = swap(pools[1].0, pools[1].1, dy1, fee);
    // pool3: token2 -> token0
    let dy3 = swap(pools[2].0, pools[2].1, dy2, fee);

    dy3 - dx // profit (can be negative)
}

/// Find optimal input amount using ternary search
pub fn find_best_input(
    pools: &[(f64, f64)], // 3 pools
    fee: f64,
    iterations: usize,
) -> (f64, f64) {
    let mut left = 0.0;
    let mut right = pools[0].0 * 0.999; // Upper limit close to pool's total token0 reserves
    
    // Ternary search for maximum profit
    for _ in 0..iterations {
        let m1 = left + (right - left) / 3.0;
        let m2 = right - (right - left) / 3.0;
        let p1 = arbitrage_profit(m1, pools, fee);
        let p2 = arbitrage_profit(m2, pools, fee);
        
        if p1 < p2 {
            left = m1;
        } else {
            right = m2;
        }
    }
    
    let best_input = (left + right) / 2.0;
    let best_profit = arbitrage_profit(best_input, pools, fee);
    (best_input, best_profit)
}

/// Helper function to convert U256 to f64 (in token units, not wei)
pub fn u256_to_f64(value: U256) -> f64 {
    value.to_string().parse::<f64>().unwrap_or(0.0) / 1e18
}

/// Helper function to convert f64 to U256 (from token units to wei)
pub fn f64_to_u256(value: f64) -> U256 {
    U256::from((value * 1e18) as u128)
}

/// Legacy function for compatibility (kept for potential future use)
/// Calculate output amount using Uniswap V2 formula
pub fn get_amount_out(amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
    if amount_in == U256::ZERO || reserve_in == U256::ZERO || reserve_out == U256::ZERO {
        return U256::ZERO;
    }
    let amount_in_with_fee = amount_in * U256::from(997u64);
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in * U256::from(1000u64) + amount_in_with_fee;
    numerator / denominator
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_basic() {
        let result = swap(1000.0, 1000.0, 100.0, 0.003);
        assert!(result > 0.0);
        assert!(result < 100.0); // Should be less than input due to slippage and fees
    }

    #[test]
    fn test_arbitrage_profit_no_profit() {
        let pools = vec![(1000.0, 1000.0), (1000.0, 1000.0), (1000.0, 1000.0)];
        let profit = arbitrage_profit(100.0, &pools, 0.003);
        assert!(profit < 0.0); // Should be negative due to fees
    }

    #[test]
    fn test_find_best_input() {
        let pools = vec![(1000.0, 1000.0), (1000.0, 1000.0), (1000.0, 1000.0)];
        let (best_input, best_profit) = find_best_input(&pools, 0.003, 100);
        assert!(best_input >= 0.0);
        assert!(best_profit <= 0.0); // Should be negative or zero for equal pools with fees
    }

    #[test] 
    fn test_u256_conversion() {
        let value = U256::from(1000000000000000000u128); // 1 token in wei
        let converted = u256_to_f64(value);
        assert_eq!(converted, 1.0);
        
        let back = f64_to_u256(converted);
        assert_eq!(back, value);
    }
}
