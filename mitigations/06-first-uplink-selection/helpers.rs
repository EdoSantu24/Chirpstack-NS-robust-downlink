use std::str::FromStr;

use anyhow::Result;
use uuid::Uuid;

use chirpstack_api::{gw, internal};
use lrwn::region::DataRateModulation;

// DELETED: `crate::config` and `rand` imports since we no longer calculate SNR margins or use randomness
use crate::region;

// Returns the gateway to use for downlink.
// NEW ALGORITHM: First-Arrived Gateway Selection
// It will filter out private gateways (gateways from a different tenant ID,
// that do not allow downlinks). The result is strictly the FIRST gateway that 
// delivered the uplink, mitigating Time-based Wormhole Replay attacks.
pub fn select_downlink_gateway(
    tenant_id: Option<Uuid>,
    _region_config_id: &str, // PREFIXED with _ to suppress unused variable warning
    _min_snr_margin: f32,    // PREFIXED with _ to suppress unused variable warning
    rx_info: &mut internal::DeviceGatewayRxInfo,
) -> Result<internal::DeviceGatewayRxInfoItem> {
    
    // STEP 1: Privacy and Tenant Filtering (Kept from Baseline)
    rx_info.items.retain(|rx_info| {
        if let Some(tenant_id) = &tenant_id {
            if tenant_id.as_bytes().to_vec() == rx_info.tenant_id {
                true
            } else {
                !rx_info.is_private_down
            }
        } else {
            !rx_info.is_private_down
        }
    });

    if rx_info.items.is_empty() {
        return Err(anyhow!(
            "RxInfo set is empty after applying filters, no downlink gateway available"
        ));
    }

    // ========================================================================
    // CHANGED: O(1) First-Arrived Gateway Selection
    // ChirpStack populates the `rx_info.items` array sequentially during the 
    // 200ms deduplication window. The gateway at index 0 is mathematically 
    // the first one to have delivered the packet over the backhaul.
    // ========================================================================
    
    let selected_gw = &rx_info.items[0];
    
    // FORMAT FIX: Convert the byte array into a continuous hex string for logging
    let gw_hex: String = selected_gw.gateway_id.iter().map(|b| format!("{:02x}", b)).collect();

    // PRINTING MESSAGE FOR TESTING/DEBUGGING
    println!(
        "\n📡 [ROUTING LOG] Selected First-Arrived Gateway: {}", 
        gw_hex
    );
    println!(
        "   -> Bypassing SNR/RSSI checks to mitigate delayed replay attacks.\n"
    );

    // Return the first item
    Ok(selected_gw.clone())
    // ========================================================================
}


