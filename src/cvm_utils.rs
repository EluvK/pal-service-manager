use itertools::Itertools;
use tencentcloud_sdk::{
    client::{cvm::cvm_instance::Price, TencentCloudClient},
    constant::{InstanceType, Region},
};

use crate::constant::ServiceInstanceType;

/// return cheapest (price, (region, zone, instance_type))
pub async fn query_spot_paid_price(
    client: &TencentCloudClient,
    candidate_regions: &[Region],
    instance_type: ServiceInstanceType,
) -> anyhow::Result<(Price, (Region, String, InstanceType))> {
    let candidate_instance_type = instance_type.to_list();

    let mut handles = vec![];

    for region in candidate_regions {
        let zones = client.cvm().zone().describe_zone(&region).await?;
        if let Some(zones) = zones {
            for (zone, instance_type) in zones
                .iter()
                .cartesian_product(candidate_instance_type.iter())
            {
                let client = client.clone();
                let zone = zone.clone();
                let region = region.clone();
                let instance_type = instance_type.clone();
                handles.push(tokio::spawn(async move {
                    (
                        client
                            .cvm()
                            .instances()
                            .query_price(&region, &zone, &instance_type)
                            .await,
                        region,
                        zone,
                        instance_type,
                    )
                }));
            }
        }
    }

    let mut price_result = vec![];
    for handle in handles {
        if let (Ok(price), region, zone, instance_type) = handle.await? {
            price_result.push((price, (region, zone, instance_type)));
        }
    }
    price_result.sort_by(|a, b| {
        a.0.instance_price
            .unit_price_discount
            .total_cmp(&b.0.instance_price.unit_price_discount)
    });
    price_result.into_iter().nth(0).ok_or(anyhow::anyhow!(
        "failed to get any available instance of {candidate_instance_type:?}"
    ))
}

pub async fn query_key_ids(client: &TencentCloudClient) -> anyhow::Result<Vec<String>> {
    client
        .cvm()
        .keys()
        .describe_key_pairs(&Region::Hongkong) // whatever here
        .await
        .map(|vk| vk.into_iter().map(|k| k.key_id).collect())
}