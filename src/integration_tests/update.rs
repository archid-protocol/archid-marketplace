#![cfg(test)]
use cosmwasm_std::{
    Addr, Coin, Uint128,
};
use cw_multi_test::Executor;

use cw20::Expiration;
use cw721_base::{
    msg::ExecuteMsg as Cw721ExecuteMsg, Extension, MintMsg,
};

use crate::integration_tests::util::{
    bank_query, create_cw721, create_swap, mint_native, mock_app, query,
};
use crate::msg::{
    DetailsResponse, ExecuteMsg, QueryMsg, SwapMsg, UpdateMsg,
};
use crate::state::{SwapType};
use crate::contract::DENOM;

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