use anyhow::Result;
use move_types::functions::Arg;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use sui_graphql_client::query_types::ObjectFilter;
use sui_graphql_client::{Client, PaginationFilter};
use sui_sdk_types::{Address, ObjectData, ObjectId};
use sui_transaction_builder::{Serialized, TransactionBuilder};

use crate::move_binding::{account_multisig as am, account_protocol as ap};
use crate::utils;

pub struct User {
    pub sui_client: Arc<Client>,
    pub address: Address,
    pub id: Option<ObjectId>,
    pub profile: Profile,
    pub multisigs: Vec<MultisigPreview>,
    pub invites: Vec<Invite>,
}

#[derive(Debug, Clone)]
pub struct Profile {
    pub username: String,
    pub avatar: String,
}

#[derive(Debug, Clone)]
pub struct MultisigPreview {
    pub id: ObjectId,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Invite {
    pub id: ObjectId,
    pub multisig_id: ObjectId,
    pub multisig_name: String,
}

impl User {
    pub const REGISTRY: &str = "0xa9ec2fd2c9ac1ed9cde4972da6014818c3343a1d65dc140a8d51567c20d8992e";

    pub async fn from_address(sui_client: Arc<Client>, address: Address) -> Result<Self> {
        let mut user = Self {
            sui_client,
            address,
            id: None,
            profile: Profile {
                username: "".to_string(),
                avatar: "".to_string(),
            },
            multisigs: Vec::new(),
            invites: Vec::new(),
        };
        user.refresh().await?;
        Ok(user)
    }

    pub async fn refresh(&mut self) -> Result<()> {
        let user = self.fetch_user_object().await?;
        if let Some(user) = user {
            self.id = Some(user.id);
            self.multisigs = self.fetch_previews(&user).await?;
        }

        self.profile = self.fetch_profile().await?;
        self.invites = self.fetch_invites().await?;

        Ok(())
    }

