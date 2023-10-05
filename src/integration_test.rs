#![cfg(test)]
use std::hash::Hash;
use std::collections::HashSet;
use serde::{de::DeserializeOwned, Serialize};

use cosmwasm_std::{
    Addr, BalanceResponse as BalanceResponseBank, BankQuery, Coin, Empty, from_binary, Querier, QueryRequest, 
    StdError, to_binary, Uint128, WasmQuery,
};
use cw_multi_test::{
    App, Contract, ContractWrapper, Executor
};

use cw20::{
    BalanceResponse, Cw20Coin, Cw20ExecuteMsg, Cw20QueryMsg, Expiration,
};
use cw721_base::{
    msg::ExecuteMsg as Cw721ExecuteMsg, msg::InstantiateMsg as Cw721InstantiateMsg, Extension,
    MintMsg, msg::QueryMsg as Cw721QueryMsg
};
use cw721::OwnerOfResponse;

use crate::msg::{
    CancelMsg, DetailsResponse, ExecuteMsg, InstantiateMsg, ListResponse, QueryMsg, SwapMsg, UpdateMsg,
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
    let cw721_id = router.store_code(contract_cw721());
    let msg = Cw721InstantiateMsg {
        name: "TESTNFT".to_string(),
        symbol: "TSNFT".to_string(),
        minter: String::from(minter),
    };   
    let contract = router
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
    let _res = app
        .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
        .unwrap();

    // cw721 seller (cw721_owner) creates a swap
    let _res = app
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();

    // cw721 buyer (cw20_owner) must approve swap contract to spend their cw20
    let cw20_approve_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: swap.to_string(),
        amount:  Uint128::from(100000_u32),
        expires: None,
    };
    let _res = app
        .execute_contract(cw20_owner.clone(), cw20, &cw20_approve_msg, &[])
        .unwrap();

    // Buyer purchases cw721, consuming the swap
    let _res = app
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
    let _res = app
        .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
        .unwrap();

    // cw721 seller (cw721_owner) creates a swap
    let _res = app
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();

    // cw20_owner does not approve enough funds
    let cw20_approve_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: swap.to_string(),
        amount:  Uint128::from(10000_u32),
        expires: None,
    };
    let _res = app
        .execute_contract(cw20_owner.clone(), cw20, &cw20_approve_msg, &[])
        .unwrap();

    // cw20's purchase fails
    assert!(
        app.execute_contract(
            cw20_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Finish(finish_msg.clone()), 
            &[]
        ).is_err()
    );

    // random has no cw20, their purchase fails
    assert!(
        app.execute_contract(
            random.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Finish(finish_msg), 
            &[]
        ).is_err()
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
    let _res = app
        .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
        .unwrap();

    // cw721 seller (cw721_owner) creates a swap
    let _res = app
        .execute_contract(cw20_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();

    // cw721 buyer (cw20_owner) allows swap contract to spend too many cw20s
    let cw20_approve_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: swap.to_string(),
        amount:  Uint128::from(900000_u32),
        expires: None,
    };
    let _res = app
        .execute_contract(cw20_owner.clone(), cw20, &cw20_approve_msg, &[])
        .unwrap();

    // Buyer purchases cw721, consuming the swap
    let _res = app
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
    let _res = app
        .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
        .unwrap();

    // cw721 seller (cw721_owner) creates a swap
    let _res = app
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();

    // Buyer purchases cw721, paying 1 ARCH and consuming the swap
    let _res = app
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
        swap_type: SwapType::Sale,
    };
    let finish_msg = creation_msg.clone();

    // Seller (cw721_owner) must approve the swap contract to spend their NFT
    let nft_approve_msg = Cw721ExecuteMsg::Approve::<Extension> {
        spender: swap.to_string(),
        token_id: token_id.clone(),
        expires: None,
    };
    let _res = app
        .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
        .unwrap();

    // cw721 seller (cw721_owner) creates a swap
    let _res = app
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
    let _res = app
        .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
        .unwrap();

    // cw721 seller (cw721_owner) creates a swap
    let _res = app
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();

    // Buyer purchases cw721, paying 10 ARCH and consuming the swap
    let _res = app
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

