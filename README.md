# Marketplace Contract
Swapping nfts of a single collection.

## Queries
`Config{}`: Get basic information about the marketplace, such as which NFT collections are allowed to list in the marketplace, and what percentage of fees are retained from Sales and Offers.

`List{start_after, limit}`: Get a paginated list of all swap ids. Pagination is identical to cw721 enumerability (e.g. start_after strings), but all other paginated entry points use numeric page numbers (not start_after strings).

`Details{id}`: Fetch details for a specific swap

`SwapsOf{address, swap_type, page, limit}`: Get all swaps created by a specific address

`GetTotal{swap_type}`: swap_type is optional. Get the total number of swaps, or the total number of swaps for a `SwapType` (`'Sale'` / `'Offer'`).

`GetOffers{page, limit}`: Fetch all swaps of type `SwapType::Offer`

`GetListings{page, limit}`: Fetch all swaps of type `SwapType::Sale`

`ListingsOfToken{token_id, cw721, swap_type, page, limit}`: Fetch all swaps for a specific token ID; can optionally be filtered by swap type.

`SwapsByPrice{min, max, swap_type, page, limit}`: Fetch all swaps within a given price range

`SwapsByDenom{payment_token, swap_type, page, limit}`: Fetch all swaps for a given denom. Works for both native and cw20 denoms (e.g. ARCH, wARCH, etc.).

`SwapsByPaymentType{cw20, swap_type, page, limit}`: Fetch all swaps by payment type (e.g. either cw20 payments or native ARCH)

## Transactions
`Create{SwapMsg}`: Create a swap
`Finish{SwapMsg}`: Finalize a trade, consuming the swap
`Cancel{CancelMsg}`: Cancel a swap
`Update{UpdateMsg}`: Update a swap

(see `execute.rs` for some additional admin only functions)

## Messages
`SwapMsg`: Message type or creating and finishing swaps
```rs
pub struct SwapMsg {
    pub id: String,
    pub cw721: Addr,
    pub payment_token: Option<Addr>, // Optional cw20 address; if `None` create swap for `aarch`
    pub token_id: String,
    pub expires: Expiration,
    pub price: Uint128,
    pub swap_type: SwapType, // Enum with a value of either 'Sale' or 'Offer'
}
```
`CancelMsg` - Message type for cancelling a swap
```rs
pub struct CancelMsg {
    pub id: String, // ID of swap to be cancelled
}
```
`UpdateMsg` - Message type for updating a swap's price and expiration
```rs
pub struct UpdateMsg {
    pub id: String, // ID of swap to be updated
    pub expires: Expiration, // New expiration (see: https://docs.rs/cw20/latest/cw20/enum.Expiration.html)
    pub price: Uint128, // New swap price (see: https://docs.rs/cosmwasm-std/latest/cosmwasm_std/struct.Uint128.html)
}
```