#![cfg(test)]

//use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{to_binary,coins, from_binary, Addr,DepsMut,QueryRequest,BankQuery, Coin, Empty, Uint128,StdError, WasmMsg, WasmQuery};
use cw20::{Cw20Coin,BalanceResponse, Expiration,Cw20Contract,Cw20QueryMsg, Cw20ExecuteMsg};
use cw_multi_test::{App, Contract, ContractWrapper,Executor};
use cw721_base::{
    msg::ExecuteMsg as Cw721ExecuteMsg, msg::InstantiateMsg as Cw721InstantiateMsg, Extension,
    MintMsg,Cw721Contract,msg::QueryMsg as Cw721QueryMsg
};
use cw721::{
    AllNftInfoResponse, ApprovalResponse, ApprovalsResponse, ContractInfoResponse, CustomMsg,
    Cw721Query,NftInfoResponse, NumTokensResponse, OperatorsResponse, OwnerOfResponse,
    TokensResponse,
};
use cosmwasm_std::testing::{
    mock_dependencies, mock_dependencies_with_balance, mock_env,MockStorage, mock_info,MOCK_CONTRACT_ADDR,
};

use crate::msg::{ExecuteMsg,DetailsResponse,QueryMsg,CreateMsg, ListResponse, InstantiateMsg};
use serde::{de::DeserializeOwned, Serialize};

fn mock_app() -> App {

    App::default()
}
pub fn contract_swap721() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

pub fn contract_cw721() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw721_base::entry::execute,
        cw721_base::entry::instantiate,
        cw721_base::entry::query,
    );
    Box::new(contract)
}

fn create_swap(router: &mut App, owner: &Addr ) -> Addr {
    
    let swap_id = router.store_code(contract_swap721());
    let msg = InstantiateMsg {      
    };
    let swap_addr = router
        .instantiate_contract(swap_id, owner.clone(), &msg, &[], "swap721",None)
        .unwrap();
    swap_addr
}

fn create_cw721(router: &mut App,minter: &Addr) -> Addr {
    //let contract = Cw721Contract::default();
    let cw721_id = router.store_code(contract_cw721());
    let msg = Cw721InstantiateMsg {
        name: "TESTNFT".to_string(),
        symbol: "TSNFT".to_string(),
        minter: String::from(minter),
    };   
    let contract=router
        .instantiate_contract(cw721_id, minter.clone(), &msg, &[], "swap721",None)
        .unwrap();    
    contract
}


fn create_cw20(
    router: &mut App,
    owner: &Addr,
    name: String,
    symbol: String,
    balance: Uint128,
) -> Addr {
    // set up cw20 contract with some tokens
    let cw20_id = router.store_code(contract_cw20());
    let msg = cw20_base::msg::InstantiateMsg {
        name: name,
        symbol: symbol,
        decimals: 2,
        initial_balances: vec![Cw20Coin{
            address: owner.to_string(),
            amount: balance,
        }],
        mint: None,
        marketing:None
    };
    let addr = router
        .instantiate_contract(cw20_id, owner.clone(), &msg, &[], "CASH",None)
        .unwrap();
    addr
}

pub fn query<M,T>(router: &mut App, target_contract: Addr, msg: M) -> Result<T, StdError>
    where
        M: Serialize + DeserializeOwned,
        T: Serialize + DeserializeOwned,
    {
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: target_contract.to_string(),
            msg: to_binary(&msg).unwrap(),
        }))
    }

// receive cw20 tokens and release upon approval
#[test]
fn test_buy() {
    let mut app=mock_app();
    
    let owner = Addr::unchecked("owner");
    let nft_owner=Addr::unchecked("nft_owner");
    let swap=create_swap(&mut app, &owner);
    let swap_inst=swap.clone();
    let nft= create_cw721(&mut app,&owner); 
    let erc20=create_cw20(
        &mut app,
        &owner,
        "testcw".to_string(),
        "tscw".to_string(),
        Uint128::from(100000_u32)
    );
    let erc20_inst=erc20.clone();
    let token_id = "petrify".to_string();
    let token_uri = "https://www.merriam-webster.com/dictionary/petrify".to_string();

    
    let mint_msg = Cw721ExecuteMsg::Mint(MintMsg::<Extension> {
        token_id: token_id.clone(),
        owner: String::from("nft_owner"),
        token_uri: Some(token_uri.clone()),
        extension: None,
    });
    let res = app
        .execute_contract(owner.clone(), nft.clone(), &mint_msg, &[])
        .unwrap();
   
    let nft_approve_msg = Cw721ExecuteMsg::Approve::<Extension> {
            spender: swap.to_string(),
            token_id: token_id.clone(),
            expires: None,
    };
    app
    .execute_contract(nft_owner.clone(), nft.clone(), &nft_approve_msg, &[])
    .unwrap();
    
    /**
    IncreaseAllowance {
        spender: String,
        amount: Uint128,
        expires: Option<Expiration>,
    },
    **/
    let erc20_approve_msg = Cw20ExecuteMsg::IncreaseAllowance  {
        spender: swap.to_string(),
        amount:  Uint128::from(100000_u32),
        expires: None,
    };
    app
    .execute_contract(owner.clone(), erc20.clone(), &erc20_approve_msg, &[])
    .unwrap();
    let creation_msg= CreateMsg{ 
        id:"firstswap".to_string(),
        contract: Addr::unchecked(nft.clone()),
        payment_token:Addr::unchecked(erc20),
        token_id:token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),    
        price:Uint128::from(100000_u32),
        swap_type:true,
    };
    let finish_msg=creation_msg.clone();
    app
    .execute_contract(nft_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
    .unwrap();
    app
    .execute_contract(owner.clone(), swap_inst.clone(), &ExecuteMsg::Finish(finish_msg), &[])
    .unwrap();
    let mut qres:DetailsResponse=query(&mut app,swap_inst.clone(),QueryMsg::Details{id:"firstswap".to_string()}).unwrap();
    println!("{}",qres.creator);
    println!("{}",qres.contract);
    println!("{}",qres.open);
    assert_eq!(qres.open, false);
    let mut new_owner:OwnerOfResponse=query(&mut app,nft.clone(),Cw721QueryMsg::OwnerOf{token_id:token_id, include_expired:None}).unwrap();
    println!("{}",new_owner.owner);
    let mut new_balance:BalanceResponse=query(&mut app,erc20_inst,Cw20QueryMsg::Balance{address:nft_owner.to_string()}).unwrap();
   println!("{:?}",new_balance);
}


