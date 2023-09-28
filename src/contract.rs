#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    entry_point, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut,Order, Env,
    MessageInfo, Reply, Response, StdResult, SubMsgResult, Uint128, WasmMsg,
};
use cw_storage_plus::Bound;

use crate::error::ContractError;
use crate::utils::query_name_owner;
use cw20::Cw20ExecuteMsg;
use cw721_base::{msg::ExecuteMsg as Cw721ExecuteMsg, Extension};

use crate::msg::{
    CancelMsg, DetailsResponse, ExecuteMsg, InstantiateMsg, ListResponse, MigrateMsg, QueryMsg,
    SwapMsg,
};
use crate::state::{all_swap_ids, CW721Swap, Config, CONFIG, SWAPS,SwapType};

use cw2::{get_contract_version, set_contract_version};

// Mainnet
// pub static DENOM: &str = "aarch";
// Testnet
pub static DENOM: &str = "aconst";

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:archid-marketplace";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        admin: msg.admin,
        cw721: msg.cw721.clone(),
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("cw721", msg.cw721))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Create(msg) => execute_create(deps, env, info, msg),
        ExecuteMsg::Finish(msg) => execute_finish(deps, env, info, msg),
        ExecuteMsg::Update(msg) => execute_update(deps, env, info, msg),
        ExecuteMsg::Cancel(msg) => execute_cancel(deps, env, info, msg),
        ExecuteMsg::UpdateConfig { config } => execute_update_config(deps, env, info, config),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::List { start_after, limit } => to_binary(&query_list(deps, start_after, limit)?),
        QueryMsg::Details { id } => to_binary(&query_details(deps, id)?),
        QueryMsg::GetOffers { page, limit } => {
            to_binary(&query_swaps(deps, SwapType::Offer, page, limit)?)
        },
        QueryMsg::GetListings { page, limit } => {
            to_binary(&query_swaps(deps, SwapType::Sale, page, limit)?)
        }
        QueryMsg::GetTotal { swap_type } => {
            to_binary(&query_swap_total(deps, swap_type)?)
        }
        QueryMsg::SwapsOf { address, swap_type, page, limit } => {
            to_binary(&query_swaps_by_creator(deps, address, swap_type, page, limit)?)
        }
        QueryMsg::SwapsByPrice { min, max, swap_type, page, limit } => {
            to_binary(&query_swaps_by_price(deps, min, max, swap_type, page, limit)?)
        }
        QueryMsg::SwapsByDenom { payment_token, swap_type, page, limit } => {
            to_binary(&query_swaps_by_denom(deps, payment_token, swap_type, page, limit)?)
        }
        QueryMsg::SwapsByPaymentType { cw20, swap_type, page, limit } => {
            to_binary(&query_swaps_by_payment_type(deps, cw20, swap_type, page, limit)?)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.result {
        SubMsgResult::Ok(_) => Ok(Response::default()),
        SubMsgResult::Err(_) => Err(ContractError::Unauthorized {}),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let original_version = get_contract_version(deps.storage)?;
    let name = CONTRACT_NAME.to_string();
    let version = CONTRACT_VERSION.to_string();
    if original_version.contract != name {
        return Err(ContractError::InvalidInput {});
    }
    if original_version.version >= version {
        return Err(ContractError::InvalidInput {});
    }
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

fn query_details(deps: Deps, id: String) -> StdResult<DetailsResponse> {
    let swap = SWAPS.load(deps.storage, &id)?;
    let details = DetailsResponse {
        creator: swap.creator,
        contract: swap.nft_contract,
        payment_token: swap.payment_token,
        token_id: swap.token_id,
        expires: swap.expires,
        price: swap.price,
        swap_type: swap.swap_type,
    };
    Ok(details)
}

// Default and Max page sizes for paginated queries
const MAX_LIMIT: u32 = 100;
const DEFAULT_LIMIT: u32 = 10;

fn query_list(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<ListResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_ref().map(|s| Bound::exclusive(s.as_str()));

    Ok(ListResponse {
        swaps: all_swap_ids(deps.storage, start, limit)?,
    })
}

fn query_swaps(
    deps: Deps,
    side: SwapType, 
    page: Option<u32>, 
    limit: Option<u32>,
) -> StdResult<Vec<CW721Swap>> {
    let page: u32 = page.unwrap_or(0_u32);
    let mut limit: u32 = limit.unwrap_or(DEFAULT_LIMIT);
    let config = CONFIG.load(deps.storage)?;
    let swaps: Result<Vec<(String, CW721Swap)>, cosmwasm_std::StdError> = SWAPS
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    let results: Vec<CW721Swap> = swaps
        .unwrap()
        .into_iter()
        .map(|t| t.1)
        .filter(|item| {
            item.nft_contract == config.cw721 
            && item.swap_type == side
        })
        .collect();
    
    let total_results = results.len() as u32;
    if total_results < limit {
        limit = total_results;
    } else if limit < DEFAULT_LIMIT {
        limit = DEFAULT_LIMIT;
    } else if limit > MAX_LIMIT {
        limit = MAX_LIMIT;
    }

    let start = (page*limit) as usize;
    let end = ((page+1)*limit) as usize;

    Ok(results[start..end].to_vec())
}

fn query_swap_total(deps: Deps, side: SwapType) -> StdResult<u128> {
    let config = CONFIG.load(deps.storage)?;
    let swaps: Result<Vec<(String, CW721Swap)>, cosmwasm_std::StdError> = SWAPS
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    let results: Vec<CW721Swap> = swaps
        .unwrap()
        .into_iter()
        .map(|t| t.1)
        .filter(|item| {
            item.nft_contract == config.cw721 && item.swap_type == side
        })
        .collect();
    
    Ok(results.len() as u128)
}

fn query_swaps_by_creator(
    deps: Deps, 
    address: Addr,
    swap_type: Option<SwapType>,
    page: Option<u32>,
    limit: Option<u32>,
) -> StdResult<Vec<CW721Swap>> {
    let side: SwapType = swap_type.unwrap_or(SwapType::Sale);
    let page: u32 = page.unwrap_or(0_u32);
    let mut limit: u32 = limit.unwrap_or(DEFAULT_LIMIT);
    let config = CONFIG.load(deps.storage)?;
    let swaps: Result<Vec<(String, CW721Swap)>, cosmwasm_std::StdError> = SWAPS
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    let results: Vec<CW721Swap> = swaps
        .unwrap()
        .into_iter()
        .map(|t| t.1)
        .filter(|item| {
            item.nft_contract == config.cw721 
            && item.creator == address
            && item.swap_type == side
        })
        .collect();

    let total_results = results.len() as u32;
    if total_results < limit {
        limit = total_results;
    } else if limit < DEFAULT_LIMIT {
        limit = DEFAULT_LIMIT;
    } else if limit > MAX_LIMIT {
        limit = MAX_LIMIT;
    }

    let start = (page*limit) as usize;
    let end = ((page+1)*limit) as usize;

    Ok(results[start..end].to_vec())
}

fn query_swaps_by_price(
    deps: Deps, 
    min: Option<Uint128>, 
    max: Option<Uint128>, 
    swap_type: Option<SwapType>,
    page: Option<u32>,
    limit: Option<u32>,
) -> StdResult<Vec<CW721Swap>> {
    let min: Uint128 = min.unwrap_or(Uint128::from(0_u32));
    let side: SwapType = swap_type.unwrap_or(SwapType::Sale);
    let page: u32 = page.unwrap_or(0_u32);
    let mut limit: u32 = limit.unwrap_or(DEFAULT_LIMIT);
    let config = CONFIG.load(deps.storage)?;
    let swaps: Result<Vec<(String, CW721Swap)>, cosmwasm_std::StdError> = SWAPS
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    // With Max range filter
    let results: Vec<CW721Swap> = if let Some(max_value) = max {
        swaps
            .unwrap()
            .into_iter()
            .map(|t| t.1)
            .filter(|item| {
                item.nft_contract == config.cw721 
                && item.price.u128() >= min.u128()
                && item.price.u128() <= max_value.u128()
                && item.swap_type == side
            })
            .collect()
    } else {
        swaps
            .unwrap()
            .into_iter()
            .map(|t| t.1)
            .filter(|item| {
                item.nft_contract == config.cw721 
                && item.price.u128() >= min.u128()
                && item.swap_type == side
            })
            .collect()
    };

    let total_results = results.len() as u32;
    if total_results < limit {
        limit = total_results;
    } else if limit < DEFAULT_LIMIT {
        limit = DEFAULT_LIMIT;
    } else if limit > MAX_LIMIT {
        limit = MAX_LIMIT;
    }

    let start = (page*limit) as usize;
    let end = ((page+1)*limit) as usize;

    Ok(results[start..end].to_vec())
}

fn query_swaps_by_denom(
    deps: Deps, 
    payment_token: Option<Addr>, 
    swap_type: Option<SwapType>,
    page: Option<u32>,
    limit: Option<u32>,
) -> StdResult<Vec<CW721Swap>> {
    let side: SwapType = swap_type.unwrap_or(SwapType::Sale);
    let page: u32 = page.unwrap_or(0_u32);
    let mut limit: u32 = limit.unwrap_or(DEFAULT_LIMIT);
    let config = CONFIG.load(deps.storage)?;
    let swaps: Result<Vec<(String, CW721Swap)>, cosmwasm_std::StdError> = SWAPS
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    // Requested cw20 denom
    let results: Vec<CW721Swap> = if let Some(token_addr) = payment_token {
        swaps
            .unwrap()
            .into_iter()
            .map(|t| t.1)
            .filter(|item| {
                item.nft_contract == config.cw721 
                && item.payment_token.clone().unwrap() == token_addr
                && item.swap_type == side
            })
            .collect()
    // Native ARCH denom
    } else {
        swaps
            .unwrap()
            .into_iter()
            .map(|t| t.1)
            .filter(|item| {
                item.nft_contract == config.cw721 
                && item.payment_token.is_none()
                && item.swap_type == side
            })
            .collect()
    };

    let total_results = results.len() as u32;
    if total_results < limit {
        limit = total_results;
    } else if limit < DEFAULT_LIMIT {
        limit = DEFAULT_LIMIT;
    } else if limit > MAX_LIMIT {
        limit = MAX_LIMIT;
    }

    let start = (page*limit) as usize;
    let end = ((page+1)*limit) as usize;

    Ok(results[start..end].to_vec())
}

fn query_swaps_by_payment_type(
    deps: Deps, 
    cw20: bool,
    swap_type: Option<SwapType>,
    page: Option<u32>,
    limit: Option<u32>,
) -> StdResult<Vec<CW721Swap>> {
    let side: SwapType = swap_type.unwrap_or(SwapType::Sale);
    let page: u32 = page.unwrap_or(0_u32);
    let mut limit: u32 = limit.unwrap_or(DEFAULT_LIMIT);
    let config = CONFIG.load(deps.storage)?;
    let swaps: Result<Vec<(String, CW721Swap)>, cosmwasm_std::StdError> = SWAPS
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    // cw20 swap
    let results: Vec<CW721Swap> = if cw20 {
        swaps
            .unwrap()
            .into_iter()
            .map(|t| t.1)
            .filter(|item| {
                item.nft_contract == config.cw721 
                && item.payment_token.is_some()
                && item.swap_type == side
            })
            .collect()
    // ARCH swap
    } else {
        swaps
            .unwrap()
            .into_iter()
            .map(|t| t.1)
            .filter(|item| {
                item.nft_contract == config.cw721 
                && item.payment_token.is_none()
                && item.swap_type == side
            })
            .collect()
    };

    let total_results = results.len() as u32;
    if total_results < limit {
        limit = total_results;
    } else if limit < DEFAULT_LIMIT {
        limit = DEFAULT_LIMIT;
    } else if limit > MAX_LIMIT {
        limit = MAX_LIMIT;
    }

    let start = (page*limit) as usize;
    let end = ((page+1)*limit) as usize;

    Ok(results[start..end].to_vec())
}

pub fn execute_create(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: SwapMsg,
) -> Result<Response, ContractError> {
    if msg.expires.is_expired(&env.block) {
        return Err(ContractError::Expired {});
    }

    let config = CONFIG.load(deps.storage)?;
    if msg.swap_type==SwapType::Sale {
        let owner = query_name_owner(&msg.token_id, &config.cw721, &deps).unwrap();
        if owner.owner != info.sender {
            return Err(ContractError::Unauthorized);
        }
    }
    let swap = CW721Swap {
        creator: info.sender,
        nft_contract: config.cw721,
        payment_token: msg.payment_token,
        token_id: msg.token_id,
        expires: msg.expires,
        price: msg.price,
        swap_type: msg.swap_type,
    };

    // Try to store it, fail if the id already exists (unmodifiable swaps)
    SWAPS.update(deps.storage, &msg.id, |existing| match existing {
        None => Ok(swap.clone()),
        Some(_) => Err(ContractError::AlreadyExists {}),
    })?;

    let payment_token: String = if swap.payment_token.is_some() {
        swap.payment_token.unwrap().to_string()
    } else {
        DENOM.to_string()
    };

    Ok(Response::new()
        .add_attribute("action", "create")
        .add_attribute("token_id", swap.token_id)
        .add_attribute("payment_token", payment_token)
        .add_attribute("price", swap.price))
}

pub fn execute_update(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: SwapMsg,
) -> Result<Response, ContractError> {
    
    let swap = SWAPS.load(deps.storage, &msg.id)?;
    if info.sender != swap.creator {
        return Err(ContractError::Unauthorized {});
    }
    let swap = CW721Swap {
        creator: info.sender,
        nft_contract: swap.nft_contract,
        payment_token: msg.payment_token,
        token_id: msg.token_id,
        expires: msg.expires,
        price: msg.price,
        swap_type: msg.swap_type,
    };
    SWAPS.remove(deps.storage, &msg.id);
    SWAPS.save(deps.storage, &msg.id, &swap)?;
    Ok(Response::new()
    .add_attribute("action", "update")
    .add_attribute("swap_id", &msg.id)
    .add_attribute("token_id", &swap.token_id))

}

pub fn execute_finish(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: SwapMsg,
) -> Result<Response, ContractError> {
    let swap = SWAPS.load(deps.storage, &msg.id)?;

    if swap.expires.is_expired(&env.block) {
        return Err(ContractError::Expired {});
    }

    // If swapping for native `aarch`
    // check payment conditions satisfied
    if swap.payment_token.is_none() {
        let required_payment = Coin {
            denom: DENOM.to_string(),
            amount: swap.price,
        };
        check_sent_required_payment(&info.funds, Some(required_payment))?;

        // Native aarch offers not allowed
        if swap.swap_type==SwapType::Offer {
            return Err(ContractError::InvalidInput {});
        }
    }

  
    let transfer_results = match msg.swap_type {
        SwapType::Offer => handle_swap_transfers(&info.sender, &swap.creator, swap.clone(), &info.funds)?,
        SwapType::Sale => handle_swap_transfers(&swap.creator, &info.sender, swap.clone(), &info.funds)?,
        
    };

    SWAPS.remove(deps.storage, &msg.id);
    let payment_token: String = if msg.payment_token.is_some() {
        msg.payment_token.unwrap().to_string()
    } else {
        DENOM.to_string()
    };

    Ok(Response::new()
        .add_attribute("action", "finish")
        .add_attribute("token_id", msg.token_id)
        .add_attribute("payment_token", payment_token)
        .add_attribute("price", msg.price)
        .add_messages(transfer_results))
}

pub fn execute_cancel(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: CancelMsg,
) -> Result<Response, ContractError> {
    let swap = SWAPS.load(deps.storage, &msg.id)?;
    if info.sender != swap.creator {
        return Err(ContractError::Unauthorized {});
    }
    SWAPS.remove(deps.storage, &msg.id);

    Ok(Response::new()
        .add_attribute("action", "cancel")
        .add_attribute("swap_id", msg.id))
}

pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    config_update: Config,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if config.admin != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    CONFIG.save(deps.storage, &config_update)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

fn handle_swap_transfers(
    nft_sender: &Addr,
    nft_receiver: &Addr,
    details: CW721Swap,
    funds: &[Coin],
) -> StdResult<Vec<CosmosMsg>> {
    // cw20 swap
    let payment_callback: CosmosMsg = if details.payment_token.is_some() {
        let token_transfer_msg = Cw20ExecuteMsg::TransferFrom {
            owner: nft_receiver.to_string(),
            recipient: nft_sender.to_string(),
            amount: details.price,
        };

        let cw20_callback: CosmosMsg = WasmMsg::Execute {
            contract_addr: details.payment_token.unwrap().into(),
            msg: to_binary(&token_transfer_msg)?,
            funds: vec![],
        }
        .into();
        cw20_callback
    // aarch swap
    } else {
        let aarch_transfer_msg = BankMsg::Send {
            to_address: nft_sender.to_string(),
            amount: funds.to_vec(),
        };

        let aarch_callback: CosmosMsg = cosmwasm_std::CosmosMsg::Bank(aarch_transfer_msg);
        aarch_callback
    };

    let nft_transfer_msg = Cw721ExecuteMsg::<Extension>::TransferNft {
        recipient: nft_receiver.to_string(),
        token_id: details.token_id.clone(),
    };

    let cw721_callback: CosmosMsg = WasmMsg::Execute {
        contract_addr: details.nft_contract.to_string(),
        msg: to_binary(&nft_transfer_msg)?,
        funds: vec![],
    }
    .into();

    Ok(vec![cw721_callback, payment_callback])
}

pub fn check_sent_required_payment(
    sent: &[Coin],
    required: Option<Coin>,
) -> Result<(), ContractError> {
    if let Some(required_coin) = required {
        let required_amount = required_coin.amount.u128();
        if required_amount > 0 {
            let sent_sufficient_funds = sent.iter().any(|coin| {
                // check if a given sent coin matches denom
                // and has sufficient amount
                coin.denom == required_coin.denom && coin.amount.u128() >= required_amount
            });

            if sent_sufficient_funds {
                return Ok(());
            } else {
                return Err(ContractError::Unauthorized {});
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_env, mock_info, MOCK_CONTRACT_ADDR
    };

    #[test]
    fn test_instantiate() {
        let mut deps = mock_dependencies();

        // Instantiate an empty contract
        let instantiate_msg = InstantiateMsg {
            admin: Addr::unchecked(MOCK_CONTRACT_ADDR),
            cw721: Addr::unchecked(MOCK_CONTRACT_ADDR),
        };
        let info = mock_info("anyone", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());
    }
}