#[test]
fn test_native_offer_accepted() {
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

    // Bidding buyer creates an offer
    let swap_id: String = "firstswap".to_string();
    let creation_msg = SwapMsg {
        id: swap_id.clone(),
        payment_token: None,
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(9000000000000000000_u128), // 9 ARCH as aarch
        swap_type: SwapType::Offer,
    };
    let finish_msg = creation_msg.clone();

    let _res = app
        .execute_contract(
            arch_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Create(creation_msg), 
            &[Coin {
                denom: String::from(DENOM),
                amount: Uint128::from(9000000000000000000_u128)
            }]
        ).unwrap();

    // Marketplace contract has received the escrow
    let marketplace_balance: Coin = bank_query(&mut app, &swap_inst);
    assert_eq!(marketplace_balance.amount, Uint128::from(9000000000000000000_u128));

    // cw721_owner must approve the swap contract to spend their NFT
    let nft_approve_msg = Cw721ExecuteMsg::Approve::<Extension> {
        spender: swap.to_string(),
        token_id: token_id.clone(),
        expires: None,
    };
    let _res = app
        .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
        .unwrap();

    // cw721_owner accepts the buyer's offer for 9 ARCH
    let _res = app
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Finish(finish_msg), &[])
        .unwrap();

    // cw721_owner has received 9 ARCH
    let cw721_owner_balance: Coin = bank_query(&mut app, &cw721_owner);
    assert_eq!(cw721_owner_balance.amount, Uint128::from(9000000000000000000_u128));

    // arch_owner's balance is now 1 ARCH
    let arch_owner_balance: Coin = bank_query(&mut app, &arch_owner);
    assert_eq!(arch_owner_balance.amount, Uint128::from(1000000000000000000_u128));

    // arch_owner has received the NFT
    let owner_query: OwnerOfResponse = query(
        &mut app,
        nft.clone(),
        Cw721QueryMsg::OwnerOf {
            token_id: token_id, 
            include_expired: None
        }
    ).unwrap();
    assert_eq!(owner_query.owner, arch_owner);

    // Marketplace contract has released the escrow
    let marketplace_balance: Coin = bank_query(&mut app, &swap_inst);
    assert_eq!(marketplace_balance.amount, Uint128::from(0_u128));
}

// XXX: cw20 spending approvals will only work for one swap at a time
// unless the dapp does some logic to calculate the approval cumulatively
// for all swaps of the cw20 token in question. Unclear how to manage this
// with expiration date and with Offers being consumed by the NFT owner.
// Seems like a nightmare from a state management perspective.
#[test]
fn test_cw20_offer_accepted() {
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

    // Bidding buyer (cw20_owner) must approve swap contract to spend their cw20
    let cw20_approve_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: swap.to_string(),
        amount:  Uint128::from(100000_u32),
        expires: None,
    };
    let _res = app
        .execute_contract(cw20_owner.clone(), cw20.clone(), &cw20_approve_msg, &[])
        .unwrap();

    // Bidding buyer (cw20_owner) creates an offer
    let creation_msg = SwapMsg {
        id: "firstswap".to_string(),
        payment_token: Some(Addr::unchecked(cw20)),
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(100000_u32),
        swap_type: SwapType::Offer,
    };
    let finish_msg = creation_msg.clone();

    let _res = app
        .execute_contract(cw20_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();

    // cw721_owner must approve the swap contract to spend their NFT
    let nft_approve_msg = Cw721ExecuteMsg::Approve::<Extension> {
        spender: swap.to_string(),
        token_id: token_id.clone(),
        expires: None,
    };
    let _res = app
        .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
        .unwrap();

    // cw721_owner accepts the cw20 buyer's offer
    let _res = app
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
    assert_eq!(owner_query.owner, cw20_owner);

    // cw721_owner has received the cw20 amount
    let balance_query: BalanceResponse = query(
        &mut app,
        cw20_inst,
        Cw20QueryMsg::Balance {
            address: cw721_owner.to_string()
        }
    ).unwrap();
    assert_eq!(balance_query.balance, Uint128::from(100000_u32));
}

