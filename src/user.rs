use anyhow::Result;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use sui_graphql_client::query_types::ObjectFilter;
use sui_graphql_client::{Client, PaginationFilter};
use sui_sdk_types::{Address, ObjectData, ObjectId};

use crate::move_binding::{account_multisig as am, account_protocol as ap};
use crate::utils;

pub struct User {
    pub sui_client: Arc<Client>,
    pub address: Address,
    pub id: ObjectId,
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
    pub async fn from_address(sui_client: Arc<Client>, address: Address) -> Result<Self> {
        let mut user = Self {
            sui_client,
            address,
            id: "0x0".parse().unwrap(),
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
            self.id = user.id;
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
                if invite.account_type == format!("{}::multisig::Multisig", &crate::ACCOUNT_MULTISIG_PACKAGE[2..]) {
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
