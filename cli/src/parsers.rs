
#[derive(Debug, Clone)]
pub struct Member {
    pub address: String,
    pub weight: u64,
    pub roles: Vec<String>,
}

impl std::str::FromStr for Member {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Format: address:weight:role1,role2
        let mut parts = s.splitn(3, ':');
        let address = parts.next().ok_or("Missing address")?.to_string();
        let weight = parts
            .next()
            .ok_or("Missing weight")?
            .parse()
            .map_err(|_| "Invalid weight")?;
        let roles = parts
            .next()
            .map(|r| r.split(',').map(|s| s.to_string()).collect())
            .unwrap_or_else(Vec::new);
        Ok(Member {
            address,
            weight,
            roles,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Role {
    pub name: String,
    pub threshold: u64,
}

impl std::str::FromStr for Role {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Format: name:threshold
        let mut parts = s.splitn(2, ':');
        let name = parts.next().ok_or("Missing name")?.to_string();
        let threshold = parts
            .next()
            .ok_or("Missing threshold")?
            .parse()
            .map_err(|_| "Invalid threshold")?;
        Ok(Role { name, threshold })
    }
}