// Over paying will fail, must send exactly the required funds
// to create a SwapType::Offer for native ARCH
#[test]
fn test_overpayment_native_offer() {
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

    // Bidding buyer creates an offer
    let swap_id: String = "firstswap".to_string();
    let creation_msg = SwapMsg {
        id: swap_id.clone(),
        payment_token: None,
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(5000000000000000000_u128), // 5 ARCH as aarch
        swap_type: SwapType::Offer,
    };

    // Sending more funds than the value of price in creation_msg
    // causes the tx to fail with ContractError::ExactFunds
    assert!(
        app.execute_contract(
            arch_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Create(creation_msg), 
            &[Coin {
                denom: String::from(DENOM),
                amount: Uint128::from(9000000000000000000_u128)
            }]
        ).is_err()
    );
}

#[test]
fn test_invalid_payment_native_offer() {
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

    // Bidding buyer creates an offer (with an invalid payment)
    let swap_id: String = "firstswap".to_string();
    let creation_msg = SwapMsg {
        id: swap_id.clone(),
        payment_token: None,
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(9000000000000000000_u128), // 9 ARCH as aarch
        swap_type: SwapType::Offer,
    };

    // Invalid payment must err
    assert!(
        app.execute_contract(
            arch_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Create(creation_msg), 
            &[Coin {
                denom: String::from(DENOM),
                amount: Uint128::from(8000000000000000000_u128)
            }]
        ).is_err()
    );

    // cw721_owner has not transferred the NFT
    let owner_query: OwnerOfResponse = query(
        &mut app,nft.clone(),
        Cw721QueryMsg::OwnerOf {
            token_id: token_id, 
            include_expired: None
        }
    ).unwrap();
    assert_eq!(owner_query.owner, cw721_owner);

    // Bidding buyer's account has not been debited
    let arch_owner_balance: Coin = bank_query(&mut app, &arch_owner);
    assert_eq!(arch_owner_balance.amount, Uint128::from(10000000000000000000_u128));
}

