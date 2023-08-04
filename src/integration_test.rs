#![cfg(test)]
use serde::{de::DeserializeOwned, Serialize};
use cosmwasm_std::{
    to_binary, Addr, QueryRequest, Empty, Uint128, StdError, WasmQuery,
};
use cw_multi_test::{App, Contract, ContractWrapper,Executor};

use cw20::{
    Cw20Coin, BalanceResponse, Expiration, Cw20QueryMsg, Cw20ExecuteMsg
};
use cw721_base::{
    msg::ExecuteMsg as Cw721ExecuteMsg, msg::InstantiateMsg as Cw721InstantiateMsg, Extension,
    MintMsg, msg::QueryMsg as Cw721QueryMsg
};
use cw721::OwnerOfResponse;

use crate::msg::{
    ExecuteMsg, DetailsResponse, QueryMsg, SwapMsg, InstantiateMsg,
};

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

fn create_swap(router: &mut App, owner: &Addr, cw721: Addr) -> Addr {
    
    let swap_id = router.store_code(contract_swap721());
    let msg = InstantiateMsg {
        admin: owner.clone(),
        cw721: cw721,  
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

// Receive cw20 tokens and release upon approval
#[test]
fn test_buy_cw20() {
    let mut app = mock_app();
    
    // Swap owner deploys
    let swap_admin = Addr::unchecked("swap_deployer");
    // cw721_owner owns the cw721
    let cw721_owner = Addr::unchecked("original_owner");
    // cw20_owner owns the cw20
    let cw20_owner = Addr::unchecked("cw20_owner");

    // cw721_owner creates the cw721
    let nft = create_cw721(&mut app, &cw721_owner);
    
    // swap_admin creates the swap contract 
    let swap = create_swap(&mut app, &swap_admin, nft.clone());
    let swap_inst = swap.clone();
    
    // cw20_owner creates a cw20 coin
    let cw20 = create_cw20(
        &mut app,
        &cw20_owner,
        "testcw".to_string(),
        "tscw".to_string(),
        Uint128::from(100000_u32)
    );
    let cw20_inst = cw20.clone();

    // cw721_owner mints a cw721 
    let token_id = "petrify".to_string();
    let token_uri = "https://www.merriam-webster.com/dictionary/petrify".to_string();
    let mint_msg = Cw721ExecuteMsg::Mint(MintMsg::<Extension> {
        token_id: token_id.clone(),
        owner: cw721_owner.to_string(),
        token_uri: Some(token_uri.clone()),
        extension: None,
    });
    let _res = app
        .execute_contract(cw721_owner.clone(), nft.clone(), &mint_msg, &[])
        .unwrap();

    // Create a SwapMsg for creating / finishing a swap
    let creation_msg = SwapMsg {
        id: "firstswap".to_string(),
        payment_token: Some(Addr::unchecked(cw20.clone())),
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),  
        price: Uint128::from(100000_u32),
        swap_type: true,
    };
    let finish_msg = creation_msg.clone();

    // Seller (cw721_owner) must approve the swap contract to spend their NFT
    let nft_approve_msg = Cw721ExecuteMsg::Approve::<Extension> {
        spender: swap.to_string(),
        token_id: token_id.clone(),
        expires: None,
    };
    app
        .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
        .unwrap();

    // cw721 seller (cw721_owner) creates a swap
    app
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();

    // cw721 buyer (cw20_owner) must approve swap contract to spend their cw20
    let cw20_approve_msg = Cw20ExecuteMsg::IncreaseAllowance  {
        spender: swap.to_string(),
        amount:  Uint128::from(100000_u32),
        expires: None,
    };
    app
        .execute_contract(cw20_owner.clone(), cw20, &cw20_approve_msg, &[])
        .unwrap();

    // Buyer purchases cw721, consuming the swap
    app
        .execute_contract(cw20_owner.clone(), swap_inst.clone(), &ExecuteMsg::Finish(finish_msg), &[])
        .unwrap();

    // Swap is now closed (open == false)
    let swap_query: DetailsResponse = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::Details{
            id: "firstswap".to_string()
        }
    ).unwrap();

    // cw20_owner has received the NFT
    let owner_query: OwnerOfResponse = query(
        &mut app,nft.clone(),
        Cw721QueryMsg::OwnerOf {
            token_id: token_id, 
            include_expired: None
        }
    ).unwrap();

    // cw721_owner has received the cw20 amount
    let balance_query: BalanceResponse = query(
        &mut app,
        cw20_inst,
        Cw20QueryMsg::Balance {
            address: cw721_owner.to_string()
        }
    ).unwrap();
   
    assert_eq!(swap_query.open, false);
    assert_eq!(owner_query.owner, cw20_owner);
    assert_eq!(balance_query.balance, Uint128::from(100000_u32));
}

