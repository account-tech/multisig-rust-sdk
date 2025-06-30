mod utils;
use utils::{init_tx, execute_tx, get_created_multisig};

use account_multisig_sdk::MultisigClient;
use account_multisig_sdk::params::ParamsArgs;
use account_multisig_sdk::params::ConfigMultisigArgs;
use sui_sdk_types::Address;

#[tokio::test]
async fn test_config_multisig_intent() {
    let mut client = MultisigClient::new_testnet();

    // TX 1: Create multisig
    let multisig_id = {
        let (pk, mut builder) = init_tx(client.sui()).await;
        client.create_multisig(&mut builder).await.unwrap();
        let effects = execute_tx(client.sui(), pk, builder).await;
        get_created_multisig(&effects).await
    };
    client.load_multisig(multisig_id).await.unwrap();

    // TX 2: Request config multisig
    {
        let (pk, mut builder) = init_tx(client.sui()).await;
        let address = pk.public_key().derive_address();
        let params = ParamsArgs::new(
            &mut builder,
            "config_multisig".to_string(),
            "Config multisig".to_string(),
            vec![0],
            1000000000000000000,
        );
        let args = ConfigMultisigArgs::new(
            &mut builder,
            vec![address, Address::ZERO],
            vec![2, 1],
            vec![vec!["460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::config".to_string()], vec![]],
            2,
            vec!["460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::config".to_string()],
            vec![1],
        );
        let resp = client
            .request_config_multisig(&mut builder, params, args)
            .await;
        assert!(resp.is_ok());
        execute_tx(client.sui(), pk, builder).await;
        // check results
        client.refresh().await.unwrap();
        let intent = client.intent("config_multisig").unwrap();
        assert_eq!(intent.type_, "460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::config::ConfigMultisigIntent");
        assert_eq!(intent.key, "config_multisig");
        assert_eq!(intent.description, "Config multisig");
        assert_eq!(intent.account, multisig_id);
        assert_eq!(intent.creator, address);
        assert_ne!(intent.creation_time, 0);
        assert!(!intent.execution_times.is_empty());
        assert_ne!(intent.expiration_time, 0);
        assert_eq!(intent.role, "460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::config");
        assert_ne!(intent.actions_bag_id, Address::ZERO);
        assert!(intent.actions_bcs.is_empty());
        assert_eq!(intent.outcome.total_weight, 0);
        assert_eq!(intent.outcome.role_weight, 0);
        assert_eq!(intent.outcome.approved.len(), 0);
    }

    // TX 3: Approve intent
    {
        let (pk, mut builder) = init_tx(client.sui()).await;
        let address = pk.public_key().derive_address();
        client.approve_intent(&mut builder, "config_multisig".to_string()).await.unwrap();
        execute_tx(client.sui(), pk, builder).await;
        // check results
        client.refresh().await.unwrap();
        let intent = client.intent("config_multisig").unwrap();
        assert_eq!(intent.outcome.total_weight, 1);
        assert_eq!(intent.outcome.role_weight, 0);
        assert_eq!(intent.outcome.approved, vec![address]);
    }

    // TX 4: Execute intent
    {
        let (pk, mut builder) = init_tx(client.sui()).await;
        let address = pk.public_key().derive_address();
        let clear = client.intent("config_multisig").unwrap().execution_times.len() <= 1;
        client.execute_config_multisig(&mut builder, "config_multisig".to_string(), clear).await.unwrap();
        execute_tx(client.sui(), pk, builder).await;
        // check results
        client.refresh().await.unwrap();
        assert!(client.intent("config_multisig").is_none());
        assert_eq!(client.multisig().unwrap().config.members.len(), 2);
        assert_eq!(client.multisig().unwrap().config.members[0].address, address.to_string());
        assert_eq!(client.multisig().unwrap().config.members[0].weight, 2);
        assert_eq!(client.multisig().unwrap().config.members[0].roles, vec!["460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::config".to_string()]);
        assert_eq!(client.multisig().unwrap().config.members[1].address, Address::ZERO.to_string());
        assert_eq!(client.multisig().unwrap().config.members[1].weight, 1);
        assert_eq!(client.multisig().unwrap().config.members[1].roles, Vec::<String>::new());
        assert_eq!(client.multisig().unwrap().config.global.threshold, 2);
        assert_eq!(client.multisig().unwrap().config.global.total_weight, 3);
        assert_eq!(client.multisig().unwrap().config.roles.len(), 1);
        assert_eq!(client.multisig().unwrap().config.roles["460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::config"].threshold, 1);
        assert_eq!(client.multisig().unwrap().config.roles["460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::config"].total_weight, 2);
    }
}