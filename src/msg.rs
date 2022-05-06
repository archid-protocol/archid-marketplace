use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Binary, Coin, Decimal,BlockInfo,Addr, Uint128};
use cw20::{Balance, Expiration};
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
   
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Create(CreateMsg),
    Finish(CreateMsg),
    Cancel(CancelMsg),

}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct CancelMsg{
    pub id:String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CreateMsg {
    pub id:String,
    pub contract: Addr,
    pub payment_token:Addr,
    pub token_id:String,    
    pub expires: Expiration,    
    pub price: Uint128,
    pub swap_type: bool,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Show all open swaps. Return type is ListResponse.
    /*List {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    */
    /// Returns the details of the named swap, error if not created.
    /// Return type: DetailsResponse.
    Details { id: String },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ListResponse {
}
// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DetailsResponse {
    pub creator: Addr,
    pub contract: Addr,
    pub payment_token:Addr,
    pub token_id:String,    
    pub expires: Expiration,    
    pub price: Uint128,
    pub swap_type:bool
}
