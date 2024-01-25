mod config;
mod constant;
mod cvm_utils;
mod psm;

use crate::psm::PalServiceManager;

#[tokio::main]
async fn main() {
    let config = config::load_from_file();
    let psm = PalServiceManager::new(config).await;

    // let candidate_regions = vec![Region::Nanjing, Region::Guangzhou, Region::Shanghai];

    // let r = query_spot_paid_price(&client, &candidate_regions, ServiceInstanceType::T4C32G).await;
    // println!("r:{r:?}");
    // let r = query_spot_paid_price(&client, &candidate_regions, ServiceInstanceType::T2C16G).await;
    // println!("r:{r:?}");
    // let r = query_spot_paid_price(&client, &candidate_regions, ServiceInstanceType::T2C8G).await;
    // println!("r:{r:?}");

    // let r = query_spot_paid_price(&client, &candidate_regions, ServiceInstanceType::T2C2G).await;
    // println!("r:{r:?}");

    // let r = query_key_ids(&client).await;
    // println!("r:{r:?}");
}
