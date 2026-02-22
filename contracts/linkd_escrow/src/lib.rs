#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, String};

// Thresholds for Time-To-Live (state rent)
const DAY_IN_LEDGERS: u32 = 17280; // Assuming ~5s per ledger
const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS; // 30 days
const INSTANCE_LIFETIME_THRESHOLD: u32 = 15 * DAY_IN_LEDGERS; // Bump if less than 15 days remain

#[derive(Clone)]
#[contracttype]
pub enum MilestoneStatus {
    Pending,
    AwaitingProof,
    UnderReview,
    Verified,
    Rejected,
}

#[derive(Clone)]
#[contracttype]
pub struct Milestone {
    pub id: u32,
    pub target_amount: i128,
    pub status: MilestoneStatus,
    pub proof_hash: String, // Fixed: Changed from Symbol to String
    pub ngo_approved: bool,
    pub auditor_approved: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    NGO,
    Auditor,
    Beneficiary,
    TokenAddress,
    TotalEscrowed,
    MilestoneCount,
    Milestone(u32), // Fixed: Isolated persistent key for infinite scaling
}

#[contract]
pub struct LinkdEscrow;

#[contractimpl]
impl LinkdEscrow {
    /// Initialize the escrow contract with roles and token
    pub fn initialize(
        env: Env,
        admin: Address,
        ngo: Address,
        auditor: Address,
        beneficiary: Address,
        token_address: Address,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Contract already initialized");
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NGO, &ngo);
        env.storage().instance().set(&DataKey::Auditor, &auditor);
        env.storage().instance().set(&DataKey::Beneficiary, &beneficiary);
        env.storage().instance().set(&DataKey::TokenAddress, &token_address);
        env.storage().instance().set(&DataKey::TotalEscrowed, &0i128);
        env.storage().instance().set(&DataKey::MilestoneCount, &0u32);

