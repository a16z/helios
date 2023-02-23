use discv5::{Discv5Config, Discv5ConfigBuilder, Enr};
use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub listen_addr: std::net::IpAddr,
    pub libp2p_port: u16,
    pub discovery_port: u16,
    pub target_peers: usize,
    #[serde(skip)]
    pub discv5_config: Discv5Config,
    pub boot_nodes_enr: Vec<Enr>,
    pub disable_discovery: bool,
}

impl Default for Config {
    // TODO: Consider defaults more closely these are lighthouse defaults practically
    fn default() -> Self {
        let filter_rate_limiter = Some(
            discv5::RateLimiterBuilder::new()
                .total_n_every(10, Duration::from_secs(1))
                .ip_n_every(9, Duration::from_secs(1))
                .node_n_every(8, Duration::from_secs(1))
                .build()
                .expect("The total rate limit has been specified"),
        );

        let discv5_config = Discv5ConfigBuilder::new()
            .enable_packet_filter()
            .session_cache_capacity(5000)
            .request_timeout(Duration::from_secs(1))
            .query_peer_timeout(Duration::from_secs(2))
            .query_timeout(Duration::from_secs(30))
            .request_retries(1)
            .enr_peer_update_min(10)
            .query_parallelism(5)
            .disable_report_discovered_peers()
            .ip_limit()
            .incoming_bucket_limit(8)
            .filter_rate_limiter(filter_rate_limiter)
            .filter_max_bans_per_ip(Some(5))
            .filter_max_nodes_per_ip(Some(10))
            //.table_filter(|enr| enr.ip4().map_or(false, |ip| is_global(&ip)))
            .ban_duration(Some(Duration::from_secs(3600)))
            .ping_interval(Duration::from_secs(300))
            .build();

        Config {
            listen_addr: "0.0.0.0".parse().expect("valid ip address"),
            libp2p_port: 9000,
            discovery_port: 9000,
            target_peers: 50,
            discv5_config,
            boot_nodes_enr: vec![],
            disable_discovery: false,
        }
    }
}
