use std::collections::HashMap;
use std::error::Error;
use alloy::providers::Provider;
use alloy::primitives::{Address, U256};
use tokio::time::{sleep, Duration};
use crate::types::{Token, PoolReserves};

// Define the MoePair interface using alloy's sol! macro
alloy::sol!(
    #[sol(rpc)]
    interface IMoePair {
        function token0() external view returns (address);
        function token1() external view returns (address);
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }
);

/// Fetch reserves for a single pool
pub async fn fetch_pool_reserves<P: Provider + Clone>(
    provider: P,
    pool_address: Address,
    block_number: u64,
) -> Result<PoolReserves, Box<dyn Error>> {
    let contract = IMoePair::new(pool_address, provider.clone());

    // Fetch token0 and token1
    let token0_addr = contract.token0().call().await?;
    let token1_addr = contract.token1().call().await?;

    // Map to our Token enum
    let token0 = Token::from_address(token0_addr)
        .ok_or("Unknown token0")?;
    let token1 = Token::from_address(token1_addr)
        .ok_or("Unknown token1")?;

    // Fetch reserves
    let reserves = contract.getReserves().call().await?;
    let reserve0 = U256::from(reserves.reserve0);
    let reserve1 = U256::from(reserves.reserve1);

    Ok(PoolReserves::new(
        token0,
        reserve0,
        token1,
        reserve1,
        block_number,
        pool_address,
    ))
}

/// Parallel fetch all pool reserves with retry mechanism
pub async fn fetch_all_reserves_with_retry<P: Provider + Clone>(
    provider: P,
    pool_addresses: &[Address],
    block_number: u64,
    max_retries: u32,
) -> Result<HashMap<Address, PoolReserves>, Box<dyn Error>> {
    let mut attempts = 0;
    
    while attempts < max_retries {
        let futures: Vec<_> = pool_addresses.iter().map(|&addr| {
            let provider_clone = provider.clone();
            async move {
                (addr, fetch_pool_reserves(provider_clone, addr, block_number).await)
            }
        }).collect();

        let results = futures::future::join_all(futures).await;
        let mut reserves_map = HashMap::new();
        let mut all_succeeded = true;

        for (addr, result) in results {
            match result {
                Ok(reserves) => {
                    reserves_map.insert(addr, reserves);
                }
                Err(e) => {
                    println!("Error fetching reserves for pool {}: {}", addr, e);
                    all_succeeded = false;
                    break;
                }
            }
        }

        if all_succeeded {
            return Ok(reserves_map);
        }

        attempts += 1;
        if attempts < max_retries {
            println!("Retrying... attempt {} of {}", attempts + 1, max_retries);
            sleep(Duration::from_secs(1)).await;
        }
    }

    Err("Failed to fetch reserves after maximum retries".into())
}

/// Get current block number from provider
pub async fn get_current_block<P: Provider>(provider: &P) -> Result<u64, Box<dyn Error>> {
    Ok(provider.get_block_number().await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Note: These tests would require a mock provider or integration test setup
    // For now, we'll just test the basic functionality
    
    #[test]
    fn test_token_from_address() {
        let wmnt_addr: Address = crate::constants::WMNT_ADDRESS.parse().unwrap();
        let token = Token::from_address(wmnt_addr);
        assert!(token.is_some());
        
        match token.unwrap() {
            Token::WMNT(_) => {},
            _ => panic!("Expected WMNT token"),
        }
    }
}
