use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct GenericResponse {
    pub status: u16,
    pub message: String,
}

#[allow(dead_code)]
pub(crate) const PROXY_IPS: [&str; 5] = ["cdn-all.xn--b6gac.eu.org", "cdn.xn--b6gac.eu.org", "cdn-b100.xn--b6gac.eu.org", "edgetunnel.anycast.eu.org", "cdn.anycast.eu.org"];

#[allow(dead_code)]
pub(crate) static mut PROXY_IP: &str = "";

#[allow(dead_code)]
pub(crate) const DOH_URL: &str = "https://sky.rethinkdns.com/1:-Pf_____9_8A_AMAIgE8kMABVDDmKOHTAKg=";

pub(crate) const FAKE_HOSTS: [&str; 8] = ["www.fmprc.gov.cn", "www.xuexi.cn", "www.gov.cn", "mail.gov.cn", "www.mofcom.gov.cn", "www.gfbzb.gov.cn", "www.miit.gov.cn", "www.12377.cn"];