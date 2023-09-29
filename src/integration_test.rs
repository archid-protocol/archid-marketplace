#![cfg(test)]
use std::hash::Hash;
use std::collections::HashSet;
use serde::{de::DeserializeOwned, Serialize};

use cosmwasm_std::{
    Addr, BalanceResponse as BalanceResponseBank, BankQuery, Coin, Querier, QueryRequest, Empty, 
    from_binary, to_binary, Uint128, StdError, WasmQuery,
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
    ExecuteMsg, ListResponse, QueryMsg, SwapMsg, InstantiateMsg
};
use crate::state::{CW721Swap, SwapType};
use crate::contract::DENOM;

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

fn mint_native(app: &mut App, beneficiary: String, amount: Uint128) {
    app.sudo(cw_multi_test::SudoMsg::Bank(
        cw_multi_test::BankSudo::Mint {
            to_address: beneficiary,
            amount: vec![Coin {
                denom: DENOM.to_string(),
                amount: amount,
            }],
        },
    ))
    .unwrap();
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

fn bank_query(app: &App, address: &Addr) -> Coin {
    let req: QueryRequest<BankQuery> = QueryRequest::Bank(BankQuery::Balance { 
        address: address.to_string(), 
        denom: DENOM.to_string() 
    });
    let res = app.raw_query(&to_binary(&req).unwrap()).unwrap().unwrap();
    let balance: BalanceResponseBank = from_binary(&res).unwrap();
    return balance.amount;
}

fn has_unique_elements<T>(iter: T) -> bool
where
    T: IntoIterator,
    T::Item: Eq + Hash,
{
    let mut uniq = HashSet::new();
    iter.into_iter().all(move |x| uniq.insert(x))
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
        swap_type: SwapType::Sale,
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
    let cw20_approve_msg = Cw20ExecuteMsg::IncreaseAllowance {
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
        swap_type:SwapType::Offer,
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
    let cw20_approve_msg = Cw20ExecuteMsg::IncreaseAllowance {
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
        swap_type:SwapType::Offer,
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
        .execute_contract(cw20_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();

    // cw721 buyer (cw20_owner) allows swap contract to spend too many cw20s
    let cw20_approve_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: swap.to_string(),
        amount:  Uint128::from(900000_u32),
        expires: None,
    };
    app
        .execute_contract(cw20_owner.clone(), cw20, &cw20_approve_msg, &[])
        .unwrap();

    // Buyer purchases cw721, consuming the swap
    app
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Finish(finish_msg), &[])
        .unwrap();

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

    assert_eq!(owner_query.owner, cw20_owner);
    assert_eq!(buyer_balance_query.balance, Uint128::from(100000_u32));
    assert_eq!(seller_balance_query.balance, Uint128::from(900000_u32));
}

// Swap buyer pays with ARCH
#[test]
fn test_buy_native() {
    let mut app = mock_app();
    
    // Swap owner deploys
    let swap_admin = Addr::unchecked("swap_deployer");
    // cw721_owner owns the cw721
    let cw721_owner = Addr::unchecked("original_owner");
    // arch_owner owns ARCH
    let arch_owner = Addr::unchecked("arch_owner");

    // cw721_owner creates the cw721
    let nft = create_cw721(&mut app, &cw721_owner);
    
    // swap_admin creates the swap contract 
    let swap = create_swap(&mut app, &swap_admin, nft.clone());
    let swap_inst = swap.clone();
    
    // Mint native to `arch_owner`
    mint_native(
        &mut app,
        arch_owner.to_string(),
        Uint128::from(10000000000000000000_u128), // 10 ARCH as aarch
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
        payment_token: None,
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(1000000000000000000_u128), // 1 ARCH as aarch
        swap_type: SwapType::Sale,
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

    // Buyer purchases cw721, paying 1 ARCH and consuming the swap
    app
        .execute_contract(
            arch_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Finish(finish_msg), 
            &[Coin {
                denom: String::from(DENOM),
                amount: Uint128::from(1000000000000000000_u128)
            }]
        )
        .unwrap();


    // arch_owner has received the NFT
    let owner_query: OwnerOfResponse = query(
        &mut app,
        nft.clone(),
        Cw721QueryMsg::OwnerOf {
            token_id: token_id, 
            include_expired: None
        }
    ).unwrap();

    // cw721_owner has received the ARCH amount
    let balance_query: Coin = bank_query(&mut app, &cw721_owner);

    assert_eq!(owner_query.owner, arch_owner);
    assert_eq!(balance_query.amount, Uint128::from(1000000000000000000_u128));
}

// cw721 buyer must send correct ARCH amount
#[test]
fn test_invalid_payment_native() {
    let mut app = mock_app();
    
    // Swap owner deploys
    let swap_admin = Addr::unchecked("swap_deployer");
    // cw721_owner owns the cw721
    let cw721_owner = Addr::unchecked("original_owner");
    // arch_owner owns ARCH
    let arch_owner = Addr::unchecked("arch_owner");

    // cw721_owner creates the cw721
    let nft = create_cw721(&mut app, &cw721_owner);
    
    // swap_admin creates the swap contract 
    let swap = create_swap(&mut app, &swap_admin, nft.clone());
    let swap_inst = swap.clone();
    
    // Mint native to `arch_owner`
    mint_native(
        &mut app,
        arch_owner.to_string(),
        Uint128::from(10000000000000000000_u128), // 10 ARCH as aarch
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
        payment_token: None,
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(5000000000000000000_u128), // 5 ARCH as aarch
        swap_type:SwapType::Offer,
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

    // Buyer attempts to purchase cw721, under paying 1 ARCH
    assert!(app.execute_contract(
        arch_owner.clone(), 
        swap_inst.clone(), 
        &ExecuteMsg::Finish(finish_msg), 
        &[Coin {
            denom: String::from(DENOM),
            amount: Uint128::from(1000000000000000000_u128)
        }]
    )
    .is_err());

    // cw721_owner has retained the NFT
    let owner_query: OwnerOfResponse = query(
        &mut app,
        nft.clone(),
        Cw721QueryMsg::OwnerOf {
            token_id: token_id, 
            include_expired: None
        }
    ).unwrap();

    // cw721_owner has not received the ARCH amount
    let cw721_owner_balance: Coin = bank_query(&mut app, &cw721_owner);
    // dbg!(cw721_owner_balance.amount);

    // arch_owner has retained their original balance (minus gas fees)
    let arch_owner_balance: Coin = bank_query(&mut app, &cw721_owner);
    // dbg!(arch_owner_balance.amount);

    assert_eq!(cw721_owner_balance.amount.u128(), 0);
    assert_eq!(arch_owner_balance.amount.u128(), 0);
    assert_eq!(owner_query.owner, cw721_owner);
}

// cw721 buyer (arch_owner) overpays
// seller (cw721_owner) receives the full overpaid amount
#[test]
fn test_overpayment_native() {
    let mut app = mock_app();
    
    // Swap owner deploys
    let swap_admin = Addr::unchecked("swap_deployer");
    // cw721_owner owns the cw721
    let cw721_owner = Addr::unchecked("original_owner");
    // arch_owner owns ARCH
    let arch_owner = Addr::unchecked("arch_owner");

    // cw721_owner creates the cw721
    let nft = create_cw721(&mut app, &cw721_owner);
    
    // swap_admin creates the swap contract 
    let swap = create_swap(&mut app, &swap_admin, nft.clone());
    let swap_inst = swap.clone();
    
    // Mint native to `arch_owner`
    mint_native(
        &mut app,
        arch_owner.to_string(),
        Uint128::from(10000000000000000000_u128), // 10 ARCH as aarch
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
        payment_token: None,
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(1000000000000000000_u128), // 1 ARCH as aarch
        swap_type: SwapType::Sale,
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

    // Buyer purchases cw721, paying 10 ARCH and consuming the swap
    app
        .execute_contract(
            arch_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Finish(finish_msg), 
            &[Coin {
                denom: String::from(DENOM),
                amount: Uint128::from(10000000000000000000_u128)
            }]
        )
        .unwrap();

    // arch_owner has received the NFT
    let owner_query: OwnerOfResponse = query(
        &mut app,
        nft.clone(),
        Cw721QueryMsg::OwnerOf {
            token_id: token_id, 
            include_expired: None
        }
    ).unwrap();

    // cw721_owner has received the ARCH amount
    let balance_query: Coin = bank_query(&mut app, &cw721_owner);

    assert_eq!(owner_query.owner, arch_owner);
    assert_eq!(balance_query.amount, Uint128::from(10000000000000000000_u128));
}

// Listing swaps should be enumerable
#[test]
fn test_pagination() {
    let mut app = mock_app();
    
    // Swap owner deploys
    let swap_admin = Addr::unchecked("swap_deployer");

    // cw721_owner owns cw721 tokens
    let cw721_owner = Addr::unchecked("cw721_owner");

    // cw721_owner creates cw721 token contract
    let nft = create_cw721(&mut app, &cw721_owner);

    // swap_admin creates the swap contract 
    let swap = create_swap(&mut app, &swap_admin, nft.clone());
    let swap_inst = swap.clone();

    // cw721_owner mints 15 cw721 tokens 
    let token_ids = vec![
        // Page 1 of 3
        "token1".to_string(),"token2".to_string(),"token3".to_string(),"token4".to_string(),"token5".to_string(),
        // Page 2 of 3
        "token6".to_string(),"token7".to_string(),"token8".to_string(),"token9".to_string(),"token10".to_string(),
        // Page 3 of 3
        "token11".to_string(),"token12".to_string(),"token13".to_string(),"token14".to_string(),"token15".to_string(),
    ];
    let token_uri = "https://www.merriam-webster.com/dictionary/petrify".to_string();

    // Mint 15 tokens and create a swap for each
    for token_id in token_ids.iter() {
        // Mint msg
        let mint_msg = Cw721ExecuteMsg::Mint(MintMsg::<Extension> {
            token_id: token_id.clone(),
            owner: cw721_owner.to_string(),
            token_uri: Some(token_uri.clone()),
            extension: None,
        });
        // Do minting
        app
            .execute_contract(cw721_owner.clone(), nft.clone(), &mint_msg, &[])
            .unwrap();

        // Approval msg
        let nft_approve_msg = Cw721ExecuteMsg::Approve::<Extension> {
            spender: swap.to_string(),
            token_id: token_id.clone(),
            expires: None,
        };
        // Do approve marketplace as spender
        app
            .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
            .unwrap();

        // Swap msg
        let creation_msg = SwapMsg {
            id: token_id.clone(),
            payment_token: None,
            token_id: token_id.clone(),    
            expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
            price: Uint128::from(1000000000000000000_u128), // 1 ARCH as aarch
            swap_type: SwapType::Sale,
        };
        // Create swap listing
        app
            .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
            .unwrap();
    }

    // Query List entry point for 3 pages
    // Paging size
    let limit: u32 = 5;
    // Page 1
    let page_1: ListResponse = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::List {
            start_after: None,
            limit: Some(limit.clone()),
        }
    ).unwrap();
    // Page 2
    let page_2: ListResponse = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::List {
            start_after: Some(page_1.swaps[4].clone()),
            limit: Some(limit.clone()),
        }
    ).unwrap();
    // Page 3
    let page_3: ListResponse = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::List {
            start_after: Some(page_2.swaps[4].clone()),
            limit: Some(limit.clone()),
        }
    ).unwrap();
    
    // Paginated results must not have any duplicates
    let mut all_res = page_1.swaps.clone();
    all_res.append(&mut page_2.swaps.clone());
    all_res.append(&mut page_3.swaps.clone());
    assert!(has_unique_elements(all_res));

    // Paginated results must each have a size equal to `limit`
    assert_eq!(page_1.swaps.len(), 5);
    assert_eq!(page_2.swaps.len(), 5);
    assert_eq!(page_3.swaps.len(), 5);

    // Query GetListings entry point for 3 pages
    // Page 1
    let page_1b: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::GetListings {
            page: None,
            limit: Some(limit.clone()),
        }
    ).unwrap();
    // Page 2
    let page_2b: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::GetListings {
            page: Some(1_u32),
            limit: Some(limit.clone()),
        }
    ).unwrap();

    // Paginated results must have correct page sizes
    assert_eq!(page_1b.len(), 10);
    assert_eq!(page_2b.len(), 5);

    // Paginated results must not have any duplicates
    let mut all_res_b = page_1b.clone();
    all_res_b.append(&mut page_2b.clone());
    let mut token_ids_b: Vec<String> = vec![];
    for swap in all_res_b.iter() {
        token_ids_b.push(swap.clone().token_id);
    }
    assert!(has_unique_elements(token_ids_b));
}