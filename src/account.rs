use std::collections::HashMap;

pub struct Multisig {
    global: Role,
    roles: HashMap<String, Role>,
    members: Vec<Member>,
}

pub struct Member {
    // social data
    username: String,
    avatar: String,
    // member data
    address: String,
    weight: u64,
    roles: Vec<String>,
}

#[derive(Debug)]
pub struct Role {
    // threshold to reach for the role
    threshold: u64,
    // sum of the weight of the members with the role
    total_weight: u64,
}