use dashmap::DashSet;
use std::env;
use std::sync::Arc;

#[derive(serde::Deserialize)]
pub struct IpInfo {
    pub country_code: Option<String>,
    pub asn:          Option<String>,
    pub as_name:      Option<String>,
}

// Top-30 datacenter ASN numbers (numeric part of "ASxxxxx").
const DATACENTER_ASNS: &[u32] = &[
    14618, 16509,  // Amazon AWS
    15169,         // Google Cloud
     8075,         // Microsoft Azure
    14061,         // DigitalOcean
    20473,         // Vultr / Choopa
    63949,         // Linode / Akamai
    24940,         // Hetzner
    16276,         // OVH
    36351,         // SoftLayer / IBM
    13335,         // Cloudflare
    54113,         // Fastly
    20940,         // Akamai Technologies
     8560,         // IONOS / 1&1
    46606,         // Unified Layer / Bluehost
    60781,         // LeaseWeb NL
   395954,         // LeaseWeb US
     9009,         // M247
    51167,         // Contabo
    47583,         // Hostinger
    40676,         // Psychz Networks
    32475,         // SingleHop
    22612,         // Namecheap Hosting
    21844,         // ThePlanet.com
    26347,         // Media Temple
    29990,         // Rackspace
    35916,         // MULTACOM
    33070,         // RagingWire / NTT
    19318,         // Interserver
    55293,         // A2 Hosting
];

// Known VPN providers in as_name — small curated list only.
const VPN_ORG_KEYWORDS: &[&str] = &[
    "nordvpn", "expressvpn", "mullvad", "protonvpn", "surfshark",
    "tor project", "ipvanish", "cyberghost", "privatevpn", "perfect privacy",
    "hide.me", "windscribe", "purevpn", "tunnelbear",
];

pub struct GeoIp {
    token:             String,
    pub known_vpn_ips: Arc<DashSet<String>>,
    vpn_store:         sled::Tree,
}

impl GeoIp {
    pub fn new(vpn_store: sled::Tree) -> Self {
        // Warm the in-memory set from persisted data.
        let known_vpn_ips = Arc::new(DashSet::new());
        for item in vpn_store.iter().flatten() {
            if let Ok(ip) = std::str::from_utf8(&item.0) {
                known_vpn_ips.insert(ip.to_string());
            }
        }
        Self {
            token: env::var("IPINFO_TOKEN").unwrap_or_default(),
            known_vpn_ips,
            vpn_store,
        }
    }

    /// Mark an IP as a confirmed VPN exit node (learned via WebRTC mismatch).
    /// Persists to sled so the entry survives server restarts.
    pub fn mark_vpn_ip(&self, ip: &str) {
        if self.known_vpn_ips.insert(ip.to_string()) {
            // Only write to disk when it's a new entry.
            let _ = self.vpn_store.insert(ip.as_bytes(), &[]);
        }
    }

    pub async fn lookup(&self, ip: &str) -> Option<IpInfo> {
        let url = format!(
            "https://api.ipinfo.io/lite/{}?token={}",
            ip, self.token
        );
        let out = tokio::process::Command::new("curl")
            .args(["-sf", "--max-time", "3", &url])
            .output()
            .await
            .ok()?;
        if !out.status.success() { return None; }
        serde_json::from_slice(&out.stdout).ok()
    }

    pub async fn country(&self, ip: &str) -> Option<String> {
        self.lookup(ip).await?.country_code
    }

    pub async fn network_type(&self, ip: &str) -> NetworkType {
        // Fastest path: already confirmed via WebRTC.
        if self.known_vpn_ips.contains(ip) {
            return NetworkType::VPN;
        }

        let Some(info) = self.lookup(ip).await else {
            return NetworkType::Unknown;
        };

        // Datacenter: authoritative ASN number check.
        if let Some(asn_str) = &info.asn {
            if let Ok(n) = asn_str.trim_start_matches("AS").parse::<u32>() {
                if DATACENTER_ASNS.contains(&n) {
                    return NetworkType::Datacenter;
                }
            }
        }

        // VPN: small curated as_name list.
        let org = info.as_name.unwrap_or_default().to_lowercase();
        if VPN_ORG_KEYWORDS.iter().any(|kw| org.contains(kw)) {
            return NetworkType::VPN;
        }

        if org.contains("mobile")
            || org.contains("cellular")
            || org.contains("mts")
            || org.contains("beeline")
            || org.contains("megafon")
        {
            return NetworkType::Mobile;
        }

        NetworkType::Residential
    }
}

#[derive(Debug, PartialEq)]
pub enum NetworkType {
    Residential,
    Mobile,
    Datacenter,
    VPN,
    Unknown,
}