#[test]
fn test_invalid_payment_cw20_offer() {
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

    // Bidding buyer (cw20_owner) does not approve enough funds
    let cw20_approve_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: swap.to_string(),
        amount:  Uint128::from(10000_u32),
        expires: None,
    };
    let _res = app
        .execute_contract(cw20_owner.clone(), cw20.clone(), &cw20_approve_msg, &[])
        .unwrap();

    // Bidding buyer (cw20_owner) creates an offer
    let creation_msg = SwapMsg {
        id: "firstswap".to_string(),
        payment_token: Some(Addr::unchecked(cw20)),
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(100000_u32),
        swap_type: SwapType::Offer,
    };
    let finish_msg = creation_msg.clone();

    let _res = app
        .execute_contract(cw20_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();

    // cw721_owner must approve the swap contract to spend their NFT
    let nft_approve_msg = Cw721ExecuteMsg::Approve::<Extension> {
        spender: swap.to_string(),
        token_id: token_id.clone(),
        expires: None,
    };
    let _res = app
        .execute_contract(cw721_owner.clone(), nft.clone(), &nft_approve_msg, &[])
        .unwrap();

    // cw721_owner accepts the cw20 buyer's offer but the swap must fail (invalid payment)
    assert!(
        app.execute_contract(
            cw721_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Finish(finish_msg), 
            &[]
        ).is_err()
    );

    // cw721_owner has not transferred the NFT
    let owner_query: OwnerOfResponse = query(
        &mut app,nft.clone(),
        Cw721QueryMsg::OwnerOf {
            token_id: token_id, 
            include_expired: None
        }
    ).unwrap();
    assert_eq!(owner_query.owner, cw721_owner);

    // cw20_owner has not transferred the cw20 amount
    let balance_query: BalanceResponse = query(
        &mut app,
        cw20_inst,
        Cw20QueryMsg::Balance {
            address: cw20_owner.to_string()
        }
    ).unwrap();
    assert_eq!(balance_query.balance, Uint128::from(100000_u32));
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
        let _res = app
            .execute_contract(cw721_owner.clone(), nft.clone(), &mint_msg, &[])
            .unwrap();

        // Approval msg
        let nft_approve_msg = Cw721ExecuteMsg::Approve::<Extension> {
            spender: swap.to_string(),
            token_id: token_id.clone(),
            expires: None,
        };
        // Do approve marketplace as spender
        let _res = app
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
        let _res = app
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
    
    // Paginated results must not have duplicates
    let mut all_res = page_1.swaps.clone();
    all_res.append(&mut page_2.swaps.clone());
    all_res.append(&mut page_3.swaps.clone());
    assert!(has_unique_elements(all_res));

    // Paginated results must each have a size equal to `limit`
    assert_eq!(page_1.swaps.len(), 5);
    assert_eq!(page_2.swaps.len(), 5);
    assert_eq!(page_3.swaps.len(), 5);

    // Query GetListings entry point for 2 pages
    // Page 1
    let page_1b: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::GetListings {
            page: None,
            limit: None,
        }
    ).unwrap();
    // Page 2
    let page_2b: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::GetListings {
            page: Some(1_u32),
            limit: None,
        }
    ).unwrap();

    // Paginated results must have correct page sizes
    assert_eq!(page_1b.len(), 10);
    assert_eq!(page_2b.len(), 5);

    // Paginated results must not have duplicates
    let mut all_res_b = page_1b.clone();
    all_res_b.append(&mut page_2b.clone());
    let mut token_ids_b: Vec<String> = vec![];
    for swap in all_res_b.iter() {
        token_ids_b.push(swap.clone().token_id);
    }
    assert!(has_unique_elements(token_ids_b));

    // Query SwapsOf entry point for 2 pages
    // Page 1
    let page_1c: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::SwapsOf {
            address: cw721_owner.clone(),
            swap_type: Some(SwapType::Sale),
            page: None,
            limit: None,
        }
    ).unwrap();
    // Page 2
    let page_2c: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::SwapsOf {
            address: cw721_owner.clone(),
            swap_type: Some(SwapType::Sale),
            page: Some(1_u32),
            limit: None,
        }
    ).unwrap();

    // Paginated results must have correct page sizes
    assert_eq!(page_1c.len(), 10);
    assert_eq!(page_2c.len(), 5);

    // Paginated results must not have duplicates
    let mut all_res_c = page_1c.clone();
    all_res_c.append(&mut page_2c.clone());
    let mut token_ids_c: Vec<String> = vec![];
    for swap in all_res_c.iter() {
        token_ids_c.push(swap.clone().token_id);
    }
    assert!(has_unique_elements(token_ids_c));

    // Query SwapsByPrice entry point for 2 pages
    // Page 1
    let page_1d: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::SwapsByPrice {
            min: Some(Uint128::from(0_u128)),
            max: Some(Uint128::from(1000000000000000000_u128)),
            swap_type: Some(SwapType::Sale),
            page: None,
            limit: None,
        }
    ).unwrap();
    // Page 2
    let page_2d: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::SwapsByPrice {
            min: Some(Uint128::from(0_u128)),
            max: Some(Uint128::from(1000000000000000000_u128)),
            swap_type: Some(SwapType::Sale),
            page: Some(1_u32),
            limit: None,
        }
    ).unwrap();

    // Paginated results must have correct page sizes
    assert_eq!(page_1d.len(), 10);
    assert_eq!(page_2d.len(), 5);

    // Paginated results must not have duplicates
    let mut all_res_d = page_1d.clone();
    all_res_d.append(&mut page_2d.clone());
    let mut token_ids_d: Vec<String> = vec![];
    for swap in all_res_d.iter() {
        token_ids_d.push(swap.clone().token_id);
    }
    assert!(has_unique_elements(token_ids_d));

    // Query SwapsByDenom entry point for 2 pages
    // Page 1
    let page_1e: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::SwapsByDenom {
            payment_token: None,
            swap_type: Some(SwapType::Sale),
            page: None,
            limit: None,
        }
    ).unwrap();
    // Page 2
    let page_2e: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::SwapsByDenom {
            payment_token: None,
            swap_type: Some(SwapType::Sale),
            page: Some(1_u32),
            limit: None,
        }
    ).unwrap();

    // Paginated results must have correct page sizes
    assert_eq!(page_1e.len(), 10);
    assert_eq!(page_2e.len(), 5);

    // Paginated results must not have duplicates
    let mut all_res_e = page_1e.clone();
    all_res_e.append(&mut page_2e.clone());
    let mut token_ids_e: Vec<String> = vec![];
    for swap in all_res_e.iter() {
        token_ids_e.push(swap.clone().token_id);
    }
    assert!(has_unique_elements(token_ids_e));


    // Query SwapsByPaymentType entry point for 2 pages
    // Page 1
    let page_1f: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::SwapsByPaymentType {
            cw20: false,
            swap_type: Some(SwapType::Sale),
            page: None,
            limit: None,
        }
    ).unwrap();
    // Page 2
    let page_2f: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::SwapsByPaymentType {
            cw20: false,
            swap_type: Some(SwapType::Sale),
            page: Some(1_u32),
            limit: None,
        }
    ).unwrap();

    // Paginated results must have correct page sizes
    assert_eq!(page_1f.len(), 10);
    assert_eq!(page_2f.len(), 5);

    // Paginated results must not have duplicates
    let mut all_res_f = page_1f.clone();
    all_res_f.append(&mut page_2f.clone());
    let mut token_ids_f: Vec<String> = vec![];
    for swap in all_res_f.iter() {
        token_ids_f.push(swap.clone().token_id);
    }
    assert!(has_unique_elements(token_ids_f));

    // Query ListingsOfToken entry point (All Listings)
    let listings_of_token_a: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::ListingsOfToken {
            token_id: "token10".to_string(),
            swap_type: None, // All Listings
            page: None,
            limit: None,
        }
    ).unwrap();
    // 1 Result
    assert_eq!(listings_of_token_a.len(), 1);

    // Query ListingsOfToken entry point (Sales)
    let listings_of_token_b: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::ListingsOfToken {
            token_id: "token10".to_string(),
            swap_type: Some(SwapType::Sale), // Sale Listings
            page: None,
            limit: None,
        }
    ).unwrap();
    // 1 Result
    assert_eq!(listings_of_token_b.len(), 1);

    // Query ListingsOfToken entry point (Offers)
    let listings_of_token_c: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::ListingsOfToken {
            token_id: "token10".to_string(),
            swap_type: Some(SwapType::Offer), // Offer Listings
            page: None,
            limit: None,
        }
    ).unwrap();
    // 0 Results
    assert_eq!(listings_of_token_c.len(), 0);
}

