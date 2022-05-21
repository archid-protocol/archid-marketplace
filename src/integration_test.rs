#![cfg(test)]

//use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{coins, from_binary, Addr,DepsMut, BalanceResponse, BankQuery, Coin, Empty, Uint128};
use cw20::{Cw20Coin, Cw20Contract, Cw20ExecuteMsg};
use cw_multi_test::{App, Contract, ContractWrapper,Executor};
use cw721_base::{
    msg::ExecuteMsg as Cw721ExecuteMsg, msg::InstantiateMsg as Cw721InstantiateMsg, Extension,
    MintMsg,Cw721Contract
};
use cosmwasm_std::testing::{
    mock_dependencies, mock_dependencies_with_balance, mock_env,MockStorage, mock_info,MOCK_CONTRACT_ADDR,
};

use crate::msg::{ExecuteMsg, ListResponse, InstantiateMsg, QueryMsg};

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



// receive cw20 tokens and release upon approval
#[test]
fn test_instantiate() {
    let mut app=mock_app();
    
    let owner = Addr::unchecked("owner");
    let nft_owner=Addr::unchecked("nft_owner");
    let swap=create_swap(&mut app, &owner);
    let nft= create_cw721(&mut app,&owner); 
    let erc20=create_cw20(
        &mut app,
        &owner,
        "testcw".to_string(),
        "tscw".to_string(),
        Uint128::from(100000_u32)
    );
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

    let approve_msg = Cw721ExecuteMsg::Approve::<Extension> {
            spender: swap.to_string(),
            token_id: token_id.clone(),
            expires: None,
    };
    app
    .execute_contract(nft_owner.clone(), nft.clone(), &approve_msg, &[])
    .unwrap();

}


