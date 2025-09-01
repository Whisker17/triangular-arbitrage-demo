use std::collections::HashMap;
use alloy::primitives::Address;
use crate::types::PoolReserves;

/// Cache structure for pool reserves to avoid unnecessary refetching
#[derive(Debug, Clone)]
pub struct ReservesCache {
    data: HashMap<Address, PoolReserves>,
    last_block: u64,
}

impl ReservesCache {
    /// Create a new empty cache
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            last_block: 0,
        }
    }

    /// Get cached reserves for a pool address
    pub fn get(&self, address: &Address) -> Option<&PoolReserves> {
        self.data.get(address)
    }

    /// Update the cache with new reserves
    pub fn update(&mut self, address: Address, reserves: PoolReserves) {
        self.last_block = reserves.block_number;
        self.data.insert(address, reserves);
    }

    /// Check if the block has changed since last update
    pub fn has_changed(&self, block_number: u64) -> bool {
        block_number > self.last_block
    }

    /// Check if any reserves have actually changed compared to cache
    pub fn reserves_changed(&self, new_reserves: &HashMap<Address, PoolReserves>) -> bool {
        if self.data.len() != new_reserves.len() {
            return true;
        }
        
        for (addr, new_reserve) in new_reserves {
            match self.data.get(addr) {
                Some(cached_reserve) => {
                    if cached_reserve.reserve_a != new_reserve.reserve_a 
                        || cached_reserve.reserve_b != new_reserve.reserve_b {
                        return true;
                    }
                }
                None => return true,
            }
        }
        false
    }

    /// Update the last block number (used when reserves didn't change but block advanced)
    pub fn update_block_number(&mut self, block_number: u64) {
        self.last_block = block_number;
    }

    /// Get the last cached block number
    pub fn get_last_block(&self) -> u64 {
        self.last_block
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.data.clear();
        self.last_block = 0;
    }

    /// Get all cached reserves
    pub fn get_all(&self) -> &HashMap<Address, PoolReserves> {
        &self.data
    }
}

impl Default for ReservesCache {
    fn default() -> Self {
        Self::new()
    }
}