// Seller must be able to cancel sale
// cw20 and native ARCH
#[test]
fn test_cancel_sales() {
    let mut app = mock_app();
    
    // Swap owner deploys
    let swap_admin = Addr::unchecked("swap_deployer");
    // cw721_owner owns the cw721
    let cw721_owner = Addr::unchecked("original_owner");

    // cw721_owner creates the cw721
    let nft = create_cw721(&mut app, &cw721_owner);
    
    // swap_admin creates the swap contract 
    let swap = create_swap(&mut app, &swap_admin, nft.clone());
    let swap_inst = swap.clone();

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
    let swap_id: String = "firstswap".to_string();
    let creation_msg = SwapMsg {
        id: swap_id.clone(),
        payment_token: None,
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(1000000000000000000_u128), // 1 ARCH as aarch
        swap_type: SwapType::Sale,
    };

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
    let _res = app
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();
    
    // Query ListingsOfToken entry point (Sales)
    let listings_of_token: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::ListingsOfToken {
            token_id: token_id.clone(),
            swap_type: Some(SwapType::Sale),
            page: None,
            limit: None,
        }
    ).unwrap();
    // 1 Result
    assert_eq!(listings_of_token.len(), 1);

    // cw721 seller (cw721_owner) cancels the swap
    let cancel_msg = CancelMsg { id: swap_id };
    let _res = app
        .execute_contract(cw721_owner, swap_inst.clone(), &ExecuteMsg::Cancel(cancel_msg), &[])
        .unwrap();
    
    // Query ListingsOfToken entry point (Sales)
    let listings_of_token: Vec<CW721Swap> = query(
        &mut app,
        swap_inst,
        QueryMsg::ListingsOfToken {
            token_id: token_id,
            swap_type: Some(SwapType::Sale),
            page: None,
            limit: None,
        }
    ).unwrap();
    // 0 Results
    assert_eq!(listings_of_token.len(), 0);
}

