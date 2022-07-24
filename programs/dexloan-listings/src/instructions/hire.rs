use anchor_lang::{prelude::*};
use anchor_spl::token::{Mint, Token, TokenAccount};
use crate::state::{Hire, HireState};
use crate::error::{DexloanError};
use crate::utils::{pay_creator_fees, freeze, thaw, FreezeParams};

pub fn init(
  ctx: Context<InitHire>,
  amount: u64,
  expiry: i64,
  borrower: Option<Pubkey>,
) -> Result<()> {
  let hire = &mut ctx.accounts.hire_account;
  let unix_timestamp = ctx.accounts.clock.unix_timestamp;
  
  msg!("unix_timestamp: {} seconds", unix_timestamp);
  msg!("expiry: {} seconds", expiry);
  
  if unix_timestamp > expiry {
      return Err(DexloanError::InvalidExpiry.into())
  }

  // Init
  hire.lender = ctx.accounts.lender.key();
  hire.mint = ctx.accounts.mint.key();
  hire.bump = *ctx.bumps.get("hire_account").unwrap();
  //
  hire.amount = amount;
  hire.expiry = expiry;
  hire.state = HireState::Listed;

  if borrower.is_some() {
    hire.borrower = borrower.unwrap();
  }

  // Delegate authority
  anchor_spl::token::approve(
      CpiContext::new(
          ctx.accounts.token_program.to_account_info(),
          anchor_spl::token::Approve {
              to: ctx.accounts.deposit_token_account.to_account_info(),
              delegate: hire.to_account_info(),
              authority: ctx.accounts.lender.to_account_info(),
          }
      ),
      1
  )?;

  let signer_bump = &[hire.bump];
  let signer_seeds = &[&[
      Hire::PREFIX,
      hire.mint.as_ref(),
      hire.lender.as_ref(),
      signer_bump
  ][..]];

  freeze(
      FreezeParams {
          delegate: hire.to_account_info(),
          token_account: ctx.accounts.deposit_token_account.to_account_info(),
          edition: ctx.accounts.edition.to_account_info(),
          mint: ctx.accounts.mint.to_account_info(),
          signer_seeds: signer_seeds
      }
  )?;

  Ok(())
}

pub fn take<'info>(ctx: Context<'_, '_, '_, 'info, TakeHire<'info>>) -> Result<()> {
    let hire = &mut ctx.accounts.hire_account;

    hire.state = HireState::Hired;
    
    let signer_bump = &[hire.bump];
    let signer_seeds = &[&[
        Hire::PREFIX,
        hire.mint.as_ref(),
        hire.lender.as_ref(),
        signer_bump
    ][..]];

    let remaining_amount = pay_creator_fees(
        &mut ctx.remaining_accounts.iter(),
        hire.amount,
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.metadata.to_account_info(),
        &ctx.accounts.lender.to_account_info(),
        &ctx.accounts.deposit_token_account,
    )?;

    // Transfer fee
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::system_instruction::transfer(
            &hire.borrower,
            &hire.lender,
            remaining_amount,
        ),
        &[
            ctx.accounts.borrower.to_account_info(),
            ctx.accounts.lender.to_account_info(),
        ]
    )?;

    // Thaw & Transfer NFT to hire account
    thaw(
        FreezeParams {
            delegate: hire.to_account_info(),
            token_account: ctx.accounts.deposit_token_account.to_account_info(),
            edition: ctx.accounts.edition.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            signer_seeds: signer_seeds
        }
    )?;
    anchor_spl::token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.deposit_token_account.to_account_info(),
                to: ctx.accounts.hire_token_account.to_account_info(),
                authority: hire.to_account_info(),
            },
            signer_seeds
        ),
        1
    )?;

    // Delegate authority & freeze hire token account
    anchor_spl::token::approve(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Approve {
                to: ctx.accounts.hire_token_account.to_account_info(),
                delegate: hire.to_account_info(),
                authority: ctx.accounts.lender.to_account_info(),
            }
        ),
        1
    )?;

    freeze(
        FreezeParams {
            delegate: hire.to_account_info(),
            token_account: ctx.accounts.hire_token_account.to_account_info(),
            edition: ctx.accounts.edition.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            signer_seeds: signer_seeds
        }
    )?;

    Ok(())
}

