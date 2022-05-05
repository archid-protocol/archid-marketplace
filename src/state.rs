use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cw_storage_plus::{Bound, Map};
use cosmwasm_std::Addr;
use cw_storage_plus::Item;
use cw20::{Balance, Expiration};
use cosmwasm_std::{Binary, Coin, Decimal, Uint128};


#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct CW721Swap {    
    pub creator: Addr,
    pub contract: Addr,
    pub payment_token:Addr,
    pub token_id:String,    
    pub expires: Expiration,    
    pub price: Uint128
    pub swap_type SwapType,
}
pub enum SwapType{
    OFFER,
    FORSALE,
}

impl CW721Swap {
    pub fn is_expired(&self, block: &BlockInfo) -> bool {
        self.expires.is_expired(block)
    }
}

pub const SWAPS: Map<&str, CW721Swap> = Map::new("cw721_swap");
pub const COMPLETED: Map<&str, bool> = Map::new("completed_swap");
pub const CANCELLED: Map<&str, bool> = Map::new("cancelled_swap");

