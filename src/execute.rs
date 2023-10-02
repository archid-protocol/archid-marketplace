use cosmwasm_std::{BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response};

use crate::state::{CW721Swap, Config, CONFIG, SWAPS, SwapType};
use crate::utils::{
    check_sent_required_payment, check_contract_balance_ok, query_name_owner, handle_swap_transfers,
};
use crate::msg::{CancelMsg, SwapMsg};
use crate::contract::DENOM;
use crate::error::ContractError;

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
    env: Env,
    info: MessageInfo,
    msg: CancelMsg,
) -> Result<Response, ContractError> {
    let swap = SWAPS.load(deps.storage, &msg.id)?;
    if info.sender != swap.creator {
        return Err(ContractError::Unauthorized {});
    }
    
    // Return escrowed funds if SwapType::Offer
    // and payment_token is ARCH (e.g. not cw20)
    let escrow = if swap.swap_type == SwapType::Offer && swap.payment_token.is_none() {
        // Check contract has adequate balance 
        // (funded at Swap creation)
        let escrowed_payment = Coin {
            denom: DENOM.to_string(),
            amount: swap.price,
        };
        check_contract_balance_ok(env, &deps, escrowed_payment.clone())?;

        let release_escrow_msg = BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: ([escrowed_payment]).to_vec(),
        };

        let released_escrow: CosmosMsg = cosmwasm_std::CosmosMsg::Bank(release_escrow_msg);
        Some(released_escrow)
    } else {
        None
    };

    SWAPS.remove(deps.storage, &msg.id);

    Ok(Response::new()
        .add_attribute("action", "cancel")
        .add_attribute("swap_id", msg.id)
        .add_messages(escrow)
    )
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