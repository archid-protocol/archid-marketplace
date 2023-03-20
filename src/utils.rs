use cosmwasm_std::{
    to_binary, Addr, Binary, Deps, DepsMut, Env, QueryRequest, StdError, StdResult, WasmQuery,
};
use cw721_base::{Extension,  QueryMsg as Cw721QueryMsg};
use cw721::OwnerOfResponse;

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