pub fn revoke(ctx: Context<RevokeHire>) -> Result<()> {
    let hire = &mut ctx.accounts.hire_account;
    let unix_timestamp = ctx.accounts.clock.unix_timestamp;

    if hire.expiry > unix_timestamp {
        return Err(DexloanError::NotExpired.into());
    }

    let signer_bump = &[hire.bump];
    let signer_seeds = &[&[
        Hire::PREFIX,
        hire.mint.as_ref(),
        hire.lender.as_ref(),
        signer_bump
    ][..]];

    // Thaw & Transfer NFT back to deposit account
    thaw(
        FreezeParams {
            delegate: hire.to_account_info(),
            token_account: ctx.accounts.deposit_token_account.to_account_info(),
            edition: ctx.accounts.edition.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            signer_seeds: signer_seeds
        }
    )?;
    anchor_spl::token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.deposit_token_account.to_account_info(),
                to: ctx.accounts.hire_token_account.to_account_info(),
                authority: hire.to_account_info(),
            },
            signer_seeds
        ),
        1
    )?;

    // Revoke delegation on deposit account
    anchor_spl::token::revoke(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Revoke {
                source: ctx.accounts.deposit_token_account.to_account_info(),
                authority: ctx.accounts.lender.to_account_info(),
            }
        )
    )?;

    Ok(())
}

pub fn close(ctx: Context<CloseHire>) -> Result<()> {
    let hire_account = &mut ctx.accounts.hire_account;
    let unix_timestamp = ctx.accounts.clock.unix_timestamp;

    if hire_account.expiry > unix_timestamp {
        return Err(DexloanError::NotExpired.into());
    }

    Ok(())
}

#[derive(Accounts)]
#[instruction(amount: u64, expiry: i64, borrower: Option<Pubkey>)]
pub struct InitHire<'info> {
  #[account(mut)]
  pub lender: Signer<'info>,
  #[account(
      mut,
      constraint = deposit_token_account.amount == 1,
      constraint = deposit_token_account.owner == lender.key(),
      associated_token::mint = mint,
      associated_token::authority = lender,
  )]
  pub deposit_token_account: Account<'info, TokenAccount>,
  #[account(
      init,
      payer = lender,
      seeds = [
        Hire::PREFIX,
        mint.key().as_ref(),
        lender.key().as_ref(),
      ],
      space = Hire::space(),
      bump,
  )]
  pub hire_account: Account<'info, Hire>,    
  #[account(constraint = mint.supply == 1)]
  pub mint: Account<'info, Mint>,
  /// CHECK: validated in cpi
  pub edition: UncheckedAccount<'info>,
  /// CHECK: validated in cpi
  pub metadata_program: UncheckedAccount<'info>, 
  /// Misc
  pub system_program: Program<'info, System>,
  pub token_program: Program<'info, Token>,
  pub clock: Sysvar<'info, Clock>,
  pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct TakeHire<'info> {
    #[account(mut)]
    /// CHECK: validated seeds constraints
    pub lender: AccountInfo<'info>,
    #[account(mut)]
    pub borrower: Signer<'info>,
    #[account(mut)]
    pub deposit_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub hire_token_account: Account<'info, TokenAccount>,
    #[account(
        seeds = [
          Hire::PREFIX,
          mint.key().as_ref(),
          lender.key().as_ref(),
        ],
        bump,
    )]
    pub hire_account: Account<'info, Hire>,    
    #[account(constraint = mint.supply == 1)]
    pub mint: Account<'info, Mint>,
    /// CHECK: validated in cpi
    pub edition: UncheckedAccount<'info>,
    /// CHECK: deserialized and checked
    pub metadata: UncheckedAccount<'info>,
    /// CHECK: validated in cpi
    pub metadata_program: UncheckedAccount<'info>, 
    /// Misc
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct RevokeHire<'info> {
    #[account(mut)]
    pub lender: Signer<'info>,
    #[account(mut)]
    /// CHECK: validated in constraints
    pub borrower: AccountInfo<'info>,
    #[account(mut)]
    pub deposit_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub hire_token_account: Account<'info, TokenAccount>,
    #[account(
        seeds = [
          Hire::PREFIX,
          mint.key().as_ref(),
          lender.key().as_ref(),
        ],
        bump,
        constraint = hire_account.state == HireState::Hired,
        constraint = hire_account.borrower == borrower.key(),
    )]
    pub hire_account: Account<'info, Hire>,    
    #[account(constraint = mint.supply == 1)]
    pub mint: Account<'info, Mint>,
    /// CHECK: validated in cpi
    pub edition: UncheckedAccount<'info>,
    /// CHECK: validated in cpi
    pub metadata_program: UncheckedAccount<'info>, 
    /// Misc
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct CloseHire<'info> {
    /// CHECK: contrained on listing_account
    #[account(mut)]
    pub lender: Signer<'info>,
    /// The listing the loan is being issued against
    #[account(
        mut,
        seeds = [
            Hire::PREFIX,
            mint.key().as_ref(),
            lender.key().as_ref(),
        ],
        bump,
        close = lender,
        constraint = hire_account.state != HireState::Hired
    )]
    pub hire_account: Account<'info, Hire>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = lender
    )]
    pub deposit_token_account: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    /// CHECK: validated in cpi
    pub edition: UncheckedAccount<'info>,
    /// CHECK: validated in cpi
    pub metadata_program: UncheckedAccount<'info>, 
    /// Misc
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub clock: Sysvar<'info, Clock>,
}