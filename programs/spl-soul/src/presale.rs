use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use anchor_spl::associated_token::AssociatedToken;
use pyth_sdk_solana::load_price_feed_from_account_info;
use crate::events::*;

declare_id!("PRESALE_ID_PLACEHOLDER"); // Replace with real ID after generation

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum PaymentCurrency {
    SOL,
    USDT,
    USDC,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum PresaleStage {
    NotStarted,
    StageOne,
    StageTwo,
    StageThree,
    Ended,
}

#[program]
pub mod soul_presale {
    use super::*;

    pub fn initialize_presale(
        ctx: Context<InitializePresale>,
        start_time: i64,
    ) -> Result<()> {
        let presale = &mut ctx.accounts.presale_state;
        
        presale.authority = ctx.accounts.authority.key();
        presale.soul_mint = ctx.accounts.soul_mint.key();
        presale.usdt_mint = ctx.accounts.usdt_mint.key();
        presale.usdc_mint = ctx.accounts.usdc_mint.key();
        presale.treasury_wallet = ctx.accounts.treasury_wallet.key();
        
        presale.stage = PresaleStage::NotStarted;
        presale.current_stage_start = start_time;
        presale.current_stage_end = start_time + 30 * 24 * 60 * 60; // 30 days
        
        // 1 billion tokens per stage with 6 decimals
        presale.tokens_per_stage = 1_000_000_000 * 10u64.pow(6);
        presale.tokens_sold_current_stage = 0;
        presale.total_tokens_sold = 0;
        
        // $0.005 = 0.5 cents
        presale.base_price_usd = 50;
        presale.is_paused = false;
        presale.min_purchase_amount = 5000; // $50 in cents
        
        // Initial allocation for each stage
        presale.stage_one_allocation = presale.tokens_per_stage;
        presale.stage_two_allocation = presale.tokens_per_stage;
        presale.stage_three_allocation = presale.tokens_per_stage;

        Ok(())
    }

    pub fn start_presale(ctx: Context<UpdatePresale>) -> Result<()> {
        let presale = &mut ctx.accounts.presale_state;
        require!(presale.stage == PresaleStage::NotStarted, PresaleError::InvalidStage);
        
        let clock = Clock::get()?;
        require!(clock.unix_timestamp >= presale.current_stage_start, PresaleError::TooEarly);
        
        presale.stage = PresaleStage::StageOne;

        // Emit stage opening event
        emit!(StageOpened {
            stage: 1,
            start_time: clock.unix_timestamp,
            end_time: presale.current_stage_end,
            token_allocation: presale.stage_one_allocation,
            price: presale.base_price_usd,
        });

        Ok(())
    }

    pub fn purchase_tokens(
        ctx: Context<PurchaseTokens>,
        currency: PaymentCurrency,
        amount_usd: u64,
    ) -> Result<()> {
        let presale = &mut ctx.accounts.presale_state;
        require!(!presale.is_paused, PresaleError::PresalePaused);
        require!(amount_usd >= presale.min_purchase_amount, PresaleError::BelowMinimum);
        
        // Calculate price based on current stage
        let price_multiplier = match presale.stage {
            PresaleStage::StageOne => 100,
            PresaleStage::StageTwo => 140,
            PresaleStage::StageThree => 196,
            _ => return Err(PresaleError::InvalidStage.into()),
        };
        
        let tokens_to_purchase = (amount_usd as u128 * 10u128.pow(6) * 100)
            .checked_div(presale.base_price_usd as u128 * price_multiplier as u128)
            .ok_or(PresaleError::CalculationError)? as u64;
            
        let available_tokens = match presale.stage {
            PresaleStage::StageOne => presale.stage_one_allocation,
            PresaleStage::StageTwo => presale.stage_two_allocation,
            PresaleStage::StageThree => presale.stage_three_allocation,
            _ => return Err(PresaleError::InvalidStage.into()),
        };
        
        require!(tokens_to_purchase <= available_tokens, PresaleError::InsufficientTokens);
        
        // Process payment based on currency
        match currency {
            PaymentCurrency::USDT => {
                let cpi_accounts = Transfer {
                    from: ctx.accounts.buyer_token_account.to_account_info(),
                    to: ctx.accounts.treasury_token_account.to_account_info(),
                    authority: ctx.accounts.buyer.to_account_info(),
                };
                let cpi_program = ctx.accounts.token_program.to_account_info();
                let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
                token::transfer(cpi_ctx, amount_usd)?;
            },
            PaymentCurrency::USDC => {
                // Same as USDT
                let cpi_accounts = Transfer {
                    from: ctx.accounts.buyer_token_account.to_account_info(),
                    to: ctx.accounts.treasury_token_account.to_account_info(),
                    authority: ctx.accounts.buyer.to_account_info(),
                };
                let cpi_program = ctx.accounts.token_program.to_account_info();
                let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
                token::transfer(cpi_ctx, amount_usd)?;
            },
            PaymentCurrency::SOL => {
                // Requires integration with Oracle for SOL/USD price
                // TODO: Add Pyth Oracle integration
            },
        }
        
        // Update presale state
        match presale.stage {
            PresaleStage::StageOne => presale.stage_one_allocation -= tokens_to_purchase,
            PresaleStage::StageTwo => presale.stage_two_allocation -= tokens_to_purchase,
            PresaleStage::StageThree => presale.stage_three_allocation -= tokens_to_purchase,
            _ => return Err(PresaleError::InvalidStage.into()),
        }
        
        presale.tokens_sold_current_stage += tokens_to_purchase;
        presale.total_tokens_sold += tokens_to_purchase;
        
        // Create or update user info
        let user_info = &mut ctx.accounts.user_info;
        user_info.wallet = ctx.accounts.buyer.key();
        user_info.total_purchased += tokens_to_purchase;
        
        // Mint tokens to buyer
        let cpi_accounts = token::MintTo {
            mint: ctx.accounts.soul_mint.to_account_info(),
            to: ctx.accounts.buyer_soul_token_account.to_account_info(),
            authority: ctx.accounts.presale_state.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let seeds = &[
            b"presale",
            &[*ctx.bumps.get("presale_state").unwrap()],
        ];
        let signer = &[&seeds[..]];
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::mint_to(cpi_ctx, tokens_to_purchase)?;

        // Emit token purchase event
        emit!(TokensPurchased {
            buyer: ctx.accounts.buyer.key(),
            token_amount: tokens_to_purchase,
            cost: amount_usd,
            stage: presale.stage as u8,
        });

        Ok(())
    }

    pub fn advance_stage(ctx: Context<UpdatePresale>) -> Result<()> {
        let presale = &mut ctx.accounts.presale_state;
        let clock = Clock::get()?;
        
        require!(clock.unix_timestamp >= presale.current_stage_end, PresaleError::TooEarly);
        
        // Emit current stage closing event
        emit!(StageClosed {
            stage: presale.stage as u8,
            time_closed: clock.unix_timestamp,
            tokens_sold: presale.tokens_sold_current_stage,
            tokens_left: match presale.stage {
                PresaleStage::StageOne => presale.stage_one_allocation,
                PresaleStage::StageTwo => presale.stage_two_allocation,
                PresaleStage::StageThree => presale.stage_three_allocation,
                _ => 0,
            },
        });
        
        match presale.stage {
            PresaleStage::StageOne => {
                let remaining_tokens = presale.stage_one_allocation;
                presale.stage = PresaleStage::StageTwo;
                // Transfer unsold tokens
                presale.stage_two_allocation += remaining_tokens;
                presale.stage_one_allocation = 0;

                if remaining_tokens > 0 {
                    emit!(TokensCarriedOver {
                        from_stage: 1,
                        to_stage: 2,
                        amount: remaining_tokens,
                    });
                }

                emit!(StageOpened {
                    stage: 2,
                    start_time: clock.unix_timestamp,
                    end_time: clock.unix_timestamp + 30 * 24 * 60 * 60,
                    token_allocation: presale.stage_two_allocation,
                    price: presale.base_price_usd * 140 / 100,
                });
            },
            PresaleStage::StageTwo => {
                let remaining_tokens = presale.stage_two_allocation;
                presale.stage = PresaleStage::StageThree;
                presale.stage_three_allocation += remaining_tokens;
                presale.stage_two_allocation = 0;

                if remaining_tokens > 0 {
                    emit!(TokensCarriedOver {
                        from_stage: 2,
                        to_stage: 3,
                        amount: remaining_tokens,
                    });
                }

                emit!(StageOpened {
                    stage: 3,
                    start_time: clock.unix_timestamp,
                    end_time: clock.unix_timestamp + 30 * 24 * 60 * 60,
                    token_allocation: presale.stage_three_allocation,
                    price: presale.base_price_usd * 196 / 100,
                });
            },
            PresaleStage::StageThree => {
                presale.stage = PresaleStage::Ended;
            },
            _ => return Err(PresaleError::InvalidStage.into()),
        }
        
        presale.current_stage_start = clock.unix_timestamp;
        presale.current_stage_end = presale.current_stage_start + 30 * 24 * 60 * 60;
        presale.tokens_sold_current_stage = 0;
        
        Ok(())
    }

    pub fn burn_remaining_tokens(ctx: Context<BurnTokens>) -> Result<()> {
        let presale = &mut ctx.accounts.presale_state;
        require!(presale.stage == PresaleStage::Ended, PresaleError::InvalidStage);
        
        let remaining_tokens = presale.stage_three_allocation;
        require!(remaining_tokens > 0, PresaleError::NoTokensToBurn);
        
        // Burn remaining tokens
        let cpi_accounts = token::Burn {
            mint: ctx.accounts.soul_mint.to_account_info(),
            from: ctx.accounts.burn_token_account.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::burn(cpi_ctx, remaining_tokens)?;
        
        let clock = Clock::get()?;
        emit!(TokensBurned {
            amount: remaining_tokens,
            burner: ctx.accounts.authority.key(),
            time: clock.unix_timestamp,
        });

        presale.stage_three_allocation = 0;
        Ok(())
    }

    pub fn toggle_pause(ctx: Context<UpdatePresale>) -> Result<()> {
        let presale = &mut ctx.accounts.presale_state;
        presale.is_paused = !presale.is_paused;
        Ok(())
    }

    pub fn withdraw_funds(ctx: Context<WithdrawFunds>, amount: u64) -> Result<()> {
        let cpi_accounts = Transfer {
            from: ctx.accounts.treasury_token_account.to_account_info(),
            to: ctx.accounts.receiver_token_account.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        let clock = Clock::get()?;
        emit!(FundsWithdrawn {
            receiver: ctx.accounts.receiver_token_account.key(),
            amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializePresale<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 32 + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 1 + 8 + 8 + 8 + 8,
        seeds = [b"presale"],
        bump
    )]
    pub presale_state: Account<'info, PresaleState>,
    
    pub soul_mint: Account<'info, Mint>,
    pub usdt_mint: Account<'info, Mint>,
    pub usdc_mint: Account<'info, Mint>,
    #[account(mut)]
    pub treasury_wallet: SystemAccount<'info>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UpdatePresale<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"presale"],
        bump,
        has_one = authority,
    )]
    pub presale_state: Account<'info, PresaleState>,
}

#[derive(Accounts)]
pub struct PurchaseTokens<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"presale"],
        bump
    )]
    pub presale_state: Account<'info, PresaleState>,
    
    #[account(mut)]
    pub soul_mint: Account<'info, Mint>,
    
    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = soul_mint,
        associated_token::authority = buyer
    )]
    pub buyer_soul_token_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub buyer_token_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub treasury_token_account: Account<'info, TokenAccount>,
    
    #[account(
        init_if_needed,
        payer = buyer,
        space = 8 + 32 + 8 + 8 + 8,
        seeds = [b"user_info", buyer.key().as_ref()],
        bump
    )]
    pub user_info: Account<'info, UserPresaleInfo>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct BurnTokens<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"presale"],
        bump,
        has_one = authority,
    )]
    pub presale_state: Account<'info, PresaleState>,
    
    #[account(mut)]
    pub soul_mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub burn_token_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct WithdrawFunds<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"presale"],
        bump,
        has_one = authority,
    )]
    pub presale_state: Account<'info, PresaleState>,
    
    #[account(mut)]
    pub treasury_token_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub receiver_token_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct PresaleState {
    pub authority: Pubkey,
    pub soul_mint: Pubkey,
    pub usdt_mint: Pubkey,
    pub usdc_mint: Pubkey,
    pub treasury_wallet: Pubkey,
    pub stage: PresaleStage,
    pub current_stage_start: i64,
    pub current_stage_end: i64,
    pub tokens_per_stage: u64,
    pub tokens_sold_current_stage: u64,
    pub total_tokens_sold: u64,
    pub base_price_usd: u64,  // In USDT cents ($0.005 = 50)
    pub is_paused: bool,
    pub min_purchase_amount: u64, // In USDT cents ($50 = 5000)
    pub stage_one_allocation: u64,
    pub stage_two_allocation: u64,
    pub stage_three_allocation: u64,
}

#[account]
pub struct UserPresaleInfo {
    pub wallet: Pubkey,
    pub total_purchased: u64,
    pub tokens_claimed: u64,
    pub last_claim_time: i64,
}

#[error_code]
pub enum PresaleError {
    #[msg("Invalid presale stage")]
    InvalidStage,
    #[msg("Presale is paused")]
    PresalePaused,
    #[msg("Purchase amount below minimum")]
    BelowMinimum,
    #[msg("Insufficient tokens available")]
    InsufficientTokens,
    #[msg("Too early for this operation")]
    TooEarly,
    #[msg("Calculation error")]
    CalculationError,
    #[msg("No tokens to burn")]
    NoTokensToBurn,
} 