        Self::extend_instance_ttl(&env);
    }

    /// Add a new milestone
    pub fn add_milestone(env: Env, target_amount: i128) -> u32 {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth(); // Fixed: Native auth, removed redundant caller param

        let milestone_id: u32 = env.storage().instance().get(&DataKey::MilestoneCount).unwrap_or(0);

        let milestone = Milestone {
            id: milestone_id,
            target_amount,
            status: MilestoneStatus::Pending,
            proof_hash: String::from_str(&env, ""), 
            ngo_approved: false,
            auditor_approved: false,
        };

        // Fixed: Use persistent storage to avoid 64KB instance trap
        env.storage().persistent().set(&DataKey::Milestone(milestone_id), &milestone);
        
        let new_count = milestone_id + 1;
        env.storage().instance().set(&DataKey::MilestoneCount, &new_count);

        Self::extend_instance_ttl(&env);
        // Bump the persistent data for the new milestone
        env.storage().persistent().extend_ttl(&DataKey::Milestone(milestone_id), INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        milestone_id
    }

    /// Deposit funds into escrow
    pub fn deposit(env: Env, from: Address, amount: i128) {
        from.require_auth();

        let token_address: Address = env.storage().instance().get(&DataKey::TokenAddress).unwrap();
        let contract_address = env.current_contract_address();

        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(&from, &contract_address, &amount);

        let mut total: i128 = env.storage().instance().get(&DataKey::TotalEscrowed).unwrap_or(0);
        total += amount;
        env.storage().instance().set(&DataKey::TotalEscrowed, &total);

        Self::extend_instance_ttl(&env);
    }

    /// NGO submits proof for milestone
    pub fn submit_proof(env: Env, milestone_id: u32, proof_hash: String) {
        let ngo: Address = env.storage().instance().get(&DataKey::NGO).unwrap();
        ngo.require_auth();

        let key = DataKey::Milestone(milestone_id);
        let mut milestone: Milestone = env.storage().persistent().get(&key).expect("Invalid milestone ID");
        
        milestone.status = MilestoneStatus::UnderReview;
        milestone.proof_hash = proof_hash;

        env.storage().persistent().set(&key, &milestone);
        env.storage().persistent().extend_ttl(&key, INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
        Self::extend_instance_ttl(&env);
    }

    /// NGO approves the release (First signature)
    pub fn approve_ngo(env: Env, milestone_id: u32) {
        let ngo: Address = env.storage().instance().get(&DataKey::NGO).unwrap();
        ngo.require_auth();

        let key = DataKey::Milestone(milestone_id);
        let mut milestone: Milestone = env.storage().persistent().get(&key).expect("Invalid milestone ID");

        milestone.ngo_approved = true;
        env.storage().persistent().set(&key, &milestone);
        
        Self::check_and_release(&env, milestone_id);
        Self::extend_instance_ttl(&env);
    }

    /// Auditor approves the release (Second signature)
    pub fn approve_auditor(env: Env, milestone_id: u32) {
        let auditor: Address = env.storage().instance().get(&DataKey::Auditor).unwrap();
        auditor.require_auth(); // MCP/AI agent signs here

        let key = DataKey::Milestone(milestone_id);
        let mut milestone: Milestone = env.storage().persistent().get(&key).expect("Invalid milestone ID");

        milestone.auditor_approved = true;
        env.storage().persistent().set(&key, &milestone);

        Self::check_and_release(&env, milestone_id);
        Self::extend_instance_ttl(&env);
    }

    /// Check if both signatures are present and release funds
    fn check_and_release(env: &Env, milestone_id: u32) {
        let key = DataKey::Milestone(milestone_id);
        let mut milestone: Milestone = env.storage().persistent().get(&key).unwrap();

        if milestone.ngo_approved && milestone.auditor_approved && !matches!(milestone.status, MilestoneStatus::Verified) {
            
            // Mark as verified
            milestone.status = MilestoneStatus::Verified;
            env.storage().persistent().set(&key, &milestone);
            env.storage().persistent().extend_ttl(&key, INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

            let beneficiary: Address = env.storage().instance().get(&DataKey::Beneficiary).unwrap();
            let token_address: Address = env.storage().instance().get(&DataKey::TokenAddress).unwrap();
            let contract_address = env.current_contract_address();

            let mut total: i128 = env.storage().instance().get(&DataKey::TotalEscrowed).unwrap();
            
            // Prevent panic if funds were not deposited yet
            if total < milestone.target_amount {
                panic!("Insufficient funds in escrow for this milestone");
            }

            // Update state BEFORE external call to prevent any re-entrancy edge cases
            total -= milestone.target_amount;
            env.storage().instance().set(&DataKey::TotalEscrowed, &total);

            // Execute transfer
            let token_client = token::Client::new(env, &token_address);
            token_client.transfer(&contract_address, &beneficiary, &milestone.target_amount);
        }
    }

    /// Admin cancels a milestone and routes funds to a refund address
    pub fn refund_milestone(env: Env, milestone_id: u32, refund_address: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let key = DataKey::Milestone(milestone_id);
        let mut milestone: Milestone = env.storage().persistent().get(&key).expect("Milestone not found");

        if matches!(milestone.status, MilestoneStatus::Verified) || matches!(milestone.status, MilestoneStatus::Rejected) {
            panic!("Cannot refund a milestone that is already verified or rejected");
        }

        // Mark as rejected so it cannot be acted upon again
        milestone.status = MilestoneStatus::Rejected;
        env.storage().persistent().set(&key, &milestone);
        env.storage().persistent().extend_ttl(&key, INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        let mut total: i128 = env.storage().instance().get(&DataKey::TotalEscrowed).unwrap();
        
        // Ensure we don't underflow if something went catastrophically wrong with deposits
        if total < milestone.target_amount {
            panic!("Insufficient funds in escrow to execute refund");
        }

        // Update state BEFORE external call (Re-entrancy protection)
        total -= milestone.target_amount;
        env.storage().instance().set(&DataKey::TotalEscrowed, &total);

        // Execute refund transfer
        let token_address: Address = env.storage().instance().get(&DataKey::TokenAddress).unwrap();
        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(&env.current_contract_address(), &refund_address, &milestone.target_amount);

        Self::extend_instance_ttl(&env);
    }

    /// View functions
    pub fn get_milestone(env: Env, milestone_id: u32) -> Milestone {
        env.storage().persistent().get(&DataKey::Milestone(milestone_id)).expect("Milestone not found")
    }

    pub fn get_total_escrowed(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::TotalEscrowed).unwrap_or(0)
    }

    pub fn get_milestone_count(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::MilestoneCount).unwrap_or(0)
    }

    /// Helper to keep the contract alive
    fn extend_instance_ttl(env: &Env) {
        env.storage().instance().extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    }
}

mod test;