// Bidders must be able to cancel offers
// Canceling of an ARCH offer should return escrow
#[test]
fn test_cancel_offers() {
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

    // Bidding buyer creates an offer
    let swap_id: String = "firstswap".to_string();
    let creation_msg = SwapMsg {
        id: swap_id.clone(),
        payment_token: None,
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(9000000000000000000_u128), // 9 ARCH as aarch
        swap_type: SwapType::Offer,
    };

    let _res = app
        .execute_contract(
            arch_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Create(creation_msg), 
            &[Coin {
                denom: String::from(DENOM),
                amount: Uint128::from(9000000000000000000_u128)
            }]
        ).unwrap();

    // Marketplace contract has received the escrow
    let marketplace_balance: Coin = bank_query(&mut app, &swap_inst);
    assert_eq!(marketplace_balance.amount, Uint128::from(9000000000000000000_u128));

    // Bidding buyer's account has been debited
    let arch_owner_balance: Coin = bank_query(&mut app, &arch_owner);
    assert_eq!(arch_owner_balance.amount, Uint128::from(1000000000000000000_u128));

    // Query ListingsOfToken entry point (Offer)
    let listings_of_token: Vec<CW721Swap> = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::ListingsOfToken {
            token_id: token_id.clone(),
            swap_type: Some(SwapType::Offer),
            page: None,
            limit: None,
        }
    ).unwrap();
    // 1 Result
    assert_eq!(listings_of_token.len(), 1);

    // Bidding buyer (arch_owner) cancels the swap
    let cancel_msg = CancelMsg { id: swap_id };
    let _res = app
        .execute_contract(arch_owner.clone(), swap_inst.clone(), &ExecuteMsg::Cancel(cancel_msg), &[])
        .unwrap();

    // Marketplace contract has released the escrow
    let marketplace_balance: Coin = bank_query(&mut app, &swap_inst);
    assert_eq!(marketplace_balance.amount, Uint128::from(0_u128));

    // Bidding buyer's account has been debited
    let arch_owner_balance: Coin = bank_query(&mut app, &arch_owner);
    assert_eq!(arch_owner_balance.amount, Uint128::from(10000000000000000000_u128));

    // Query ListingsOfToken entry point (Offer)
    let listings_of_token: Vec<CW721Swap> = query(
        &mut app,
        swap_inst,
        QueryMsg::ListingsOfToken {
            token_id: token_id,
            swap_type: Some(SwapType::Offer),
            page: None,
            limit: None,
        }
    ).unwrap();
    // 0 Results
    assert_eq!(listings_of_token.len(), 0);
}

