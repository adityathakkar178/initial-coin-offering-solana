use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct ICOAccount {
    pub total_supply: u64,
    pub admin: Pubkey,
    pub balance: Vec<(Pubkey, u64)>,
    pub pre_sale_price: u64,
    pub pre_sale_limit: u64,
    pub sale_price: u64,
    pub sale_limit: u64,
    pub sale_start_time: u64,
    pub sale_end_time: u64,
    pub total_price_earned: u64,
    pub pre_sale_account: Vec<PreSaleAccount>,
    pub sale_account: Vec<SaleAccount>,
}

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct PreSaleAccount {
    pub address: Pubkey,
    pub token_amount: u64,
    pub token_price: u64,
    pub whitelist_account: bool,
}

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct SaleAccount {
    pub address: Pubkey,
    pub token_amount: u64,
    pub token_price: u64,
}

impl PreSaleAccount {
    pub fn whitelist(&mut self) {
        self.whitelist_account = !self.whitelist_account;
    }
}

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("ICO Program Enter Point");

    let account_iter = &mut accounts.iter();
    let ico_accounts = next_account_info(account_iter)?;

    if ico_accounts.owner != program_id {
        msg!("ICO account does not have correct program id");
        return Err(ProgramError::IncorrectProgramId);
    }

    let mut ico_state = ICOAccount::try_from_slice(&ico_accounts.data.borrow())?;

    match instruction_data[0] {
        0 => {
            intialize_ico(program_id, &mut ico_state, account_iter);
        }
        1 => {
            let recipient_account_info = next_account_info(account_iter)?;
            let amount_bytes = instruction_data[1..9].try_into().unwrap();
            let amount = u64::from_le_bytes(amount_bytes);
            mint_tokens(&mut ico_state, &recipient_account_info.key, amount)?;
        }
        2 => {
            pre_sale(&mut ico_state, accounts)?;
        }
        3 => {
            sale(&mut ico_state, accounts)?;
        }
        4 => {
            let account_to_whitelist_info = next_account_info(account_iter)?;
            whitelist_account(&mut ico_state, &account_to_whitelist_info.key)?;
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }

    ico_state.serialize(&mut &mut ico_accounts.data.borrow_mut()[..])?;

    Ok(())
}

pub fn intialize_ico(
    program_id: &Pubkey,
    ico_state: &mut ICOAccount,
    account_iter: &mut std::slice::Iter<'_, AccountInfo>,
) -> ProgramResult {
    let admin_account = next_account_info(account_iter)?;

    if admin_account.key != program_id {
        msg!("Caller is not the admin");
        return Err(ProgramError::InvalidAccountData);
    }

    ico_state.admin = *admin_account.key;
    ico_state.total_supply = 10000;
    ico_state.pre_sale_price = 100;
    ico_state.pre_sale_limit = 50;
    ico_state.sale_price = 200;
    ico_state.sale_limit = 100;
    ico_state.sale_start_time = 0;
    ico_state.sale_end_time = 100;
    ico_state
        .balance
        .push((*admin_account.key, ico_state.total_supply));
    Ok(())
}

pub fn mint_tokens(
    ico_state: &mut ICOAccount,
    recipient_accounts: &Pubkey,
    amount: u64,
) -> ProgramResult {
    if let Some((_, balance)) = ico_state
        .balance
        .iter_mut()
        .find(|(account, _)| *account == *recipient_accounts)
    {
        *balance += amount;
        return Ok(());
    }
    ico_state.balance.push((*recipient_accounts, amount));

    Ok(())
}

pub fn whitelist_account(
    ico_state: &mut ICOAccount,
    account_to_whitelist: &Pubkey,
) -> ProgramResult {
    for pre_sale_account in &mut ico_state.pre_sale_account {
        if &pre_sale_account.address == account_to_whitelist {
            pre_sale_account.whitelist();
            return Ok(());
        }
    }
    Err(ProgramError::InvalidAccountData)
}

pub fn pre_sale(ico_state: &mut ICOAccount, accounts: &[AccountInfo]) -> ProgramResult {
    let acount_iter: &mut std::slice::Iter<'_, AccountInfo<'_>> = &mut accounts.iter();
    let buyer_account = next_account_info(acount_iter)?;
    let current_time = Clock::get()?.unix_timestamp as u64;

    if current_time > ico_state.sale_start_time {
        return Err(ProgramError::InvalidInstructionData);
    }

    let mut is_whiltelisted = false;
    let buyer_account_info = buyer_account.key;
    for pre_sale_account in &ico_state.pre_sale_account {
        if &pre_sale_account.address == buyer_account_info && pre_sale_account.whitelist_account {
            is_whiltelisted = true;
            break;
        }
    }

    if !is_whiltelisted {
        return Err(ProgramError::InvalidAccountData);
    }

    let amount_bytes = &buyer_account.data.borrow()[..8];
    let amount = u64::from_le_bytes(amount_bytes.try_into().unwrap());
    let total_cost = amount * ico_state.pre_sale_price;

    if total_cost != buyer_account.lamports() {
        return Err(ProgramError::InvalidAccountData);
    }

    for pre_sale_account in &mut ico_state.pre_sale_account {
        if &pre_sale_account.address == buyer_account_info {
            pre_sale_account.token_amount += amount;
        }
    }

    **buyer_account.try_borrow_mut_lamports()? -= total_cost;

    if let Some((_, buyer_balance)) = ico_state
        .balance
        .iter_mut()
        .find(|(account, _)| *account == *buyer_account.key)
    {
        *buyer_balance += amount;
    } else {
        return Err(ProgramError::InvalidAccountData);
    }

    if let Some((_, admin_balance)) = ico_state
        .balance
        .iter_mut()
        .find(|(account, _)| *account == ico_state.admin)
    {
        *admin_balance -= amount;
    } else {
        return Err(ProgramError::InvalidAccountData);
    }

    ico_state.total_price_earned += total_cost;

    Ok(())
}

pub fn sale(ico_state: &mut ICOAccount, accounts: &[AccountInfo]) -> ProgramResult {
    let acount_iter: &mut std::slice::Iter<'_, AccountInfo<'_>> = &mut accounts.iter();
    let buyer_account = next_account_info(acount_iter)?;
    let buyer_account_info = buyer_account.key;
    let current_time = Clock::get()?.unix_timestamp as u64;

    if current_time < ico_state.sale_start_time
        || ico_state.sale_start_time >= ico_state.sale_end_time
    {
        return Err(ProgramError::InvalidInstructionData);
    }

    let amount_bytes = &buyer_account.data.borrow()[..8];
    let amount = u64::from_le_bytes(amount_bytes.try_into().unwrap());
    let total_cost = amount * ico_state.sale_price;

    if total_cost != buyer_account.lamports() {
        return Err(ProgramError::InvalidAccountData);
    }

    for sale_account in &mut ico_state.sale_account {
        if &sale_account.address == buyer_account_info {
            sale_account.token_amount += amount;
        }
    }

    **buyer_account.try_borrow_mut_lamports()? -= total_cost;

    if let Some((_, buyer_balance)) = ico_state
        .balance
        .iter_mut()
        .find(|(account, _)| *account == *buyer_account.key)
    {
        *buyer_balance += amount;
    } else {
        return Err(ProgramError::InvalidAccountData);
    }

    if let Some((_, admin_balance)) = ico_state
        .balance
        .iter_mut()
        .find(|(account, _)| *account == ico_state.admin)
    {
        *admin_balance -= amount;
    } else {
        return Err(ProgramError::InvalidAccountData);
    }

    ico_state.total_price_earned += total_cost;

    Ok(())
}
