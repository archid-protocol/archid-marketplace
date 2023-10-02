#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    Addr, to_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, entry_point, Env, MessageInfo, 
    Reply, Response, StdResult, SubMsgResult, WasmMsg,
};

use crate::error::ContractError;
use crate::utils::{check_sent_required_payment, check_contract_balance_ok, query_name_owner,};
use cw20::Cw20ExecuteMsg;
use cw721_base::{msg::ExecuteMsg as Cw721ExecuteMsg, Extension};

use crate::query::{
    query_details, query_list, query_swap_total, query_swaps, query_swaps_by_creator, query_swaps_by_denom,
    query_swaps_by_payment_type, query_swaps_by_price, query_swaps_of_token,
};
use crate::msg::{
    CancelMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, SwapMsg,
};
use crate::state::{CW721Swap, Config, CONFIG, SWAPS, SwapType};

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
        QueryMsg::GetTotal { swap_type } => to_binary(&query_swap_total(deps, swap_type)?),
        QueryMsg::GetOffers { page, limit } => {
            to_binary(&query_swaps(deps, SwapType::Offer, page, limit)?)
        },
        QueryMsg::GetListings { page, limit } => {
            to_binary(&query_swaps(deps, SwapType::Sale, page, limit)?)
        }
        QueryMsg::ListingsOfToken { token_id, swap_type, page, limit } => {
            to_binary(&query_swaps_of_token(deps, token_id, swap_type, page, limit)?)
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
    let has_payment_token = msg.payment_token.is_some();
    // SwapType::Sale
    if msg.swap_type == SwapType::Sale {
        let owner = query_name_owner(&msg.token_id, &config.cw721, &deps).unwrap();
        if owner.owner != info.sender {
            return Err(ContractError::Unauthorized);
        }
    // SwapType::Offer
    } else if msg.swap_type == SwapType::Offer && !has_payment_token {
        let required_payment = Coin {
            denom: DENOM.to_string(),
            amount: msg.price,
        };
        check_sent_required_payment(&info.funds, Some(required_payment))?;
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

    let payment_token: String = if has_payment_token {
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
        // Native aarch offer
        if swap.swap_type == SwapType::Offer {
            // Check contract has adequate balance 
            // (funded at Swap creation)
            check_contract_balance_ok(env, &deps, required_payment)?;
        // Native aarch sale
        } else {
            // Check buyer sent correct payment
            check_sent_required_payment(&info.funds, Some(required_payment))?;
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
        let payment_funds = if details.swap_type == SwapType::Sale { funds.to_vec() } else { 
            ([Coin {
                denom: String::from(DENOM),
                amount: details.price,
            }])
            .to_vec()
        };
        let aarch_transfer_msg = BankMsg::Send {
            to_address: nft_sender.to_string(),
            amount: payment_funds,
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