    pub async fn fetch_user_object(&self) -> Result<Option<ap::user::User>> {
        let page = self
            .sui_client
            .objects(
                Some(ObjectFilter {
                    owner: Some(self.address),
                    type_: Some(
                        format!("{}::user::User", crate::ACCOUNT_PROTOCOL_PACKAGE).as_str(),
                    ),
                    ..Default::default()
                }),
                PaginationFilter::default(),
            )
            .await?;

        if let Some(object) = page.data().first() {
            if let ObjectData::Struct(move_struct) = object.data() {
                let user: ap::user::User = bcs::from_bytes(move_struct.contents())?;
                Ok(Some(user))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub async fn fetch_previews(&self, user: &ap::user::User) -> Result<Vec<MultisigPreview>> {
        let multisig_type = format!(
            "{}::multisig::Multisig",
            &crate::ACCOUNT_MULTISIG_PACKAGE[2..]
        );
        let ids = user
            .accounts
            .contents
            .iter()
            .find(|entry| entry.key == multisig_type)
            .map(|entry| entry.value.clone())
            .unwrap_or_default();

        let objects = utils::get_objects(&self.sui_client, ids).await?;
        let mut previews = Vec::new();
        for object in objects {
            if let ObjectData::Struct(move_struct) = object.data() {
                let account: ap::account::Account<am::multisig::Multisig> =
                    bcs::from_bytes(move_struct.contents())?;
                previews.push(MultisigPreview {
                    id: account.id,
                    name: account
                        .metadata
                        .inner
                        .contents
                        .iter()
                        .find(|entry| entry.key == "name")
                        .map(|entry| entry.value.to_string())
                        .unwrap_or_default(),
                });
            }
        }
        Ok(previews)
    }

    pub async fn fetch_profile(&self) -> Result<Profile> {
        let username = self.sui_client.default_suins_name(self.address).await?;
        Ok(Profile {
            username: username.unwrap_or_default(),
            avatar: "".to_string(), // can't get avatar from suins easily as of now
        })
    }

    pub async fn fetch_invites(&self) -> Result<Vec<Invite>> {
        // get invite objects
        let invite_objects = utils::get_owned_objects(
            &self.sui_client,
            self.address,
            Some(format!("{}::invite::Invite", crate::ACCOUNT_PROTOCOL_PACKAGE).as_str()),
        )
        .await?;
        let mut multisig_to_invite = HashMap::new();
        for object in invite_objects {
            if let ObjectData::Struct(move_struct) = object.data() {
                let invite: ap::user::Invite = bcs::from_bytes(move_struct.contents())?;
                if invite.account_type
                    == format!(
                        "{}::multisig::Multisig",
                        &crate::ACCOUNT_MULTISIG_PACKAGE[2..]
                    )
                {
                    multisig_to_invite.insert(invite.account_addr, invite.id);
                }
            }
        }

        // get multisig objects
        let multisig_objects = utils::get_objects(
            &self.sui_client,
            multisig_to_invite.keys().cloned().collect(),
        )
        .await?;
        let mut invites = Vec::new();
        for object in multisig_objects {
            if let ObjectData::Struct(move_struct) = object.data() {
                let account: ap::account::Account<am::multisig::Multisig> =
                    bcs::from_bytes(move_struct.contents())?;
                invites.push(Invite {
                    id: *multisig_to_invite.get(account.id.as_address()).unwrap(),
                    multisig_id: account.id,
                    multisig_name: account
                        .metadata
                        .inner
                        .contents
                        .iter()
                        .find(|entry| entry.key == "name")
                        .map(|entry| entry.value.to_string())
                        .unwrap_or_default(),
                });
            }
        }

        Ok(invites)
    }

    pub async fn create_user(
        &self,
        builder: &mut TransactionBuilder,
    ) -> Result<Arg<ap::user::User>> {
        if self.id.is_some() {
            return Err(anyhow::anyhow!("User already exists"));
        }
        let user = ap::user::new(builder);
        Ok(user)
    }

    pub async fn transfer_user(
        &self,
        builder: &mut TransactionBuilder,
        user: Arg<ap::user::User>,
    ) -> Result<()> {
        if self.id.is_none() {
            return Err(anyhow::anyhow!("User not found"));
        }
        let mut registry = self.registry_arg(builder).await?;
        let address = builder.input(Serialized(&self.address));
        ap::user::transfer(builder, registry.borrow_mut(), user, address.into());
        Ok(())
    }

    pub async fn delete_user(&self, builder: &mut TransactionBuilder) -> Result<()> {
        if self.id.is_none() {
            return Err(anyhow::anyhow!("User not found"));
        }
        let mut registry = self.registry_arg(builder).await?;
        let user = self
            .user_arg(builder, *self.id.unwrap().as_address())
            .await?;
        ap::user::destroy(builder, registry.borrow_mut(), user);
        Ok(())
    }

    pub async fn send_invite(
        &self,
        builder: &mut TransactionBuilder,
        multisig: &Arg<ap::account::Account<am::multisig::Multisig>>,
        recipient: Address,
    ) -> Result<()> {
        let recipient_arg = builder.input(Serialized(&recipient));
        am::multisig::send_invite(builder, multisig.borrow(), recipient_arg.into());
        Ok(())
    }

    pub async fn accept_invite(
        &self,
        builder: &mut TransactionBuilder,
        invite_id: Address,
    ) -> Result<()> {
        let mut user = if self.id.is_none() {
            self.create_user(builder).await?
        } else {
            self.user_arg(builder, *self.id.unwrap().as_address())
                .await?
        };
        let invite = self.invite_arg(builder, invite_id).await?;

        ap::user::accept_invite(builder, user.borrow_mut(), invite);

        if self.id.is_none() {
            self.transfer_user(builder, user).await?;
        }
        Ok(())
    }

    pub async fn refuse_invite(
        &self,
        builder: &mut TransactionBuilder,
        invite_id: Address,
    ) -> Result<()> {
        let invite = self.invite_arg(builder, invite_id).await?;
        ap::user::refuse_invite(builder, invite);
        Ok(())
    }

    // === Helpers ===

    pub async fn registry_arg(
        &self,
        builder: &mut TransactionBuilder,
    ) -> Result<Arg<ap::user::Registry>> {
        let registry_input =
            utils::get_object_as_input(&self.sui_client, Self::REGISTRY.parse().unwrap()).await?;
        let registry_arg = builder.input(registry_input.by_mut()).into();
        Ok(registry_arg)
    }

    pub async fn user_arg(
        &self,
        builder: &mut TransactionBuilder,
        user_id: Address,
    ) -> Result<Arg<ap::user::User>> {
        let user_input = utils::get_object_as_input(&self.sui_client, user_id).await?;
        let user_arg = builder.input(user_input.by_val()).into();
        Ok(user_arg)
    }

    pub async fn invite_arg(
        &self,
        builder: &mut TransactionBuilder,
        invite_id: Address,
    ) -> Result<Arg<ap::user::Invite>> {
        let invite_input = utils::get_object_as_input(&self.sui_client, invite_id).await?;
        let invite_arg = builder.input(invite_input.by_val()).into();
        Ok(invite_arg)
    }
}

impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("User")
            .field("address", &self.address)
            .field("id", &self.id)
            .field("profile", &self.profile)
            .field("multisigs", &self.multisigs)
            .field("invites", &self.invites)
            .finish()
    }
}