// cw721 buyer must send correct cw20 amount
#[test]
fn test_invalid_payment_cw20() {
    let mut app = mock_app();
    
    // Swap owner deploys
    let swap_admin = Addr::unchecked("swap_deployer");
    // cw721_owner owns the cw721
    let cw721_owner = Addr::unchecked("original_owner");
    // cw20_owner owns the cw20
    let cw20_owner = Addr::unchecked("cw20_owner");
    // random has no cw20 or cw721 tokens
    let random = Addr::unchecked("owns_nothing");

    // cw721_owner creates the cw721
    let nft = create_cw721(&mut app, &cw721_owner);
    
    // swap_admin creates the swap contract 
    let swap = create_swap(&mut app, &swap_admin, nft.clone());
    let swap_inst = swap.clone();
    
    // cw20_owner creates a cw20 coin
    let cw20 = create_cw20(
        &mut app,
        &cw20_owner,
        "testcw".to_string(),
        "tscw".to_string(),
        Uint128::from(100000_u32)
    );

    // cw721_owner mints a cw721 
    let token_id = "petrify".to_string();
    let token_uri = "https://www.merriam-webster.com/dictionary/petrify".to_string();
    let mint_msg = Cw721ExecuteMsg::Mint(MintMsg::<Extension> {
        token_id: token_id.clone(),
        owner: cw721_owner.to_string(),
        token_uri: Some(token_uri.clone()),
        extension: None,
    });
    let _res = app
        .execute_contract(cw721_owner.clone(), nft.clone(), &mint_msg, &[])
        .unwrap();

    // Create a SwapMsg for creating / finishing a swap
    let creation_msg = SwapMsg {
        id: "firstswap".to_string(),
        payment_token: Some(Addr::unchecked(cw20.clone())),
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),  
        price: Uint128::from(100000_u32),
        swap_type: true,
    };
    let finish_msg = creation_msg.clone();

    // Seller (cw721_owner) must approve the swap contract to spend their NFT
    let nft_approve_msg = Cw721ExecuteMsg::Approve::<Extension> {
        spender: swap.to_string(),
        token_id: token_id.clone(),
        expires: None,
    };
    app
        .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
        .unwrap();

    // cw721 seller (cw721_owner) creates a swap
    app
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();

    // cw20_owner does not approve enough funds
    let cw20_approve_msg = Cw20ExecuteMsg::IncreaseAllowance  {
        spender: swap.to_string(),
        amount:  Uint128::from(10000_u32),
        expires: None,
    };
    app
        .execute_contract(cw20_owner.clone(), cw20, &cw20_approve_msg, &[])
        .unwrap();

    // cw20's purchase fails
    assert!(
        app
            .execute_contract(cw20_owner.clone(), swap_inst.clone(), &ExecuteMsg::Finish(finish_msg.clone()), &[])
            .is_err()
    );

    // random has no cw20, their purchase fails
    assert!(
        app
            .execute_contract(random.clone(), swap_inst.clone(), &ExecuteMsg::Finish(finish_msg), &[])
            .is_err()
    );

    // Swap is still open (open == true)
    let swap_query: DetailsResponse = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::Details{
            id: "firstswap".to_string()
        }
    ).unwrap();
   
    assert_eq!(swap_query.open, true);
}

