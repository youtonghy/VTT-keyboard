use crate::settings::PrivacySettings;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use thiserror::Error;
use url::{Host, Url};

#[derive(Debug, Error)]
pub enum PrivacyError {
    #[error("{0}")]
    Blocked(String),
}

pub fn ensure_external_network_allowed(
    privacy: &PrivacySettings,
    context: &str,
) -> Result<(), PrivacyError> {
    if !privacy.enabled {
        return Ok(());
    }
    Err(PrivacyError::Blocked(blocked_message(context)))
}

pub fn ensure_url_allowed(
    privacy: &PrivacySettings,
    url: &str,
    context: &str,
) -> Result<(), PrivacyError> {
    if !privacy.enabled {
        return Ok(());
    }
    if is_local_or_lan_url(url) {
        return Ok(());
    }
    Err(PrivacyError::Blocked(format!(
        "{}。当前地址不是本机或局域网地址: {}",
        blocked_message(context),
        public_host_label(url).unwrap_or_else(|| url.trim().to_string())
    )))
}

pub fn blocked_message(context: &str) -> String {
    let context = context.trim();
    if context.is_empty() {
        "隐私模式已启用，已阻止访问公网。请使用本机或局域网内的本地模型服务。".to_string()
    } else {
        format!("隐私模式已启用，已阻止{context}访问公网。请使用本机或局域网内的本地模型服务。")
    }
}

pub fn is_local_or_lan_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value.trim()) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") {
        return false;
    }
    match url.host() {
        Some(Host::Ipv4(ip)) => is_local_or_lan_ip(IpAddr::V4(ip)),
        Some(Host::Ipv6(ip)) => is_local_or_lan_ip(IpAddr::V6(ip)),
        Some(Host::Domain(domain)) => is_local_domain(domain),
        None => false,
    }
}

fn public_host_label(value: &str) -> Option<String> {
    Url::parse(value.trim())
        .ok()
        .and_then(|url| url.host_str().map(ToString::to_string))
}

fn is_local_or_lan_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_local_or_lan_ipv4(ip),
        IpAddr::V6(ip) => is_local_or_lan_ipv6(ip),
    }
}

fn is_local_or_lan_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_unspecified()
}

fn is_local_or_lan_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()
        || ip.is_unspecified()
        || is_unique_local_ipv6(ip)
        || is_unicast_link_local_ipv6(ip)
}

fn is_unique_local_ipv6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_unicast_link_local_ipv6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn is_local_domain(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".lan")
        || host == "host.docker.internal"
        || host == "gateway.docker.internal"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_and_lan_urls_are_allowed() {
        for value in [
            "http://127.0.0.1:28765",
            "http://localhost:8080/v1",
            "http://192.168.1.20:8000",
            "http://10.0.0.2",
            "http://172.16.0.5",
            "http://169.254.10.20",
            "http://[::1]:8000",
            "http://[fd12:3456::1]:8000",
            "http://speaker.local:8000",
            "ws://nas.lan:8765",
        ] {
            assert!(is_local_or_lan_url(value), "{value}");
        }
    }

    #[test]
    fn public_urls_are_blocked() {
        for value in [
            "https://api.openai.com/v1",
            "wss://dashscope.aliyuncs.com/api-ws/v1/inference",
            "https://8.8.8.8",
            "http://example.com",
            "file:///tmp/model",
        ] {
            assert!(!is_local_or_lan_url(value), "{value}");
        }
    }

    #[test]
    fn privacy_disabled_allows_public_url() {
        let privacy = PrivacySettings { enabled: false };
        assert!(ensure_url_allowed(&privacy, "https://api.openai.com/v1", "测试").is_ok());
    }

    #[test]
    fn privacy_enabled_blocks_public_url() {
        let privacy = PrivacySettings { enabled: true };
        let error = ensure_url_allowed(&privacy, "https://api.openai.com/v1", "测试")
            .expect_err("public url should be blocked")
            .to_string();
        assert!(error.contains("隐私模式已启用"));
        assert!(error.contains("api.openai.com"));
    }
}
