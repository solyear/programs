
use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_instruction;

declare_id!("GZaouCEwsbDEvgjE6kTsE4q4dQSFUyVUfVSKBRAjhc4m");

#[program]
pub mod sequence_tracker {
    use super::*;

    /// Initializes the program with a maximum sequence limit and price per sequence.
    pub fn initialize(ctx: Context<Initialize>, max_sequence: u64, price: u64) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        global_state.max_sequence = max_sequence;
        global_state.admin = *ctx.accounts.admin.key;
        global_state.fee_account = *ctx.accounts.fee_account.key;
        global_state.next_sequence = 0; // Start from 0
        global_state.price = price;
        Ok(())
    }
    /// Allows a user to buy the next available sequence up to the specified end range.
    pub fn buy_sequence(ctx: Context<BuySequence>, end: u64) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;

        // Ensure the admin matches the expected admin in the global state
        require!(
            *ctx.accounts.fee_account.key == global_state.fee_account,
            CustomError::InvalidFeeAccount
        );

        // Check that the referrer is not the same as the buyer
        require!(
            *ctx.accounts.referrer.key != ctx.accounts.buyer.key(),
            CustomError::InvalidReferrer
        );

        // Check validity of the requested range
        require!(end > 0, CustomError::InvalidInterval);
        let start = global_state.next_sequence;
        let calculated_end = start + end - 1;
        require!(
            calculated_end < global_state.max_sequence,
            CustomError::ExceedsMaxSequence
        );

        // Calculate the total price
        let total_price = (end) * global_state.price;

        // Transfer SOL from buyer to admin
        let ix = system_instruction::transfer(
            &ctx.accounts.buyer.key,
            &ctx.accounts.fee_account.key,
            total_price,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.fee_account.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        // Update the global state to reflect the next available sequence
        global_state.next_sequence = calculated_end + 1;

        // Add the new sequence interval to the user's account
        let user_sequence = &mut ctx.accounts.user_sequence;
        let new_interval = SequenceInterval {
            start,
            end: calculated_end,
        };
        if user_sequence.owner == ctx.accounts.buyer.key() {
            user_sequence.intervals.push(new_interval);
        } else {
            // First purchase for this user
            user_sequence.owner = *ctx.accounts.buyer.key;
            user_sequence.referrals = 0;
            user_sequence.total_referrals_given = 0;
            user_sequence.global_state = *ctx.accounts.global_state.to_account_info().key;
            user_sequence.intervals = vec![new_interval];
        }

        // Add referral to referred user sequence
        let ref_user_sequence = &mut ctx.accounts.ref_user_sequence;

        if ref_user_sequence.owner == ctx.accounts.referrer.key() {
            ref_user_sequence.referrals = ref_user_sequence.referrals + end;
            ref_user_sequence.total_referrals_given = ref_user_sequence.total_referrals_given + end;

        } else {
            // First purchase for this user
            ref_user_sequence.owner = *ctx.accounts.referrer.key;
            ref_user_sequence.referrals = end;
            ref_user_sequence.total_referrals_given = end;
            ref_user_sequence.global_state = *ctx.accounts.global_state.to_account_info().key;
            ref_user_sequence.intervals = vec![];
        }

        Ok(())
    }
    /// Allows admin to buy for reciever the next available sequence up to the specified end range.
    pub fn buy_sequence_admin(ctx: Context<BuySequenceAdmin>, end: u64) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;

        // Ensure the admin matches the expected admin in the global state
        require!(
            *ctx.accounts.buyer.key == global_state.admin,
            CustomError::InvalidAdmin
        );

        // Check validity of the requested range
        require!(end > 0, CustomError::InvalidInterval);
        let start = global_state.next_sequence;
        let calculated_end = start + end - 1;
        require!(
            calculated_end < global_state.max_sequence,
            CustomError::ExceedsMaxSequence
        );

        // Update the global state to reflect the next available sequence
        global_state.next_sequence = calculated_end + 1;

        // Add the new sequence interval to the user's account
        let user_sequence = &mut ctx.accounts.user_sequence;
        let new_interval = SequenceInterval {
            start,
            end: calculated_end,
        };
        if user_sequence.owner == ctx.accounts.reciever.key() {
            user_sequence.intervals.push(new_interval);
        } else {
            // First purchase for this user
            user_sequence.owner = *ctx.accounts.reciever.key;
            user_sequence.global_state = *ctx.accounts.global_state.to_account_info().key;
            user_sequence.intervals = vec![new_interval];
            user_sequence.referrals = 0;
            user_sequence.total_referrals_given = 0;

        }

        Ok(())
    }


