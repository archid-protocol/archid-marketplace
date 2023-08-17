use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr, Uint128};
use cw20::Expiration;
use crate::state::{Config,SwapType};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub admin: Addr,
    pub cw721: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Create(SwapMsg),
    Finish(SwapMsg),
    Cancel(CancelMsg),
    Update(SwapMsg),
    UpdateConfig { config: Config, },
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct CancelMsg {
    pub id: String,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SwapMsg {
    pub id: String,
    pub payment_token: Option<Addr>, // Optional cw20 address; if `None` create swap for `aarch`
    pub token_id: String,
    pub expires: Expiration,
    pub price: Uint128,
    pub swap_type: SwapType,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Get all swaps (enumerable)
    /// Return type: ListResponse
    List {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    // Count total `SwapType::Offer` or `SwapType::Sale`
    GetTotal {
        token_id: String, 
        swap_type: SwapType,
    },
    /// Get all swaps of type `SwapType::Offer`
    GetOffers {
        token_id: String,      
        page: u32,
    },
    /// Get all swaps of type `SwapType::Sale`
    GetListings {
        token_id: String, 
        page: u32,
    },
    /// Show all swaps created by a specific address
    /// Results include both `SwapType::Offer` and `SwapType::Sale`
    SwapsOf { address: Addr },

    /// Returns the details of the named swap, error if not created.
    /// Return type: DetailsResponse.
    Details { id: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}

// List swaps
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ListResponse {
    pub swaps: Vec<String>,
}

// Get details about a swap
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DetailsResponse {
    pub creator: Addr,
    pub contract: Addr,
    pub payment_token: Option<Addr>,
    pub token_id: String,    
    pub expires: Expiration,    
    pub price: Uint128,
    pub swap_type: SwapType
    
}