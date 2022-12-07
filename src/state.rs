use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    BlockInfo, Addr, Order, Uint128, Storage, StdResult,
};
use cw_storage_plus::{ 
    Map, Bound,
};

use cw20::{Expiration};

// swap type of true equals offer, swap type of false equals buy
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct CW721Swap {    
    pub creator: Addr,
    pub nft_contract: Addr,
    pub payment_token:Addr,
    pub token_id:String,    
    pub expires: Expiration,    
    pub price: Uint128,
    pub swap_type:bool,
}
pub fn all_swap_ids<'a>(
    storage: &dyn Storage,
    start: Option<Bound<'a, &'a str>>,
    limit: usize,
) -> StdResult<Vec<String>> {
    SWAPS
        .keys(storage, start, None, Order::Ascending)
        .take(limit)
        .collect()
}

impl CW721Swap {
    pub fn is_expired(&self, block: &BlockInfo) -> bool {
        self.expires.is_expired(block)
    }
}

pub const SWAPS: Map<&str, CW721Swap> = Map::new("cw721_swap");
pub const COMPLETED: Map<&str, bool> = Map::new("completed_swap");
pub const CANCELLED: Map<&str, bool> = Map::new("cancelled_swap");

