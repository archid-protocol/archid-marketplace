#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;
use cw20::{Balance, Expiration};
use crate::error::ContractError;
use crate::msg::{
    is_valid_name, BalanceHuman, CreateMsg, DetailsResponse, ExecuteMsg, InstantiateMsg,
    ListResponse, QueryMsg, ReceiveMsg,
};
use crate::state::{State, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:test";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        count: msg.count,
        owner: info.sender.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("count", msg.count.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreateMsg  {} => createListing(deps),
       // ExecuteMsg::Cancel { id } => try_reset(deps, info, id),
    }
}
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        //QueryMsg::List { start_after, limit } => to_binary(&query_list(deps, start_after, limit)?),
        QueryMsg::Details { id } => to_binary(&query_details(deps, id)?),
    }
}
fn query_details(deps: Deps, id: String) -> StdResult<DetailsResponse> {
    let swap = SWAPS.load(deps.storage, &id)?;

    // Convert balance to human balance
    
    let details = DetailsResponse {
        swap.contract,
        swap.payment_token,
        swap.token_id,    
        swap.expires,    
        swap.price,
        swap.swap_type 
    };
    Ok(details)
}
pub fn execute_create(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CreateMsg,
    ) -> Result<Response, ContractError> {
        if !is_valid_name(&msg.id) {
            return Err(ContractError::InvalidId {});
        }
    
          
        // Ensure this is 32 bytes hex-encoded, and decode
        let hash = parse_hex_32(&msg.hash)?;
    
        if msg.expires.is_expired(&env.block) {
            return Err(ContractError::Expired {});
        }
    
        let recipient = deps.api.addr_validate(&msg.recipient)?;
        let swap = CW721Swap {
            msg.contract,
            msg.payment_token,
            msg.token_id,    
            msg.expires,    
            msg.price,
            msg.swap_type 
        };
    
        // Try to store it, fail if the id already exists (unmodifiable swaps)
        SWAPS.update(deps.storage, &msg.id, |existing| match existing {
            None => Ok(swap),
            Some(_) => Err(ContractError::AlreadyExists {}),
        })?;
    
        let res = Response::new()
            .add_attribute("action", "create")
            .add_attribute("id", msg.id)
            .add_attribute("hash", msg.hash)
            .add_attribute("recipient", msg.recipient);
        Ok(res)
}




fn send_tokens(to: &Addr, amount: Balance) -> StdResult<Vec<SubMsg>> {
    if amount.is_empty() {
        Ok(vec![])
    } else {
        match amount {
            Balance::Native(coins) => {
                let msg = BankMsg::Send {
                    to_address: to.into(),
                    amount: coins.into_vec(),
                };
                Ok(vec![SubMsg::new(msg)])
            }
            Balance::Cw20(coin) => {
                let msg = Cw20ExecuteMsg::Transfer {
                    recipient: to.into(),
                    amount: coin.amount,
                };
                let exec = WasmMsg::Execute {
                    contract_addr: coin.address.into(),
                    msg: to_binary(&msg)?,
                    funds: vec![],
                };
                Ok(vec![SubMsg::new(exec)])
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info,
    };
    use cosmwasm_std::{coins, from_binary};

    
}
