use cosmwasm_std::{
    Addr, BalanceResponse, BankQuery, Coin, DepsMut, Env, from_binary, QueryRequest, 
    to_binary, StdError, WasmQuery,
};
use crate::error::ContractError;
use cw721_base::{QueryMsg as Cw721QueryMsg};
use cw721::OwnerOfResponse;
use crate::contract::DENOM;

pub fn query_name_owner(
    id: &str,
    cw721: &Addr,
    deps: &DepsMut,
) -> Result<OwnerOfResponse, StdError> {
    let query_msg = Cw721QueryMsg::OwnerOf {
        token_id: id.to_owned(),
        include_expired: None,
    };
    let req = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: cw721.to_string(),
        msg: to_binary(&query_msg).unwrap(),
    });
    let res: OwnerOfResponse = deps.querier.query(&req)?;
    Ok(res)
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

pub fn check_contract_balance_ok(
    env: Env,
    deps: &DepsMut,
    required: Coin,
) -> Result<(), ContractError> {
    if required.denom != DENOM.to_string() {
        return Err(ContractError::InsufficientBalance {});
    }
    let swap_instance: &Addr = &env.contract.address;
    let required_amount = required.amount.u128();

    // Balance query
    let req: QueryRequest<BankQuery> = QueryRequest::Bank(BankQuery::Balance { 
        address: swap_instance.to_string(),
        denom: DENOM.to_string(),
    });
    let res = deps.querier.raw_query(&to_binary(&req).unwrap()).unwrap().unwrap();
    let query: BalanceResponse = from_binary(&res).unwrap();
    let balance: Coin = query.amount;
    if balance.amount.u128() < required_amount {
        return Err(ContractError::InsufficientBalance {});
    }

    Ok(())
}