// Updating SwapType::Sale
#[test]
fn test_updating_sales() {
    let mut app = mock_app();
    
    // Swap owner deploys
    let swap_admin = Addr::unchecked("swap_deployer");
    // cw721_owner owns the cw721
    let cw721_owner = Addr::unchecked("original_owner");

    // cw721_owner creates the cw721
    let nft = create_cw721(&mut app, &cw721_owner);
    
    // swap_admin creates the swap contract 
    let swap = create_swap(&mut app, &swap_admin, nft.clone());
    let swap_inst = swap.clone();

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
    let swap_id: String = "firstswap".to_string();
    let creation_msg = SwapMsg {
        id: swap_id.clone(),
        payment_token: None,
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(1000000000000000000_u128), // 1 ARCH as aarch
        swap_type: SwapType::Sale,
    };

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
    let _res = app
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(creation_msg), &[])
        .unwrap();

    // Original swap details (price and expiration) are correct
    let swap_details: DetailsResponse = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::Details {
            id: swap_id.clone(),
        }
    ).unwrap();
    assert_eq!(swap_details.expires, Expiration::from(cw20::Expiration::AtHeight(384798573487439743)));
    assert_eq!(swap_details.price, Uint128::from(1000000000000000000_u128));

    // cw721 seller (cw721_owner) updates the swap
    let update_msg = UpdateMsg {
        id: swap_id.clone(),
        expires: Expiration::from(cw20::Expiration::AtHeight(400000000000000000)),
        price: Uint128::from(2000000000000000000_u128),
    };
    let _res = app
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Update(update_msg), &[])
        .unwrap();

    // Swap details (price and expiration) must be updated
    let swap_details: DetailsResponse = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::Details {
            id: swap_id.clone(),
        }
    ).unwrap();
    assert_eq!(swap_details.expires, Expiration::from(cw20::Expiration::AtHeight(400000000000000000)));
    assert_eq!(swap_details.price, Uint128::from(2000000000000000000_u128));
}

// Updating SwapType::Offer
#[test]
fn test_updating_offers() {
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

    // Bidding buyer creates an offer
    let swap_id: String = "firstswap".to_string();
    let creation_msg = SwapMsg {
        id: swap_id.clone(),
        payment_token: None,
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(9000000000000000000_u128), // 9 ARCH as aarch
        swap_type: SwapType::Offer,
    };

    let _res = app
        .execute_contract(
            arch_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Create(creation_msg), 
            &[Coin {
                denom: String::from(DENOM),
                amount: Uint128::from(9000000000000000000_u128)
            }]
        ).unwrap();

    // Marketplace contract has received the escrow
    let marketplace_balance: Coin = bank_query(&mut app, &swap_inst);
    assert_eq!(marketplace_balance.amount, Uint128::from(9000000000000000000_u128));

    // Bidding buyer's account has been debited
    let arch_owner_balance: Coin = bank_query(&mut app, &arch_owner);
    assert_eq!(arch_owner_balance.amount, Uint128::from(1000000000000000000_u128));

    // Original swap details (price and expiration) are correct
    let swap_details: DetailsResponse = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::Details {
            id: swap_id.clone(),
        }
    ).unwrap();

    assert_eq!(swap_details.expires, Expiration::from(cw20::Expiration::AtHeight(384798573487439743)));
    assert_eq!(swap_details.price, Uint128::from(9000000000000000000_u128));

    // Bidder (arch_owner) updates the swap
    let update_msg = UpdateMsg {
        id: swap_id.clone(),
        expires: Expiration::from(cw20::Expiration::AtHeight(400000000000000000)),
        price: Uint128::from(1000000000000000000_u128),
    };
    // Swap update fails if the correct updated escrow amount is not sent
    assert!(
        // No escrow sent
        app.execute_contract(
            arch_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Update(update_msg.clone()), 
            &[],
        ).is_err()
    );
    assert!(
        // Escrow too low
        app.execute_contract(
            arch_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Update(update_msg.clone()), 
            &[Coin {
                denom: String::from(DENOM),
                amount: Uint128::from(10000000000_u128),
            }],
        ).is_err()
    );
    // Sending UpdateMsg with the correct updated escrow succeeds
    let _res = app
        .execute_contract(
            arch_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Update(update_msg), 
            &[Coin {
                denom: String::from(DENOM),
                amount: Uint128::from(1000000000000000000_u128),
            }],
        )
        .unwrap();

    // Marketplace contract has released the legacy escrow (9 ARCH)
    // and received the incoming escrow (1 ARCH) of the new swap price
    let marketplace_balance: Coin = bank_query(&mut app, &swap_inst);
    assert_eq!(marketplace_balance.amount, Uint128::from(1000000000000000000_u128));

    // Bidding buyer's account has been debited the new swap price (1 ARCH)
    // and has received the legacy escrow (9 ARCH) from the marketplace
    let arch_owner_balance: Coin = bank_query(&mut app, &arch_owner);
    assert_eq!(arch_owner_balance.amount, Uint128::from(9000000000000000000_u128));
}