pub fn set_tx_info_data_rate(
    tx_info: &mut chirpstack_api::gw::DownlinkTxInfo,
    dr: &DataRateModulation,
) -> Result<()> {
    match dr {
        DataRateModulation::Lora(v) => {
            tx_info.modulation = Some(gw::Modulation {
                parameters: Some(gw::modulation::Parameters::Lora(gw::LoraModulationInfo {
                    bandwidth: v.bandwidth,
                    spreading_factor: v.spreading_factor as u32,
                    code_rate: gw::CodeRate::from_str(&v.coding_rate)
                        .map_err(|e| anyhow!("{}", e))?
                        .into(),
                    polarization_inversion: true,
                    code_rate_legacy: "".into(),
                    preamble: 0,
                    no_crc: false,
                })),
            });
        }
        DataRateModulation::Fsk(v) => {
            tx_info.modulation = Some(gw::Modulation {
                parameters: Some(gw::modulation::Parameters::Fsk(gw::FskModulationInfo {
                    datarate: v.bitrate,
                    frequency_deviation: v.bitrate / 2, // see: https://github.com/brocaar/chirpstack-gateway-bridge/issues/16
                })),
            });
        }
        DataRateModulation::LrFhss(_) => {
            return Err(anyhow!("LR-FHSS is not supported for downlink"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::storage::tenant;
    use crate::test;

    struct Test {
        min_snr_margin: f32,
        tenant_id: Option<Uuid>,
        rx_info: internal::DeviceGatewayRxInfo,
        expected_gws: Vec<Vec<u8>>,
    }

    #[tokio::test]
    async fn test_select_downlink_gateway() {
        let _guard = test::prepare().await;

        let t = tenant::create(tenant::Tenant {
            name: "test-tenant".into(),
            ..Default::default()
        })
        .await
        .unwrap();

        let tests = vec![
            // single item
            Test {
                tenant_id: None,
                min_snr_margin: 0.0,
                rx_info: internal::DeviceGatewayRxInfo {
                    dr: 0,
                    items: vec![internal::DeviceGatewayRxInfoItem {
                        lora_snr: -5.0,
                        gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                expected_gws: vec![vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]],
            },
            // two items, CHANGED: first arrived wins (index 0) regardless of SNR
            Test {
                tenant_id: None,
                min_snr_margin: 5.0,
                rx_info: internal::DeviceGatewayRxInfo {
                    dr: 2, 
                    items: vec![
                        internal::DeviceGatewayRxInfoItem {
                            lora_snr: -12.0, // Worse SNR, but arrived first!
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
                            ..Default::default()
                        },
                        internal::DeviceGatewayRxInfoItem {
                            lora_snr: -11.0, 
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                expected_gws: vec![vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]],
            },
            // two items, CHANGED: first arrived wins
            Test {
                tenant_id: None,
                min_snr_margin: 5.0,
                rx_info: internal::DeviceGatewayRxInfo {
                    dr: 2, 
                    items: vec![
                        internal::DeviceGatewayRxInfoItem {
                            lora_snr: -12.0,
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
                            ..Default::default()
                        },
                        internal::DeviceGatewayRxInfoItem {
                            lora_snr: -10.0,
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                expected_gws: vec![vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]],
            },
            // four items, CHANGED: first arrived wins
            Test {
                tenant_id: None,
                min_snr_margin: 5.0,
                rx_info: internal::DeviceGatewayRxInfo {
                    dr: 2, 
                    items: vec![
                        internal::DeviceGatewayRxInfoItem {
                            lora_snr: -12.0,
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
                            ..Default::default()
                        },
                        internal::DeviceGatewayRxInfoItem {
                            lora_snr: -11.0,
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02],
                            ..Default::default()
                        },
                        internal::DeviceGatewayRxInfoItem {
                            lora_snr: -10.0,
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03],
                            ..Default::default()
                        },
                        internal::DeviceGatewayRxInfoItem {
                            lora_snr: -9.0,
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                expected_gws: vec![
                    vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
                ],
            },
            // is_private_down is set, first gateway matches tenant.
            Test {
                tenant_id: Some(t.id.into()),
                min_snr_margin: 0.0,
                rx_info: internal::DeviceGatewayRxInfo {
                    items: vec![
                        internal::DeviceGatewayRxInfoItem {
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
                            is_private_down: true,
                            tenant_id: t.id.as_bytes().to_vec(),
                            ..Default::default()
                        },
                        internal::DeviceGatewayRxInfoItem {
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02],
                            is_private_down: true,
                            tenant_id: Uuid::new_v4().as_bytes().to_vec(),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                expected_gws: vec![vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]],
            },
            // is_private_down is set, second gateway matches tenant.
            // NOTE: Filter drops GW 1, so GW 2 becomes index 0!
            Test {
                tenant_id: Some(t.id.into()),
                min_snr_margin: 0.0,
                rx_info: internal::DeviceGatewayRxInfo {
                    items: vec![
                        internal::DeviceGatewayRxInfoItem {
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
                            is_private_down: true,
                            tenant_id: Uuid::new_v4().as_bytes().to_vec(),
                            ..Default::default()
                        },
                        internal::DeviceGatewayRxInfoItem {
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02],
                            is_private_down: true,
                            tenant_id: t.id.as_bytes().to_vec(),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                expected_gws: vec![vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02]],
            },
            // is_private_down is set for one gateway, no tenant id given.
            // NOTE: Filter drops GW 1, so GW 2 becomes index 0!
            Test {
                tenant_id: None,
                min_snr_margin: 0.0,
                rx_info: internal::DeviceGatewayRxInfo {
                    items: vec![
                        internal::DeviceGatewayRxInfoItem {
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
                            is_private_down: true,
                            tenant_id: t.id.as_bytes().to_vec(),
                            ..Default::default()
                        },
                        internal::DeviceGatewayRxInfoItem {
                            gateway_id: vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02],
                            is_private_down: false,
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                expected_gws: vec![vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02]],
            },
        ];

        for test in &tests {
            let mut rx_info = test.rx_info.clone();
            let mut gw_map = HashMap::new();

            let mut expected_gws = HashMap::new();
            for gw_id in &test.expected_gws {
                expected_gws.insert(gw_id.clone(), ());
            }

            for _ in 0..100 {
                let out = select_downlink_gateway(
                    test.tenant_id,
                    "eu868",
                    test.min_snr_margin,
                    &mut rx_info,
                )
                .unwrap();
                gw_map.insert(out.gateway_id, ());
            }

            assert_eq!(test.expected_gws.len(), gw_map.len());
            assert!(
                expected_gws.keys().all(|k| gw_map.contains_key(k)),
                "Expected: {:?}, got: {:?}",
                expected_gws,
                gw_map
            );
        }
    }
}
