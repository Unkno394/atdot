use sqlx::PgPool;
use crate::fraud::geoip::{GeoIp, NetworkType};

pub async fn score(
    db:          &PgPool,
    geo:         &GeoIp,
    ip:          Option<&str>,
    visitor_id:  Option<&str>,
    user_agent:  Option<&str>,
    webrtc_ip:   Option<&str>,
    ipv6:        Option<&str>,
    timezone:    Option<&str>,
    fingerprint: Option<&str>,
) -> f32 {
    // WebRTC mismatch is the primary VPN signal. When confirmed, persist the
    // exit-node IP so future requests skip the IPinfo round-trip.
    let webrtc = webrtc_mismatch_risk(geo, ip, webrtc_ip, ipv6);

    let asn_risk    = asn_risk(geo, ip).await;
    let tz_risk     = timezone_country_mismatch(geo, ip, timezone).await;
    let shared_ip   = shared_ip_risk(db, ip).await;
    let ua_risk     = user_agent_risk(user_agent);
    let fingerprint = fingerprint_collision_risk(db, fingerprint).await;
    let temporal    = temporal_pattern_risk(db, ip).await;
    let ip_rotation = visitor_ip_rotation_risk(db, visitor_id).await;

    (webrtc      * 0.25
   + asn_risk    * 0.20
   + shared_ip   * 0.15
   + fingerprint * 0.15
   + ip_rotation * 0.10
   + tz_risk     * 0.08
   + temporal    * 0.05
   + ua_risk     * 0.02)
    .clamp(0.0, 1.0)
}

async fn asn_risk(geo: &GeoIp, ip: Option<&str>) -> f32 {
    let Some(ip) = ip else { return 0.0 };
    match geo.network_type(ip).await {
        NetworkType::VPN         => 0.90,
        NetworkType::Datacenter  => 0.70,
        NetworkType::Mobile      => 0.05,
        NetworkType::Residential => 0.0,
        NetworkType::Unknown     => 0.20,
    }
}

async fn timezone_country_mismatch(
    geo:      &GeoIp,
    ip:       Option<&str>,
    timezone: Option<&str>,
) -> f32 {
    let (Some(ip), Some(tz)) = (ip, timezone) else { return 0.0 };
    let Some(country) = geo.country(ip).await else { return 0.0 };
    let Some(tz_country) = timezone_to_country(tz) else { return 0.0 };
    if country != tz_country { 0.70 } else { 0.0 }
}

fn timezone_to_country(tz: &str) -> Option<&'static str> {
    match tz {
        t if t.starts_with("Europe/Moscow")
          || t.starts_with("Europe/Kaliningrad")  => Some("RU"),
        t if t.starts_with("Europe/London")       => Some("GB"),
        t if t.starts_with("Europe/Berlin")
          || t.starts_with("Europe/Hamburg")      => Some("DE"),
        t if t.starts_with("America/New_York")
          || t.starts_with("America/Los_Angeles") => Some("US"),
        t if t.starts_with("Asia/Shanghai")
          || t.starts_with("Asia/Chongqing")      => Some("CN"),
        t if t.starts_with("Europe/Riga")         => Some("LV"),
        _ => None,
    }
}

async fn shared_ip_risk(db: &PgPool, ip: Option<&str>) -> f32 {
    let Some(ip) = ip else { return 0.1 };
    if ip.is_empty() || ip == "127.0.0.1" { return 0.0; }

    let unique: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(DISTINCT visitor_id) as "c!"
           FROM events
           WHERE ip = $1
             AND timestamp > NOW() - INTERVAL '24 hours'
             AND visitor_id IS NOT NULL"#,
        ip
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    match unique {
        0..=2    => 0.0,
        3..=10   => 0.15,
        11..=50  => 0.40,
        51..=200 => 0.70,
        _        => 0.90,
    }
}

async fn visitor_ip_rotation_risk(db: &PgPool, visitor_id: Option<&str>) -> f32 {
    let Some(vid) = visitor_id else { return 0.0 };

    let ip_count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(DISTINCT ip) as "c!"
           FROM events
           WHERE visitor_id = $1
             AND timestamp > NOW() - INTERVAL '7 days'"#,
        vid
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    match ip_count {
        0..=3  => 0.0,
        4..=8  => 0.20,
        9..=20 => 0.55,
        _      => 0.85,
    }
}

async fn fingerprint_collision_risk(db: &PgPool, fingerprint: Option<&str>) -> f32 {
    let Some(fp) = fingerprint else { return 0.0 };

    let collisions: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(DISTINCT visitor_id) as "c!"
           FROM events
           WHERE payload->>'fingerprint_id' = $1
             AND timestamp > NOW() - INTERVAL '7 days'"#,
        fp
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    match collisions {
        0..=2   => 0.0,
        3..=10  => 0.30,
        11..=30 => 0.65,
        _       => 0.90,
    }
}

async fn temporal_pattern_risk(db: &PgPool, ip: Option<&str>) -> f32 {
    let Some(ip) = ip else { return 0.0 };

    let active_hours: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(DISTINCT EXTRACT(HOUR FROM timestamp)) as "c!"
           FROM events
           WHERE ip = $1
             AND timestamp > NOW() - INTERVAL '7 days'"#,
        ip
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    match active_hours {
        0..=8   => 0.0,
        9..=14  => 0.15,
        15..=20 => 0.40,
        _       => 0.75,
    }
}

fn webrtc_mismatch_risk(
    geo:        &GeoIp,
    request_ip: Option<&str>,
    webrtc_ip:  Option<&str>,
    ipv6:       Option<&str>,
) -> f32 {
    if let (Some(req), Some(wrt)) = (request_ip, webrtc_ip) {
        if !is_private_ip(wrt) && ip_prefix(req) != ip_prefix(wrt) {
            // Confirm and remember the VPN exit node for instant future detection.
            geo.mark_vpn_ip(req);
            return 0.85;
        }
    }

    if let (Some(_), Some(v6)) = (request_ip, ipv6) {
        if !is_private_ip(v6) && !v6.is_empty() {
            return 0.60;
        }
    }

    0.0
}

fn is_private_ip(ip: &str) -> bool {
    ip.starts_with("192.168.")
        || ip.starts_with("10.")
        || ip.starts_with("172.16.")
        || ip.starts_with("127.")
        || ip.starts_with("::1")
        || ip.starts_with("fc")
        || ip.starts_with("fd")
}

fn ip_prefix(ip: &str) -> Option<(&str, &str)> {
    let mut parts = ip.splitn(3, '.');
    match (parts.next(), parts.next()) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    }
}

fn user_agent_risk(ua: Option<&str>) -> f32 {
    let Some(ua) = ua else { return 0.5 };
    let ua_lower = ua.to_lowercase();

    if ua_lower.contains("headless")
        || ua_lower.contains("phantomjs")
        || ua_lower.contains("selenium")
        || ua_lower.contains("webdriver")
        || ua_lower.contains("puppeteer")
        || ua.is_empty()
    {
        return 0.95;
    }

    if !ua_lower.contains("mozilla") && !ua_lower.contains("webkit") {
        return 0.6;
    }

    0.0
}