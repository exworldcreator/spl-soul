use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

pub mod presale;
pub mod events;

use presale::*;
use events::*;

// Replace this key with your real Base58 key generated via `anchor keys gen`
declare_id!("G1RZSqt72nyisqmEaocAMV42fKwepARaAo17JtL1rGoW");

#[program]
mod soul_token {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, total_supply: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.total_supply = total_supply * 10u64.pow(6); // Account for 6 decimals
        state.authority = ctx.accounts.authority.key();

        state.team_supply = state.total_supply * 10 / 100; // 10%
        state.dex_liquidity_supply = state.total_supply * 5 / 100; // 5%
        state.cex_marketing_supply = state.total_supply * 10 / 100; // 10%
        state.cex_marketing_unlocked = state.cex_marketing_supply; // Available at TGE
        state.development_supply = state.total_supply * 30 / 100; // 30%
        state.development_unlocked = state.development_supply / 6; // 5% at TGE
        state.community_supply = state.total_supply * 15 / 100; // 15% (1.5B SOUL)
        state.community_unlocked = state.community_supply * 30 / 100; // 30% at TGE

        state.tge_time = Clock::get()?.unix_timestamp;

        // The remaining 30% (Pre Sale) will be managed by a separate contract
        Ok(())
    }

    pub fn unlock_team_tokens(ctx: Context<UnlockTokens>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        let current_time = Clock::get()?.unix_timestamp;
        let days_since_tge = (current_time - state.tge_time) as u64 / (24 * 60 * 60); // Приводим к u64

        let unlock_interval = 180u64; // 6 месяцев
        let unlock_amount_per_period = state.team_supply / 4; // 25% каждые 6 месяцев
        let periods_passed = days_since_tge / unlock_interval;
        let total_unlocked = unlock_amount_per_period * periods_passed.min(4u64); // Используем u64 для min

        require!(total_unlocked > state.team_unlocked, TokenError::NoTokensToUnlock);

        let amount_to_unlock = total_unlocked - state.team_unlocked;
        let cpi_accounts = token::MintTo {
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.token_account.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::mint_to(cpi_ctx, amount_to_unlock)?;

        state.team_unlocked += amount_to_unlock;
        Ok(())
    }

    pub fn unlock_development_tokens(ctx: Context<UnlockTokens>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        let current_time = Clock::get()?.unix_timestamp;
        let days_since_tge = (current_time - state.tge_time) as u64 / (24 * 60 * 60); // Приводим к u64

        let unlock_interval = 180u64; // 180 дней
        let unlock_amount_per_period = state.development_supply / 6; // 500 млн за период
        let periods_passed = (days_since_tge / unlock_interval + 1).min(6u64); // Включаем TGE, используем u64
        let total_unlocked = unlock_amount_per_period * periods_passed;

        require!(total_unlocked > state.development_unlocked, TokenError::NoTokensToUnlock);

        let amount_to_unlock = total_unlocked - state.development_unlocked;
        let cpi_accounts = token::MintTo {
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.token_account.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::mint_to(cpi_ctx, amount_to_unlock)?;

        state.development_unlocked += amount_to_unlock;
        Ok(())
    }

    pub fn unlock_community_tokens(ctx: Context<UnlockTokens>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        let current_time = Clock::get()?.unix_timestamp;
        let days_since_tge = (current_time - state.tge_time) as u64 / (24 * 60 * 60); // Приводим к u64

        let unlock_interval = 60u64; // 60 дней
        let periods_passed = (days_since_tge / unlock_interval + 1).min(5u64); // Включаем TGE, макс 5
        let total_unlocked = if periods_passed == 1 {
            state.community_supply * 30 / 100 // 300 млн на TGE
        } else {
            let base_unlocked = state.community_supply * 30 / 100;
            let additional_unlocked = (periods_passed - 1) * 200_000_000u64 * 10u64.pow(6);
            base_unlocked.checked_add(additional_unlocked).unwrap_or(state.community_supply)
        }.min(state.community_supply);

        require!(total_unlocked > state.community_unlocked, TokenError::NoTokensToUnlock);

        let amount_to_unlock = total_unlocked - state.community_unlocked; // Исправлено с team_unlocked на community_unlocked
        let cpi_accounts = token::MintTo {
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.token_account.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::mint_to(cpi_ctx, amount_to_unlock)?;

        state.community_unlocked += amount_to_unlock;
        Ok(())
    }

    pub fn add_dex_liquidity(ctx: Context<UnlockTokens>, amount: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;
        require!(amount <= state.dex_liquidity_supply, TokenError::ExceedsDexLiquidityLimit);

        let cpi_accounts = token::MintTo {
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.token_account.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::mint_to(cpi_ctx, amount)?;

        state.dex_liquidity_supply -= amount;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = payer,
        mint::decimals = 6,
        mint::authority = authority,
    )]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    #[account(
        init,
        payer = payer,
        space = 8 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8,
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, TokenState>,
}

#[derive(Accounts)]
pub struct UnlockTokens<'info> {
    #[account(mut)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    pub state: Account<'info, TokenState>,
}

#[account]
pub struct TokenState {
    pub authority: Pubkey,
    pub total_supply: u64,
    pub team_supply: u64,
    pub team_unlocked: u64,
    pub dex_liquidity_supply: u64,
    pub cex_marketing_supply: u64,
    pub cex_marketing_unlocked: u64,
    pub development_supply: u64,
    pub development_unlocked: u64,
    pub community_supply: u64,
    pub community_unlocked: u64,
    pub tge_time: i64,
}

#[error_code]
pub enum TokenError {
    #[msg("No tokens available to unlock")]
    NoTokensToUnlock,
    #[msg("Exceeds DEX liquidity limit")]
    ExceedsDexLiquidityLimit,
}

