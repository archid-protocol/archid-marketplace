#![cfg(test)]
use cosmwasm_std::{
    Addr, Coin, Uint128,
};
use cw_multi_test::Executor;

use cw20::Expiration;
use cw721_base::{
    msg::ExecuteMsg as Cw721ExecuteMsg, Extension, MintMsg, msg::QueryMsg as Cw721QueryMsg,
};
use cw721::OwnerOfResponse;

use crate::integration_tests::util::{
    bank_query, create_cw721, create_swap, mint_native, mock_app, query,
};
use crate::msg::{
    ExecuteMsg, ListResponse, QueryMsg, SwapMsg,
};
use crate::state::{SwapType};
use crate::contract::DENOM;

// After finishing a swap, all swaps for the same token
// must be removed from storage (they'd be invalid)
#[test]
fn test_finshing_swap_prunes_storage() {
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
    let sale_msg = SwapMsg {
        id: "firstswap".to_string(),
        payment_token: None,
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(1000000000000000000_u128), // 1 ARCH as aarch
        swap_type: SwapType::Sale,
    };
    let finish_msg = sale_msg.clone();

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
        .execute_contract(cw721_owner.clone(), swap_inst.clone(), &ExecuteMsg::Create(sale_msg), &[])
        .unwrap();

    // arch_owner makes an offer for the same token (for a lower price)
    let offer_msg = SwapMsg {
        id: "secondswap".to_string(),
        payment_token: None,
        token_id: token_id.clone(),    
        expires: Expiration::from(cw20::Expiration::AtHeight(384798573487439743)),
        price: Uint128::from(100000000000000000_u128), // 0.1 ARCH as aarch
        swap_type: SwapType::Offer,
    };

    let _res = app
        .execute_contract(
            arch_owner.clone(), 
            swap_inst.clone(), 
            &ExecuteMsg::Create(offer_msg),
            &[Coin {
                denom: String::from(DENOM),
                amount: Uint128::from(100000000000000000_u128)
            }]
        ).unwrap();

    // Marketplace contract has received the escrow
    let marketplace_balance: Coin = bank_query(&mut app, &swap_inst);
    assert_eq!(marketplace_balance.amount, Uint128::from(100000000000000000_u128));

    // Buyer purchases cw721, paying 1 ARCH and consuming the original swap
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

    // Marketplace contract has released the escrow
    let marketplace_balance: Coin = bank_query(&mut app, &swap_inst);
    assert_eq!(marketplace_balance.amount, Uint128::from(0_u128));

    // arch_owner has received the released escrow
    // balance must be 9 ARCH (not 8.9 ARCH)
    let arch_owner_balance: Coin = bank_query(&mut app, &arch_owner);
    assert_eq!(arch_owner_balance.amount, Uint128::from(9000000000000000000_u128));

    // Total swaps must be 0 
    // (both swaps removed from storage)
    let limit: u32 = 5;
    let swap_list: ListResponse = query(
        &mut app,
        swap_inst.clone(),
        QueryMsg::List {
            start_after: None,
            limit: Some(limit.clone()),
        }
    ).unwrap();
    assert_eq!(swap_list.swaps.len(), 0);
}