/// Allows user to claim reward according to referral count
pub fn claim_referral_reward(ctx: Context<ClaimReferralReward>) -> Result<()> {
    let global_state = &mut ctx.accounts.global_state;
    let user_sequence = &mut ctx.accounts.user_sequence;

    // Ensure the admin matches the expected admin in the global state
    require!(
        *ctx.accounts.buyer.key == user_sequence.owner,
        CustomError::InvalidOwner
    );

    require!(
        user_sequence.referrals >= 10,
        CustomError::InvalidRefCount
    );

    // Calculate reward interval based on referral count
    let reward_intervals = user_sequence.referrals / 10;
    let remaining_referrals = user_sequence.referrals % 10;

    let start = global_state.next_sequence;
    let end = start + reward_intervals - 1;
    global_state.next_sequence = end + 1;

    require!(
        end < global_state.max_sequence,
        CustomError::ExceedsMaxSequence
    );


    let new_interval = SequenceInterval { start, end };
    user_sequence.intervals.push(new_interval);
    // Update referral count for the user
    user_sequence.referrals = remaining_referrals;

    Ok(())
}


}

/// Accounts for initializing the global state
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + 8 + 32 + 32 + 8 + 8, // Anchor's discriminator + struct fields
        owner = crate::ID
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub admin: AccountInfo<'info>,
    #[account(mut)]
    pub fee_account: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}


/// Accounts for buying a sequence
#[derive(Accounts)]
pub struct BuySequence<'info> {
    #[account(
        mut,
        constraint = global_state.max_sequence > global_state.next_sequence,
        owner = crate::ID // Ensure the account is owned by the program
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        init_if_needed,
        payer = buyer,
        seeds = [b"user_sequence", buyer.key().as_ref(), global_state.key().as_ref()],
        bump,
        space = 8 + 32 + 32 + 4 + (16 * 20) + 8 + 8, // Anchor's discriminator + fields
        owner = crate::ID // Ensure the account is owned by the program
    )]
    pub user_sequence: Account<'info, UserSequence>,
    #[account(
        init_if_needed,
        payer = buyer,
        seeds = [b"user_sequence", referrer.key().as_ref(), global_state.key().as_ref()],
        bump,
        space = 8 + 32 + 32 + 4 + (16 * 20) + 8 + 8, // Anchor's discriminator + fields
        owner = crate::ID // Ensure the account is owned by the program
    )]
    pub ref_user_sequence: Account<'info, UserSequence>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(mut)]
    pub fee_account: AccountInfo<'info>,
    #[account(mut)]
    pub referrer: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for buying a sequence for admin
#[derive(Accounts)]
pub struct BuySequenceAdmin<'info> {
    #[account(
        mut,
        constraint = global_state.max_sequence > global_state.next_sequence,
        owner = crate::ID // Ensure the account is owned by the program
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        init_if_needed,
        payer = buyer,
        seeds = [b"user_sequence", reciever.key().as_ref(), global_state.key().as_ref()],
        bump,
        space = 8 + 32 + 32 + 4 + (16 * 20) + 8 + 8, // Anchor's discriminator + fields
        owner = crate::ID // Ensure the account is owned by the program
    )]
    pub user_sequence: Account<'info, UserSequence>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(mut)]
    pub reciever: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for claiming reward
#[derive(Accounts)]
pub struct ClaimReferralReward<'info> {
    #[account(
        mut,
        constraint = global_state.max_sequence > global_state.next_sequence,
        owner = crate::ID // Ensure the account is owned by the program
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        init_if_needed,
        payer = buyer,
        seeds = [b"user_sequence", buyer.key().as_ref(), global_state.key().as_ref()],
        bump,
        space = 8 + 32 + 32 + 4 + (16 * 20) + 8 + 8, // Anchor's discriminator + fields
        owner = crate::ID // Ensure the account is owned by the program
    )]
    pub user_sequence: Account<'info, UserSequence>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    pub system_program: Program<'info, System>,
}


/// Global state of the program
#[account]
pub struct GlobalState {
    pub max_sequence: u64,  // Maximum allowable sequence
    pub admin: Pubkey,      // Admin of the program
    pub fee_account: Pubkey, // Fee account of the program
    pub next_sequence: u64, // Next available sequence
    pub price: u64,         // Price per sequence
}

/// User-specific sequence data
#[account]
pub struct UserSequence {
    pub owner: Pubkey,                    // Owner of the sequence
    pub global_state: Pubkey,             // Reference to the global state
    pub intervals: Vec<SequenceInterval>, // List of assigned intervals
    pub referrals: u64,                   // Total refs of account
    pub total_referrals_given: u64,       // Total referrals given by this user

}

/// A sequence interval
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SequenceInterval {
    pub start: u64,
    pub end: u64,
}

/// Custom errors for the program
#[error_code]
pub enum CustomError {
    #[msg("The requested interval is invalid.")]
    InvalidInterval,
    #[msg("The requested interval exceeds the maximum sequence.")]
    ExceedsMaxSequence,
    #[msg("The provided fee account is invalid.")]
    InvalidFeeAccount,
    #[msg("The provided admin is invalid.")]
    InvalidAdmin,
    #[msg("The referrer cannot be the same as the buyer.")]
    InvalidReferrer,
    #[msg("The claimer is not sequence owner")]
    InvalidOwner,
    #[msg("The claimer needs at least 10 referral")]
    InvalidRefCount,
}
