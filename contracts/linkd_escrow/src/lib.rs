#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol, Vec};

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
    pub proof_hash: Symbol,
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
    Milestones,
    MilestoneCount,
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
        env.storage()
            .instance()
            .set(&DataKey::TokenAddress, &token_address);
        env.storage().instance().set(&DataKey::TotalEscrowed, &0i128);
        env.storage().instance().set(&DataKey::MilestoneCount, &0u32);

        let milestones: Vec<Milestone> = Vec::new(&env);
        env.storage().instance().set(&DataKey::Milestones, &milestones);
    }

    /// Add a new milestone
    pub fn add_milestone(env: Env, caller: Address, target_amount: i128) -> u32 {
        caller.require_auth();

        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if caller != admin {
            panic!("Only admin can add milestones");
        }

        let mut milestone_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MilestoneCount)
            .unwrap_or(0);

        let milestone = Milestone {
            id: milestone_count,
            target_amount,
            status: MilestoneStatus::Pending,
            proof_hash: Symbol::new(&env, ""),
            ngo_approved: false,
            auditor_approved: false,
        };

        let mut milestones: Vec<Milestone> = env
            .storage()
            .instance()
            .get(&DataKey::Milestones)
            .unwrap_or(Vec::new(&env));
        milestones.push_back(milestone);

        env.storage().instance().set(&DataKey::Milestones, &milestones);

        milestone_count += 1;
        env.storage()
            .instance()
            .set(&DataKey::MilestoneCount, &milestone_count);

        milestone_count - 1
    }

    /// Deposit funds into escrow
    pub fn deposit(env: Env, from: Address, amount: i128) {
        from.require_auth();

        let token_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .unwrap();
        let contract_address = env.current_contract_address();

        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(&from, &contract_address, &amount);

        let mut total: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalEscrowed)
            .unwrap_or(0);
        total += amount;
        env.storage().instance().set(&DataKey::TotalEscrowed, &total);
    }

    /// NGO submits proof for milestone
    pub fn submit_proof(env: Env, caller: Address, milestone_id: u32, proof_hash: Symbol) {
        caller.require_auth();

        let ngo: Address = env.storage().instance().get(&DataKey::NGO).unwrap();
        if caller != ngo {
            panic!("Only NGO can submit proof");
        }

        let mut milestones: Vec<Milestone> =
            env.storage().instance().get(&DataKey::Milestones).unwrap();

        let mut milestone = milestones.get(milestone_id).expect("Invalid milestone ID");
        milestone.status = MilestoneStatus::UnderReview;
        milestone.proof_hash = proof_hash;

        milestones.set(milestone_id, milestone);
        env.storage().instance().set(&DataKey::Milestones, &milestones);
    }

    /// NGO approves the release (First signature)
    pub fn approve_ngo(env: Env, caller: Address, milestone_id: u32) {
        caller.require_auth();

        let ngo: Address = env.storage().instance().get(&DataKey::NGO).unwrap();
        if caller != ngo {
            panic!("Only NGO can approve as NGO");
        }

        let mut milestones: Vec<Milestone> =
            env.storage().instance().get(&DataKey::Milestones).unwrap();
        let mut milestone = milestones.get(milestone_id).expect("Invalid milestone ID");

        milestone.ngo_approved = true;
        milestones.set(milestone_id, milestone);
        env.storage().instance().set(&DataKey::Milestones, &milestones);

        Self::check_and_release(&env, milestone_id);
    }

    /// Auditor approves the release (Second signature)
    pub fn approve_auditor(env: Env, caller: Address, milestone_id: u32) {
        caller.require_auth();

        let auditor: Address = env.storage().instance().get(&DataKey::Auditor).unwrap();
        if caller != auditor {
            panic!("Only Auditor can approve as Auditor");
        }

        let mut milestones: Vec<Milestone> =
            env.storage().instance().get(&DataKey::Milestones).unwrap();
        let mut milestone = milestones.get(milestone_id).expect("Invalid milestone ID");

        milestone.auditor_approved = true;
        milestones.set(milestone_id, milestone);
        env.storage().instance().set(&DataKey::Milestones, &milestones);

        Self::check_and_release(&env, milestone_id);
    }

    /// Check if both signatures are present and release funds
    fn check_and_release(env: &Env, milestone_id: u32) {
        let mut milestones: Vec<Milestone> =
            env.storage().instance().get(&DataKey::Milestones).unwrap();
        let mut milestone = milestones.get(milestone_id).unwrap();

        if milestone.ngo_approved && milestone.auditor_approved && !matches!(milestone.status, MilestoneStatus::Verified) {
            // Mark as verified
            milestone.status = MilestoneStatus::Verified;
            milestones.set(milestone_id, milestone.clone());
            env.storage().instance().set(&DataKey::Milestones, &milestones);

            // Release funds to beneficiary
            let beneficiary: Address = env.storage().instance().get(&DataKey::Beneficiary).unwrap();
            let token_address: Address = env
                .storage()
                .instance()
                .get(&DataKey::TokenAddress)
                .unwrap();
            let contract_address = env.current_contract_address();

            let token_client = token::Client::new(env, &token_address);
            token_client.transfer(&contract_address, &beneficiary, &milestone.target_amount);

            // Update total escrowed
            let mut total: i128 = env.storage().instance().get(&DataKey::TotalEscrowed).unwrap();
            total -= milestone.target_amount;
            env.storage().instance().set(&DataKey::TotalEscrowed, &total);
        }
    }

    /// View functions
    pub fn get_milestone(env: Env, milestone_id: u32) -> Milestone {
        let milestones: Vec<Milestone> =
            env.storage().instance().get(&DataKey::Milestones).unwrap();
        milestones.get(milestone_id).unwrap()
    }

    pub fn get_total_escrowed(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalEscrowed)
            .unwrap_or(0)
    }

    pub fn get_milestone_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::MilestoneCount)
            .unwrap_or(0)
    }
}

mod test;
