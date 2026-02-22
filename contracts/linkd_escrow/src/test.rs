#[cfg(test)]
mod test {
    use crate::{LinkdEscrow, LinkdEscrowClient, MilestoneStatus};
    use soroban_sdk::{testutils::Address as _, token, Address, Env, String};

    fn create_token_contract<'a>(env: &Env, admin: &Address) -> (token::Client<'a>, Address) {
        let token_address = env.register_stellar_asset_contract(admin.clone());
        let token = token::Client::new(env, &token_address);
        (token, token_address)
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, LinkdEscrow);
        let client = LinkdEscrowClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let ngo = Address::generate(&env);
        let auditor = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let (_, token_address) = create_token_contract(&env, &admin);

        client.initialize(&admin, &ngo, &auditor, &beneficiary, &token_address);

        assert_eq!(client.get_milestone_count(), 0);
        assert_eq!(client.get_total_escrowed(), 0);
    }

    #[test]
    fn test_dual_signature_flow() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, LinkdEscrow);
        let client = LinkdEscrowClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let ngo = Address::generate(&env);
        let auditor = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let donor = Address::generate(&env);

        let (token, token_address) = create_token_contract(&env, &admin);
        let admin_client = token::StellarAssetClient::new(&env, &token_address);
        admin_client.mint(&donor, &5000);

        client.initialize(&admin, &ngo, &auditor, &beneficiary, &token_address);

        client.add_milestone(&1000);

        client.deposit(&donor, &5000);
        assert_eq!(client.get_total_escrowed(), 5000);

        let proof = String::from_str(&env, "ipfs://QmProofHash");
        client.submit_proof(&0, &proof);

        // Verify isolation of signatures
        client.approve_ngo(&0);
        assert_eq!(client.get_total_escrowed(), 5000);
        assert_eq!(token.balance(&beneficiary), 0);

        client.approve_auditor(&0);
        assert_eq!(client.get_total_escrowed(), 4000);
        assert_eq!(token.balance(&beneficiary), 1000);

        let m = client.get_milestone(&0);
        assert!(matches!(m.status, MilestoneStatus::Verified));
    }

    #[test]
    fn test_refund_flow() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, LinkdEscrow);
        let client = LinkdEscrowClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let ngo = Address::generate(&env);
        let auditor = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let donor = Address::generate(&env);
        let treasury = Address::generate(&env);

        let (token, token_address) = create_token_contract(&env, &admin);
        let admin_client = token::StellarAssetClient::new(&env, &token_address);
        admin_client.mint(&donor, &5000);

        client.initialize(&admin, &ngo, &auditor, &beneficiary, &token_address);
        client.add_milestone(&1000);
        client.deposit(&donor, &5000);

        client.refund_milestone(&0, &treasury);

        assert_eq!(client.get_total_escrowed(), 4000);
        assert_eq!(token.balance(&treasury), 1000);

        let m = client.get_milestone(&0);
        assert!(matches!(m.status, MilestoneStatus::Rejected));
    }
}
