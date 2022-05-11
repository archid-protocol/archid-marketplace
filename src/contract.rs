#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, CosmosMsg,Binary, Deps,BankMsg, DepsMut,Uint128, Env, MessageInfo, Response, StdResult,Addr,WasmMsg, SubMsg};
use cw2::set_contract_version;
use cw20::{Balance, Expiration,Cw20ExecuteMsg};
use cw721_base::{
    msg::ExecuteMsg as Cw721ExecuteMsg, msg::InstantiateMsg as Cw721InstantiateMsg, Extension,
    MintMsg,
};
use crate::error::ContractError;
use crate::msg::{
    CreateMsg, DetailsResponse, ExecuteMsg, InstantiateMsg,
     QueryMsg,CancelMsg
};
use crate::state::{ CW721Swap,SWAPS,CANCELLED,COMPLETED};
//pub type Extension = Option<Empty>;
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
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    
    Ok(Response::new())       
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Create(msg) => {execute_create(deps,_env,info,msg)},
        ExecuteMsg::Finish(msg) =>{execute_finish(deps,_env,info,msg)},
        ExecuteMsg::Cancel(msg) =>{execute_cancel(deps,_env,info,msg)}
       // ExecuteMsg::Cancel { id } => try_reset(deps, info, id),
    }
}
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        //QueryMsg::List { start_after, limit } => to_binary(&query_list(deps, start_after, limit)?),
        QueryMsg::Details { id } => to_binary(&query_details(deps, id)?)
    }
}
fn query_details(deps: Deps, id: String) -> StdResult<DetailsResponse> {
    let swap = SWAPS.load(deps.storage, &id)?;

    // Convert balance to human balance
    let mut available:bool=true;
    let _can = CANCELLED.may_load(deps.storage, &id)?;
    let _com=  COMPLETED.may_load(deps.storage,&id)?;

    available=!(_can!=None || _com!=None);
    
    
    let details = DetailsResponse{
        creator:swap.creator,
        contract:swap.nft_contract,
        payment_token:swap.payment_token,
        token_id:swap.token_id,    
        expires:swap.expires,    
        price:swap.price,
        swap_type:swap.swap_type,
        open:available
    };
    Ok(details)
}
pub fn execute_create(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CreateMsg,
    ) -> Result<Response, ContractError> {
        
        if msg.expires.is_expired(&env.block) {
            return Err(ContractError::Expired {});
        }
    
       // let recipient = deps.api.addr_validate(&msg.recipient)?;
        let swap = CW721Swap {
            creator:info.sender,
            nft_contract:msg.contract,
            payment_token:msg.payment_token,
            token_id:msg.token_id,    
            expires:msg.expires,    
            price:msg.price,
            swap_type:msg.swap_type, 
        };
    
        // Try to store it, fail if the id already exists (unmodifiable swaps)
        SWAPS.update(deps.storage, &msg.id, |existing| match existing {
            None => Ok(swap),
            Some(_) => Err(ContractError::AlreadyExists {}),
        })?;
    
        let res = Response::new();
            
        Ok(res)
}
pub fn execute_finish(deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CreateMsg)-> Result<Response, ContractError> {
        let swap = SWAPS.load(deps.storage, &msg.id)?;
        let _can = CANCELLED.load(deps.storage, &msg.id)?;
        let _com=  COMPLETED.load(deps.storage,&msg.id)?;
        if msg.expires.is_expired(&env.block) {
            return Err(ContractError::Expired {});
        }
        if _can==true{
            return Err(ContractError::Cancelled {});
        }
        if _com==true{
            return Err(ContractError::Completed {});
        }
        let transfer_results= match msg.swap_type{
            true => handle_swap_transfers(&swap.creator,&info.sender,swap.clone())?,
            false=> handle_swap_transfers(&info.sender,&swap.creator,swap.clone())?,
        };
        COMPLETED.update(deps.storage, &msg.id, |existing| match existing {
            None => Ok(true),
            Some(_) => Err(ContractError::AlreadyExists {}),
        })?;
        let res = Response::new();
            
        Ok(res)
    }

pub fn execute_cancel(deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CancelMsg)-> Result<Response, ContractError> {
        let res = Response::new();
        CANCELLED.update(deps.storage, &msg.id, |existing| match existing {
            None => Ok(true),
            Some(_) => Err(ContractError::AlreadyExists {}),
        })?;
        Ok(res)
    }


fn handle_swap_transfers(nft_sender:&Addr,nft_receiver: &Addr,details:CW721Swap) -> StdResult<Vec<SubMsg>> {
    
    let nft_transfer_msg = Cw721ExecuteMsg::<Extension>::TransferNft{
        recipient: nft_receiver.to_string(),
        token_id: details.token_id.clone(),
    };;
   
    let cw721_callback = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: details.nft_contract.to_string(),
        msg: to_binary(&nft_transfer_msg)?,
        funds: vec![],
    });
    
    let token_transfer_msg = Cw20ExecuteMsg::TransferFrom {
        owner: nft_sender.to_string(),
        recipient:nft_receiver.to_string(),
        amount: details.price
    };
    let cw20_callback = WasmMsg::Execute {
        contract_addr: details.payment_token.into(),
        msg: to_binary(&token_transfer_msg)?,
        funds: vec![],
    };
    Ok(vec![SubMsg::new(cw721_callback),SubMsg::new(cw20_callback)])
}
#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info,MOCK_CONTRACT_ADDR,
    };
    use cosmwasm_std::{coins, from_binary};
    #[test]
    fn test_instantiate() {
        let mut deps = mock_dependencies();

        // Instantiate an empty contract
        let instantiate_msg = InstantiateMsg {};
        let info = mock_info("anyone", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());
    }
    #[test]
    fn test_creation() {
        let mut deps = mock_dependencies();

        // Instantiate an empty contract
        let instantiate_msg = InstantiateMsg {};
        let info = mock_info("anyone", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());
        let creation_msg= CreateMsg{ 
            id:"firstswap".to_string(),
            contract: Addr::unchecked(MOCK_CONTRACT_ADDR),
            payment_token:Addr::unchecked(MOCK_CONTRACT_ADDR),
            token_id:"2343".to_string(),    
            expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),    
            price:Uint128::from(100000_u32),
            swap_type:true,
        };
        let  info2 = mock_info("someone", &[]);
        execute(deps.as_mut(), mock_env(), info2, ExecuteMsg::Create(creation_msg)).unwrap();
        let creation_msg2= CreateMsg{ 
            id:"2ndswap".to_string(),
            contract: Addr::unchecked(MOCK_CONTRACT_ADDR),
            payment_token:Addr::unchecked(MOCK_CONTRACT_ADDR),
            token_id:"2343".to_string(),    
            expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),    
            price:Uint128::from(100000_u32),
            swap_type:true,
        };
        let  info2 = mock_info("anyone", &[]);
        execute(deps.as_mut(), mock_env(), info2, ExecuteMsg::Create(creation_msg2)).unwrap();
       // let mut deps = mock_dependencies();
        let mut qres:DetailsResponse=from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Details{id:"2ndswap".to_string()}).unwrap()).unwrap();
        println!("{}",qres.creator);
        println!("{}",qres.contract);
        println!("{}",qres.open);
        assert_eq!(qres.open, true);
    }
}
