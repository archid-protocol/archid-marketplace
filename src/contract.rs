#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    Addr, Binary, CosmosMsg, Deps, DepsMut, entry_point, Env, MessageInfo, 
    Reply, Response, StdResult, SubMsgResult, to_binary, WasmMsg,
};
use cw_storage_plus::Bound;

use crate::error::ContractError;

use cw20::Cw20ExecuteMsg;
use cw721_base::{ 
    msg::ExecuteMsg as Cw721ExecuteMsg, Extension, 
};

use crate::msg::{
    CancelMsg, SwapMsg, DetailsResponse, ExecuteMsg, InstantiateMsg, 
    QueryMsg, ListResponse, MigrateMsg,
};
use crate::state::{ 
    all_swap_ids, CANCELLED, COMPLETED, Config, CONFIG, CW721Swap, SWAPS,
};

use cw2::{get_contract_version, set_contract_version};

pub static DENOM: &str = "aarch";

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
        ExecuteMsg::Create(msg) => { execute_create(deps, env, info, msg) },
        ExecuteMsg::Finish(msg) => { execute_finish(deps, env, info, msg) },
        ExecuteMsg::Cancel(msg) => { execute_cancel(deps, env, info, msg) },
        ExecuteMsg::UpdateConfig { config } => execute_update_config(deps, env, info, config),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::List { start_after, limit } => to_binary(&query_list(deps, start_after, limit)?),
        QueryMsg::Details { id } => to_binary(&query_details(deps, id)?)
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

    // Convert balance to human balance
    let can = CANCELLED.may_load(deps.storage, &id)?;
    let com =  COMPLETED.may_load(deps.storage,&id)?;

    let available: bool =! (can.is_some() || com.is_some());

    let details = DetailsResponse {
        creator: swap.creator,
        contract: swap.nft_contract,
        payment_token: swap.payment_token,
        token_id: swap.token_id,    
        expires: swap.expires,    
        price: swap.price,
        swap_type: swap.swap_type,
        open: available,
    };
    Ok(details)
}
const MAX_LIMIT: u32 = 30;
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

// pub fn execute_create_default(//here
//     deps: DepsMut,
//     env: Env,
//     info: MessageInfo,
//     msg: SwapMsgDefault,
// ) -> Result<Response, ContractError> {
//     if msg.expires.is_expired(&env.block) {
//         return Err(ContractError::Expired {});
//     }

//     let config = CONFIG.load(deps.storage)?;

//     let swap = CW721SwapNative {
//         creator: info.sender,
//         nft_contract: config.cw721,
//         token_id: msg.token_id,    
//         expires: msg.expires,    
//         price: msg.price,
//         swap_type: msg.swap_type,
//     };
// }

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

pub fn execute_finish(deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: SwapMsg
)-> Result<Response, ContractError> {
    let swap = SWAPS.load(deps.storage, &msg.id)?;
    let can = CANCELLED.may_load(deps.storage, &msg.id)?;
    let com = COMPLETED.may_load(deps.storage, &msg.id)?;

    if msg.expires.is_expired(&env.block) {
        return Err(ContractError::Expired {});
    }
    if can.is_some() {
        return Err(ContractError::Cancelled {});
    }
    if com.is_some() {
        return Err(ContractError::Completed {});
    }

    // XXX: @jjj This part is pretty confusing
    // swap type true equals offer, swap type false equals buy
    let transfer_results = match msg.swap_type {
        true => handle_swap_transfers(&swap.creator, &info.sender, swap.clone())?,
        false => handle_swap_transfers(&info.sender, &swap.creator, swap.clone())?,
    };

    COMPLETED.update(deps.storage, &msg.id, |existing| match existing {
        None => Ok(true),
        Some(_) => Err(ContractError::AlreadyExists {}),
    })?;

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

pub fn execute_cancel(deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: CancelMsg
)-> Result<Response, ContractError> {
    let swap = SWAPS.load(deps.storage, &msg.id)?;
    if info.sender != swap.creator{
        return Err(ContractError::Unauthorized {});
    }
    CANCELLED.update(deps.storage, &msg.id, |existing| match existing {
        None => Ok(true),
        Some(_) => Err(ContractError::AlreadyExists {}),
    })?;

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

    Ok(Response::new()
        .add_attribute("action", "update_config"))
}

fn handle_swap_transfers(
    nft_sender: &Addr, 
    nft_receiver: &Addr,
    details: CW721Swap
) -> StdResult<Vec<CosmosMsg>> {
    let token_transfer_msg = Cw20ExecuteMsg::TransferFrom {
        owner: nft_receiver.to_string(),
        recipient: nft_sender.to_string(),
        amount: details.price
    };
    let cw20_callback:CosmosMsg = WasmMsg::Execute {
        contract_addr: details.payment_token.unwrap().into(),
        msg: to_binary(&token_transfer_msg)?,
        funds: vec![],
    }.into();
    let nft_transfer_msg = Cw721ExecuteMsg::<Extension>::TransferNft{
        recipient: nft_receiver.to_string(),
        token_id: details.token_id.clone(),
    };
   
    let cw721_callback:CosmosMsg = WasmMsg::Execute {
        contract_addr: details.nft_contract.to_string(),
        msg: to_binary(&nft_transfer_msg)?,
        funds: vec![],
    }.into();
    
    Ok(vec![cw721_callback,cw20_callback])
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_env, mock_info, MOCK_CONTRACT_ADDR,
    };
    use cosmwasm_std::{
        from_binary, Uint128,
    };
    use cw20::Expiration;
   
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
    #[test]
    fn test_creation() {
        let mut deps = mock_dependencies();

        // Instantiate an empty contract
        let instantiate_msg = InstantiateMsg {
            admin: Addr::unchecked(MOCK_CONTRACT_ADDR),
            cw721: Addr::unchecked(MOCK_CONTRACT_ADDR),
        };
        let info = mock_info("anyone", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        let creation_msg = SwapMsg {
            id: "firstswap".to_string(),
            payment_token: Some(Addr::unchecked(MOCK_CONTRACT_ADDR)),
            token_id: "2343".to_string(),    
            expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),    
            price: Uint128::from(100000_u32),
            swap_type: true,
        };
        
        let  info2 = mock_info("someone", &[]);

        execute(deps.as_mut(), mock_env(), info2, ExecuteMsg::Create(creation_msg)).unwrap();

        let creation_msg2 = SwapMsg {
            id: "2ndswap".to_string(),
            payment_token: Some(Addr::unchecked(MOCK_CONTRACT_ADDR)),
            token_id: "2343".to_string(),    
            expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),    
            price: Uint128::from(100000_u32),
            swap_type: true,
        };

        let  info2 = mock_info("anyone", &[]);

        execute(deps.as_mut(), mock_env(), info2, ExecuteMsg::Create(creation_msg2)).unwrap();
        
        let qres: DetailsResponse = from_binary(
            &query(
                deps.as_ref(), 
                mock_env(), 
                QueryMsg::Details { id:"2ndswap".to_string() }
            ).unwrap()
        ).unwrap();

        assert_eq!(qres.open, true);
    }
}