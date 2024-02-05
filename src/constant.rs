use tencentcloud_sdk::constant::InstanceType;

#[derive(Debug, Clone)]
pub enum ServiceInstanceType {
    T2C2G, // simple test
    T2C8G,
    T4C8G,
    T2C16G,
    T4C16G,
    T4C32G,
    T8C32G,
}

impl TryFrom<String> for ServiceInstanceType {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_ascii_lowercase().as_str() {
            "2c2g" => Ok(Self::T2C2G),
            "2c8g" => Ok(Self::T2C8G),
            "4c8g" => Ok(Self::T4C8G),
            "2c16g" => Ok(Self::T2C16G),
            "4c16g" => Ok(Self::T4C16G),
            "4c32g" => Ok(Self::T4C32G),
            "8c32g" => Ok(Self::T8C32G),
            _ => Err(anyhow::anyhow!("{} is not a valid instance type", value)),
        }
    }
}

impl ServiceInstanceType {
    pub fn to_list(&self) -> Vec<InstanceType> {
        match self {
            ServiceInstanceType::T2C2G => vec![InstanceType::SA2Medium2],
            ServiceInstanceType::T2C8G => vec![InstanceType::SA2Medium8],
            ServiceInstanceType::T4C8G => vec![InstanceType::SA2Large8, InstanceType::SA3Large8],
            ServiceInstanceType::T2C16G => vec![
                InstanceType::MA3Medium16,
                // InstanceType::M5Medium16
            ],
            ServiceInstanceType::T4C16G => vec![
                InstanceType::SA2Large16,
                InstanceType::SA3Large16,
                // InstanceType::S5Large16,
                // InstanceType::S6Large16,
                // InstanceType::SA5Large16,
            ],
            ServiceInstanceType::T4C32G => vec![
                InstanceType::MA3Large32,
                InstanceType::MA2Large32,
                // InstanceType::M5Large32, // too expensive
            ],
            ServiceInstanceType::T8C32G => vec![InstanceType::SA22Xlarge32],
        }
    }
}
