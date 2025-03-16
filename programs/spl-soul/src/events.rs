use anchor_lang::prelude::*;

#[event]
pub struct TokensPurchased {
    #[index]
    pub buyer: Pubkey,
    pub token_amount: u64,
    pub cost: u64,
    pub stage: u8,
}

#[event]
pub struct StageOpened {
    #[index]
    pub stage: u8,
    pub start_time: i64,
    pub end_time: i64,
    pub token_allocation: u64,
    pub price: u64,
}

#[event]
pub struct StageClosed {
    #[index]
    pub stage: u8,
    pub time_closed: i64,
    pub tokens_sold: u64,
    pub tokens_left: u64,
}

#[event]
pub struct TokensCarriedOver {
    #[index]
    pub from_stage: u8,
    #[index]
    pub to_stage: u8,
    pub amount: u64,
}

#[event]
pub struct TokensBurned {
    pub amount: u64,
    #[index]
    pub burner: Pubkey,
    pub time: i64,
}

#[event]
pub struct FundsWithdrawn {
    #[index]
    pub receiver: Pubkey,
    pub amount: u64,
    pub time: i64,
} 