// cw721 buyer increases payment allowance too high
// but correct payment for swap is still enforced
#[test]
fn test_overpayment_cw20() {
    let mut app = mock_app();
    
    // Swap owner deploys
    let swap_admin = Addr::unchecked("swap_deployer");
    // cw721_owner owns the cw721
    let cw721_owner = Addr::unchecked("original_owner");
    // cw20_owner owns the cw20
    let cw20_owner = Addr::unchecked("cw20_owner");

    // cw721_owner creates the cw721
    let nft = create_cw721(&mut app, &cw721_owner);
    
    // swap_admin creates the swap contract 
    let swap = create_swap(&mut app, &swap_admin, nft.clone());
    let swap_inst = swap.clone();
    
    // cw20_owner creates a cw20 coin
    let cw20 = create_cw20(
        &mut app,
        &cw20_owner,
        "testcw".to_string(),
        "tscw".to_string(),
        Uint128::from(1000000_u32)
    );
    let cw20_inst = cw20.clone();

    // cw721_owner mints a cw721 
    let token_id = "petrify".to_string();
    let token_uri = "https://www.merriam-webster.com/dictionary/petrify".to_string();
    let mint_msg = Cw721ExecuteMsg::Mint(MintMsg::<Extension> {
        token_id: token_id.clone(),
        owner: cw721_owner.to_string(),
        token_uri: Some(token_uri.clone()),
        extension: None,
    });
    let _res = app
        .execute_contract(cw721_owner.clone(), nft.clone(), &mint_msg, &[])
        .unwrap();

    // Create a SwapMsg for creating / finishing a swap
    let creation_msg = SwapMsg {
        id: "firstswap".to_string(),
        payment_token: Some(Addr::unchecked(cw20.clone())),
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),  
        price: Uint128::from(100000_u32),
        swap_type: true,
    };
    let finish_msg = creation_msg.clone();

    // Seller (cw721_owner) must approve the swap contract to spend their NFT
    let nft_approve_msg = Cw721ExecuteMsg::Approve::<Extension> {
        spender: swap.to_string(),
        token_id: token_id.clone(),
        expires: None,
    };
    app
        .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
        .unwrap();

    // cw721 seller (cw721_owner) creates a swap
    app
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();

    // cw721 buyer (cw20_owner) allows swap contract to spend too many cw20s
    let cw20_approve_msg = Cw20ExecuteMsg::IncreaseAllowance  {
        spender: swap.to_string(),
        amount:  Uint128::from(900000_u32),
        expires: None,
    };
    app
        .execute_contract(cw20_owner.clone(), cw20, &cw20_approve_msg, &[])
        .unwrap();

    // Buyer purchases cw721, consuming the swap
    app
        .execute_contract(cw20_owner.clone(), swap_inst.clone(), &ExecuteMsg::Finish(finish_msg), &[])
        .unwrap();

    // Swap is now closed (open == false)
    let swap_query: DetailsResponse = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::Details{
            id: "firstswap".to_string()
        }
    ).unwrap();

    // cw20_owner has received the NFT
    let owner_query: OwnerOfResponse = query(
        &mut app,nft.clone(),
        Cw721QueryMsg::OwnerOf {
            token_id: token_id, 
            include_expired: None
        }
    ).unwrap();

    // cw721_owner has still received the correct cw20 amount
    let buyer_balance_query: BalanceResponse = query(
        &mut app,
        cw20_inst.clone(),
        Cw20QueryMsg::Balance {
            address: cw721_owner.to_string()
        }
    ).unwrap();

    // swap contract has spent correct cw20 amount from cw20_owner's balance
    let seller_balance_query: BalanceResponse = query(
        &mut app,
        cw20_inst,
        Cw20QueryMsg::Balance {
            address: cw20_owner.to_string()
        }
    ).unwrap();
   
    assert_eq!(swap_query.open, false);
    assert_eq!(owner_query.owner, cw20_owner);
    assert_eq!(buyer_balance_query.balance, Uint128::from(100000_u32));
    assert_eq!(seller_balance_query.balance, Uint128::from(900000_u32));
}