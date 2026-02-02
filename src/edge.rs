use std::fmt;
use std::{env, process};

use reqwest::blocking::Response;
use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

use crate::config::Config;

pub fn new_client() -> EdgeClient {
    match load_client() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}

pub fn load_client() -> anyhow::Result<EdgeClient> {
    let config = Config::load();

    let url = env::var("EDGE_URL")
        .ok()
        .or_else(|| config.get_current_context().map(|c| c.url.clone()))
        .ok_or_else(|| {
            anyhow::anyhow!("No URL provided, either via config or env var. Try:\nedgectl login")
        })?;

    if let Ok(password) = env::var("EDGE_PASSWORD") {
        let client = EdgeClient::with_url(&url);
        let username = env::var("EDGE_USER").unwrap_or_else(|_| "admin".to_owned());
        client.login(username, password)?;
        return Ok(client);
    }

    let token = env::var("EDGE_TOKEN")
        .ok()
        .or_else(|| config.get_current_context().map(|c| c.token.clone()))
        .ok_or_else(|| {
            anyhow::anyhow!("No credentials found. Run 'edgectl login' to authenticate.")
        })?;

    Ok(EdgeClient::with_url_and_token(&url, &token))
}

pub struct EdgeClient {
    pub client: reqwest::blocking::Client,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRespUser {
    // pub id: String,
    pub username: String,
    pub role: String,
    // pub group: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTokenInit {
    pub name: String,
    pub role: String,
    pub expires_at: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiToken {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    pub role: String,
    pub expires_at: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum InputAdminStatus {
    On,
    Off,
}

impl fmt::Display for InputAdminStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::On => write!(f, "on"),
            Self::Off => write!(f, "off"),
        }
    }
}

impl<'de> Deserialize<'de> for InputAdminStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(Self::Off),
            1 => Ok(Self::On),
            _ => Err(D::Error::unknown_variant(&value.to_string(), &["0", "1"])),
        }
    }
}

impl Serialize for InputAdminStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Off => serializer.serialize_u8(0),
            Self::On => serializer.serialize_u8(1),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ThumbnailMode {
    None,
    Core,
    Edge,
}

impl fmt::Display for ThumbnailMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThumbnailMode::None => write!(f, "none"),
            ThumbnailMode::Edge => write!(f, "edge"),
            ThumbnailMode::Core => write!(f, "core"),
        }
    }
}

impl<'de> Deserialize<'de> for ThumbnailMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(ThumbnailMode::None),
            1 => Ok(ThumbnailMode::Edge),
            2 => Ok(ThumbnailMode::Core),
            _ => Err(D::Error::unknown_variant(&value.to_string(), &["0", "2"])),
        }
    }
}

impl Serialize for ThumbnailMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            ThumbnailMode::None => serializer.serialize_u8(0),
            ThumbnailMode::Edge => serializer.serialize_u8(1),
            ThumbnailMode::Core => serializer.serialize_u8(2),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InputMetrics {
    pub rist_metrics: Option<Vec<RistMetric>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RistMetric {
    #[serde(rename = "type")]
    pub metric_type: String,
    pub state: Option<String>,
    pub channel_id: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub channel_id: u32,
    pub active: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub id: String,
    pub name: String,
    pub admin_status: InputAdminStatus,
    pub owner: String, // group
    // broadcast_standard: String,
    pub buffer_size: u32,
    // channel_group: u8,
    pub created_at: String, // really a date
    pub updated_at: String, // really a date
    pub preview_settings: Option<PreviewSettings>,
    pub thumbnail_mode: ThumbnailMode, // 0,1 or 2
    pub tr101290_enabled: bool,
    pub appliances: Vec<InputAppliance>,
    pub health: InputHealth,
    pub ports: Option<Vec<InputPort>>,
    pub metrics: Option<InputMetrics>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewInput {
    pub name: String,
    pub tr101290_enabled: bool,
    pub broadcast_standard: String,
    pub thumbnail_mode: ThumbnailMode,
    pub video_preview_mode: String, // "on demand" | "off"
    pub admin_status: InputAdminStatus,
    pub ports: Vec<NewInputPort>,
    pub buffer_size: u16,
    pub max_bitrate: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derive_from: Option<DerivableInputSource>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSettings {
    pub mode: String, // enum ?
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InputHealth {
    pub state: String,
    pub title: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InputAppliance {
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivableInputSource {
    pub parent_input: String,
    pub delay: u32,
    pub ingest_transform: Option<IngestTransform>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum IngestTransform {
    #[serde(rename_all = "camelCase")]
    #[serde(rename = "mpts-demux")]
    MptsDemuxTransform {
        services: Vec<u16>,
        pid_map: Option<PidMap>,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PidMap {
    pub rules: Vec<PIDRule>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "action")]
pub enum PIDRule {
    #[serde(rename_all = "camelCase")]
    Map { pid: u16, dest_pid: u16 },
    #[serde(rename_all = "camelCase")]
    Delete { pid: u16 },
    #[serde(rename_all = "camelCase")]
    SetNull { pid: u16 },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub name: String,
    pub id: String,
    pub admin_status: OutputAdminStatus,
    // The ID of the group
    pub group: String,
    // The ID of the input
    pub input: Option<String>,
    pub health: Option<OutputHealth>,
    pub redundancy_mode: Option<OutputRedundancyMode>,
    pub delay: Option<u32>, // in ms
    pub delay_mode: Option<OutputDelayMode>,
    pub created_at: String, // really a date
    pub updated_at: String, // really a date
    pub misconfigured: Option<bool>,
    pub alarms: Option<Vec<OutputAlarm>>,
    pub appliances: Vec<LimitedAppliance>,
    pub ports: Vec<OutputPort>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewOutput {
    pub name: String,
    pub admin_status: OutputAdminStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay_mode: Option<OutputDelayMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub input: String,
    pub ports: Vec<OutputPort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redundancy_mode: Option<OutputRedundancyMode>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum OutputAdminStatus {
    Off = 0,
    On = 1,
}

impl fmt::Display for OutputAdminStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::On => write!(f, "on"),
            Self::Off => write!(f, "off"),
        }
    }
}

impl Serialize for OutputAdminStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Off => serializer.serialize_u8(0),
            Self::On => serializer.serialize_u8(1),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub enum OutputRedundancyMode {
    None = 0,
    Failover = 1,
    Active = 2,
}

impl fmt::Display for OutputRedundancyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Failover => write!(f, "failover"),
            Self::Active => write!(f, "active"),
        }
    }
}

impl<'de> Deserialize<'de> for OutputRedundancyMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Failover),
            2 => Ok(Self::Active),
            _ => Err(D::Error::unknown_variant(
                &value.to_string(),
                &["0", "1", "2"],
            )),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub enum OutputDelayMode {
    BasedOnArrivalTime = 1,
    BasedOnOriginTime = 2,
}

impl fmt::Display for OutputDelayMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BasedOnArrivalTime => write!(f, "On Arrival"),
            Self::BasedOnOriginTime => write!(f, "On Origin"),
        }
    }
}

impl<'de> Deserialize<'de> for OutputDelayMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            1 => Ok(Self::BasedOnArrivalTime),
            2 => Ok(Self::BasedOnOriginTime),
            _ => Err(D::Error::unknown_variant(&value.to_string(), &["1", "2"])),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OutputAlarm {
    pub alarm_cause: String,
    pub alarm_severity: String,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AlarmWithImpact {
    pub alarm_id: String,
    pub alarm_cause: String,
    pub alarm_severity: String,
    #[serde(rename = "type")]
    pub alarm_type: String,
    pub object_name: String,
    pub repeat_count: u32,
    pub affected_input: Option<String>,
    pub affected_output: Option<String>,
    pub appliance_name: Option<String>,
    pub input_id: Option<String>,
    pub input_name: Option<String>,
    pub object_purpose: Option<String>,
    pub output_id: Option<String>,
    pub output_name: Option<String>,
    pub physical_port_id: Option<String>,
    pub raised_at: Option<String>,
    pub region: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Alarm {
    pub alarm_id: String,
    pub alarm_cause: String,
    pub alarm_severity: String,
    #[serde(rename = "type")]
    pub alarm_type: String,
    pub object_name: String,
    pub repeat_count: u32,
    pub appliance_name: Option<String>,
    pub input_id: Option<String>,
    pub input_name: Option<String>,
    pub object_purpose: Option<String>,
    pub output_id: Option<String>,
    pub output_name: Option<String>,
    pub physical_port_id: Option<String>,
    pub raised_at: Option<String>,
    pub cleared_at: Option<String>,
    pub region: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewGroup {
    pub name: String,
    pub appliance_secret: String,
}

impl<'de> Deserialize<'de> for OutputAdminStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(Self::Off),
            1 => Ok(Self::On),
            _ => Err(D::Error::unknown_variant(&value.to_string(), &["0", "1"])),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OutputHealth {
    pub state: OutputHealthState,
    pub title: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum OutputHealthState {
    NotConfigured,
    MetricsMissing,
    Tr101290Priority1Error,
    ReducedRedundancy,
    AllOk,
    NotAcknowledged,
    InputError,
    OutputError,
    Alarm,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "mode")]
pub enum OutputPort {
    Udp(UdpOutputPort),
    Rtp(RtpOutputPort),
    Rist(RistOutputPort),
    Srt(SrtOutputPort),
    Rtmp(RtmpOutputPort),
    Zixi(ZixiOutputPort),
    Unix(UnixOutputPort),
    Sdi(SdiOutputPort),
    Asi(AsiOutputPort),
    MatroxSdi(MatroxSdiOutputPort),
    ComprimatoSdi(ComprimatoSdiOutputPort),
    ComprimatoNdi(ComprimatoNdiOutputPort),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UdpOutputPort {
    pub address: String,
    pub port: u16,
    pub physical_port: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_address: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RtpOutputPort {
    pub address: String,
    pub port: u16,
    pub physical_port: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fec: Option<OutputPortFec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fec_rows: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fec_cols: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_address: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum OutputPortFec {
    #[serde(rename = "1D")]
    Fec1D,
    #[serde(rename = "2D")]
    Fec2D,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RistOutputPort {
    pub address: String,
    pub port: u16,
    pub physical_port: String,
    pub profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_address: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "srtMode")]
pub enum SrtOutputPort {
    Listener(SrtListenerOutputPort),
    Caller(SrtCallerOutputPort),
    Rendezvous(SrtRendezvousOutputPort),
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SrtListenerOutputPort {
    pub local_ip: String,
    pub local_port: u16,
    pub physical_port: String,
    pub latency: u16,
    pub pbkeylen: SrtKeylen,
    pub rate_limiting: SrtRateLimiting,
    // Required on core nodes
    pub whitelist_cidr_block: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SrtKeylen {
    #[serde(rename = "16")]
    Aes128,
    #[serde(rename = "24")]
    Aes192,
    #[serde(rename = "32")]
    Aes256,
    #[serde(rename = "-1")]
    None,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SrtRateLimiting {
    #[serde(rename = "Not enforced")]
    NotEnforced,
    #[serde(rename = "Absolute")]
    Absolute,
    #[serde(rename = "Relative to input")]
    RelativeToInput,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SrtCallerOutputPort {
    pub physical_port: String,
    pub remote_ip: String,
    pub remote_port: u16,

    pub latency: u16,
    pub pbkeylen: SrtKeylen,
    pub rate_limiting: SrtRateLimiting,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SrtRendezvousOutputPort {
    pub local_ip: String,
    pub remote_ip: String,
    pub remote_port: u16, // Both local and remote port
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RtmpOutputPort {
    pub rtmp_destination_address: String,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "zixiMode")]
pub enum ZixiOutputPort {
    Pull(ZixiPullOutputPort),
    Push(ZixiPushOutputPort),
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ZixiPullOutputPort {
    pub stream_id: String,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ZixiPushOutputPort {
    pub stream_id: String,
    pub link_set_1: Vec<ZixiLink>, // ZixiLinkSet: length 1, 2 or 3
    pub link_set_2: Option<Vec<ZixiLink>>, // ZixiLinkSet: length 1, 2 or 3
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ZixiLink {
    // pub local_ip: String,
    pub remote_ip: String,
    pub remote_port: u16,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UnixOutputPort {}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SdiOutputPort {
    pub physical_port: String,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AsiOutputPort {}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MatroxSdiOutputPort {}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ComprimatoSdiOutputPort {}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ComprimatoNdiOutputPort {}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Appliance {
    pub name: String,
    pub hostname: String,
    pub contact: String,
    pub serial: String,
    pub id: String,
    pub version: ApplianceVersion,
    // lastMessageAt
    pub last_registered_at: Option<String>, // iso8601/rfc3339
    pub health: Option<ApplianceHealth>,
    pub physical_ports: Vec<AppliancePhysicalPort>,
    pub region: RegionReference,
    pub secondary_region: Option<RegionReference>,
    #[serde(rename = "type")]
    pub kind: String,
    // owner is the group id
    pub owner: String,
    pub alarms: Vec<ApplianceAlarm>,
    // features
    // logLevel
    // collectHostMetrics
    // ristserverLogLevel
    // settings
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LimitedAppliance {
    // pub id: String,
    pub name: String,
    // #[serde(rename = "type")]
    // pub kind: String,
    // region { id, name }
    // secondaryRegion
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApplianceVersion {
    pub control_image_version: Option<String>,
    pub control_software_version: String,
    pub data_image_version: Option<String>,
    pub data_software_version: Option<String>,
    pub va_version: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApplianceHealth {
    pub title: String,
    pub state: ApplianceHealthState,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ApplianceHealthState {
    Connected,
    Missing,
    NeverConnected,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApplianceAlarm {
    pub alarm_cause: String,
    pub alarm_severity: String, // critical | major | minor | warning | cleared
    pub time: String,
    // #[serde(rename = "type")]
    // pub kind: String, // va | edge | backend | backend-monitor | prometheus
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppliancePhysicalPort {
    pub id: String,
    pub name: String,
    // example: 86:71:48:3b:e3:1b
    // pub mac: String,
    // pub index: String,
    pub port_type: AppliancePortType,
    // pub appliance: { id, name, type, version }
    // owner is the group id
    // pub owner: Strig,
    pub addresses: Vec<PhysicalPortAddress>,
    pub networks: Vec<ApplianceNetwork>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum AppliancePortType {
    Ip,
    Coax,
    Videon,
    Ndi,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalPortAddress {
    pub address: String,
    // pub netmask: String,
    pub public_address: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApplianceNetwork {
    // pub id: String,
    pub name: String,
    // pub enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplianceInput {
    // pub appliance_id: String,
    // pub appliance_name: String,
    pub input_name: String,
    pub input_group: String,
    pub input_admin_status: InputAdminStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplianceOutput {
    // pub appliance_id: String,
    // pub appliance_name: String,
    // pub input_name: String,
    // pub input_group: String,
    // pub input_admin_status: InputAdminStatus,
    // pub output_id: String,
    pub output_name: String,
    pub output_group: String,
    pub output_admin_status: OutputAdminStatus,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InputPort {
    pub id: String,
    pub copies: u8,
    // "internalStreamId": 20485,
    pub physical_port: String,
    // "appliance": "a2253ca5-78a4-45a5-ae96-d8d4091b49ea",
    // "priority": 0,
    pub mode: String,
    // "resolution": "1920x1080",
    // "scanMode": "progressive",
    // "frameRate": "30",
    // "timestampResolution": "seconds"
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "mode")]
pub enum NewInputPort {
    Rtp(RtpInputPort),
    Udp(UdpInputPort),
    Sdi(SdiInputPort),
    Srt(SrtInputPort),
    Rist(RistInputPort),
    Generator(GeneratorInputPort),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtpInputPort {
    pub copies: u8,
    pub physical_port: String,
    pub address: String,
    pub port: u16,
    pub fec: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multicast_address: Option<String>,
    pub whitelist_cidr_block: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpInputPort {
    pub copies: u8,
    pub physical_port: String,
    pub address: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multicast_address: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "srtMode")]
pub enum SrtInputPort {
    #[serde(rename_all = "camelCase")]
    Caller {
        physical_port: String,
        remote_ip: String,
        remote_port: u16,
        latency: u16,
        reduced_bitrate_detection: bool,
        unrecovered_packets_detection: bool,
    },
    #[serde(rename_all = "camelCase")]
    Listener {
        physical_port: String,
        latency: u16,
        local_ip: String,
        local_port: u16,
        reduced_bitrate_detection: bool,
        unrecovered_packets_detection: bool,
        whitelist_cidr_block: Option<Vec<String>>,
    },
    Rendezvous,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RistInputPort {
    pub physical_port: String,
    pub address: String,
    pub port: u16,
    pub profile: String, // can only be 'simple'
    pub whitelist_cidr_block: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SdiInputPort {
    pub copies: u8,
    pub physical_port: String,
    pub encoder_settings: SdiEncoderSettings,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SdiEncoderSettings {
    pub video_codec: String,
    pub total_bitrate: u64,
    pub gop_size_frames: u16,
    pub audio_streams: Vec<SdiEncoderAudioStream>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SdiEncoderAudioStream {
    pub codec: String,
    pub pair: u8,
    pub bitrate: u16,
    #[serde(rename = "type")]
    pub kind: String, // enum: stereo | mono
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorInputPort {
    pub copies: u8,
    pub physical_port: String,
    pub bitrate: GeneratorBitrate,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum GeneratorBitrate {
    Vbr,
    Cbr(GeneratorBitrateCBR),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorBitrateCBR {
    pub bitrate: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OutputRecipientList {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewOutputRecipientList {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub group: String,
    pub add_outputs: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GroupRecipientList {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewGroupRecipientList {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub group: String,
    pub add_groups: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    pub appliance_secret: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Port {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub id: String,
    pub name: String,
    #[serde(rename = "default_region")]
    pub default_region: Option<bool>,
    pub external: ExternalRegionMode,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRegion {
    pub name: String,
    pub external: ExternalRegionMode,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RegionReference {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAppliancePayload {
    pub region: RegionReference,
    pub secondary_region: Option<RegionReference>,
}

#[derive(Debug, Clone)]
pub enum ExternalRegionMode {
    Core = 0,
    ExternalK8s = 1,
    External = 2,
}

impl<'de> Deserialize<'de> for ExternalRegionMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(Self::Core),
            1 => Ok(Self::ExternalK8s),
            2 => Ok(Self::External),
            _ => Err(D::Error::unknown_variant(
                &value.to_string(),
                &["0", "1", "2"],
            )),
        }
    }
}

impl Serialize for ExternalRegionMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Core => serializer.serialize_u8(0),
            Self::ExternalK8s => serializer.serialize_u8(1),
            Self::External => serializer.serialize_u8(2),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesNode {
    pub name: String,
    pub status: String,
    #[serde(rename = "externalIP")]
    pub external_ip: String,
    #[serde(rename = "internalIP")]
    pub internal_ip: String,
    pub hostname: String,
    pub roles: Vec<KubernetesRole>,
    pub kubelet_version: Option<String>,
    pub region: KubernetesNodeRegion,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct KubernetesNodeRegion {
    // pub id: String,
    pub name: String,
    pub external: ExternalRegionMode,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum KubernetesRole {
    Core,
    Thumb,
    Video,
    VideoStandby,
    EdgeConnect,
    LoadSimulator,
}

impl fmt::Display for KubernetesRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core => write!(f, "core"),
            Self::Thumb => write!(f, "thumb"),
            Self::Video => write!(f, "video"),
            Self::VideoStandby => write!(f, "video-standby"),
            Self::EdgeConnect => write!(f, "edge-connect"),
            Self::LoadSimulator => write!(f, "load-simulator"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Tunnel {
    pub id: u32,
    pub r#type: TunnelType,
    pub client_name: String,
    pub server_name: String,
    pub inputs: Vec<TunnelInput>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TunnelInput {
    // pub id: String,
    // pub name: String,
}

#[derive(Debug, Clone)]
pub enum TunnelType {
    External,
    Internal,
    InterRegion,
}

impl<'de> Deserialize<'de> for TunnelType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            1 => Ok(Self::External),
            2 => Ok(Self::Internal),
            3 => Ok(Self::InterRegion),
            _ => Err(D::Error::unknown_variant(
                &value.to_string(),
                &["1", "2", "3"],
            )),
        }
    }
}

impl Serialize for TunnelType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            Self::External => 1u8,
            Self::Internal => 2u8,
            Self::InterRegion => 3u8,
        };
        serializer.serialize_u8(value)
    }
}

impl fmt::Display for TunnelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::External => write!(f, "external"),
            Self::Internal => write!(f, "internal"),
            Self::InterRegion => write!(f, "inter-region"),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    pub product: Product,
    pub build_time: String,
    pub release: String,
    pub commit: String,
    pub pipeline: String,
    // entitlements_public_key: String,
    // entitlements_public_key_hash: String,
}

#[derive(Debug, Clone)]
pub enum Product {
    NimbraEdge,
    ConnectIt,
}

impl<'de> Deserialize<'de> for Product {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_ref() {
            "705be4c2-0e82-4356-9ce2-6484c116796f" => Ok(Self::NimbraEdge),
            "d0b70d6d-8db6-4524-8d4f-7ee716af241a" => Ok(Self::ConnectIt),
            _ => Err(D::Error::unknown_variant(
                &value.to_string(),
                &[
                    "705be4c2-0e82-4356-9ce2-6484c116796f",
                    "d0b70d6d-8db6-4524-8d4f-7ee716af241a",
                ],
            )),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSettings {
    pub log_level: LogLevel,
    pub va_update_url: String,
    pub default_delay: u32,
    pub default_broadcast_standard: BroadcastStandard,
    pub default_handover_method: HandoverMethod,
    pub zixi_zec_key: Option<String>,
    pub zixi_receiver_key: Option<String>,
    pub zixi_feeder_key: Option<String>,
    pub ntp_server: String,
    pub ntp_enabled: bool,
    pub entitlements: serde_json::Value,
    pub ssl_redirect_enabled: bool,
    pub exp_features: serde_json::Value,
    pub custom_tab_title: String,
    pub alarm_notifications_enabled: bool,
    pub sso: serde_json::Value,
    pub ad_bucket_size_threshold: String,
    pub backup: BackupConf,
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Fatal,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl<'de> Deserialize<'de> for LogLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(Self::Fatal),
            1 => Ok(Self::Error),
            2 => Ok(Self::Warn),
            3 => Ok(Self::Info),
            4 => Ok(Self::Debug),
            5 => Ok(Self::Trace),
            _ => Err(D::Error::unknown_variant(
                &value.to_string(),
                &["0", "1", "2", "3", "4", "5"],
            )),
        }
    }
}

impl Serialize for LogLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Fatal => serializer.serialize_u8(0),
            Self::Error => serializer.serialize_u8(1),
            Self::Warn => serializer.serialize_u8(2),
            Self::Info => serializer.serialize_u8(3),
            Self::Debug => serializer.serialize_u8(4),
            Self::Trace => serializer.serialize_u8(5),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum BroadcastStandard {
    Dvb,
    Atsc,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum HandoverMethod {
    Udp,
    Unix,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BackupConf {
    pub external_s3: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct EdgeQuery<T: Serialize> {
    filter: T,
}

#[derive(Debug)]
pub enum EdgeError {
    RequestError(reqwest::Error),
    NonSuccessStatus(reqwest::StatusCode, EdgeApiError),
    ServerError(reqwest::StatusCode, EdgeApiError),
    ClientError(reqwest::StatusCode, EdgeApiError),
}

impl std::error::Error for EdgeError {}

#[derive(Debug)]
pub enum EdgeApiError {
    ApiError(EdgeErrorResp),
    ParseError(String),
}

#[derive(Deserialize, Debug)]
pub struct EdgeErrorResp {
    title: String,
    detail: EdgeErrorResponseDetail,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum EdgeErrorResponseDetail {
    Detail(String),
    InvalidParameters(Vec<InvalidParameter>),
}

#[derive(Deserialize, Debug)]
struct InvalidParameter {
    name: String,
    reason: String,
}

impl From<Result<EdgeErrorResp, String>> for EdgeApiError {
    fn from(res: Result<EdgeErrorResp, String>) -> Self {
        match res {
            Ok(api_err) => Self::ApiError(api_err),
            Err(e) => Self::ParseError(e),
        }
    }
}

impl std::fmt::Display for EdgeApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::ApiError(api_err) => api_err.fmt(f),
            Self::ParseError(e) => write!(f, "error parsing error: {}", e),
        }
    }
}

impl std::fmt::Display for EdgeErrorResp {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Error: {}, type: {}, details: {}",
            self.title, self.kind, self.detail,
        )
    }
}

impl std::fmt::Display for EdgeErrorResponseDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Detail(s) => s.fmt(f),
            Self::InvalidParameters(params) => write!(
                f,
                "invalid parameters: {}",
                params
                    .iter()
                    .map(|d| format!("{}: {}", d.name, d.reason))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}

impl From<reqwest::Error> for EdgeError {
    fn from(item: reqwest::Error) -> Self {
        Self::RequestError(item)
    }
}

impl std::fmt::Display for EdgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::RequestError(e) => e.fmt(f),
            Self::NonSuccessStatus(s, res) => {
                write!(f, "Unsuccessful HTTP status ({}): {}", s, res)
            }
            Self::ServerError(s, res) => write!(f, "HTTP server error ({}): {}", s, res),
            Self::ClientError(s, res) => write!(f, "HTTP client error ({}): {}", s, res),
        }
    }
}

trait ResponseExt {
    fn error_if_not_success(self) -> Result<Response, EdgeError>;
}

impl ResponseExt for Response {
    fn error_if_not_success(self) -> Result<Self, EdgeError> {
        let status_code = self.status();
        if !status_code.is_success() {
            let content_type = self
                .headers()
                .get("content-type")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_owned());
            let body = self.text().ok();

            let resp = get_edge_error_detail(content_type, body);

            return Err(if status_code.is_client_error() {
                EdgeError::ClientError(status_code, resp.into())
            } else if status_code.is_server_error() {
                EdgeError::ServerError(status_code, resp.into())
            } else {
                EdgeError::NonSuccessStatus(status_code, resp.into())
            });
        }

        Ok(self)
    }
}

fn get_edge_error_detail(
    content_type: Option<String>,
    body: Option<String>,
) -> Result<EdgeErrorResp, String> {
    let Some(body) = body else {
        return Err("Missing body".to_owned());
    };
    let Some(content_type) = content_type else {
        return Err("Failed to decode error response: content-type missing".to_owned());
    };

    if content_type != "application/json" {
        return Err(format!(
            "Decoding error response with content-type {} not supported",
            content_type
        ));
    }

    let json = serde_json::from_str::<EdgeErrorResp>(body.to_owned().as_ref());
    match json {
        Ok(res) => Ok(res),
        Err(e) => Err(format!(
            "Decoding error response as JSON failed, {}, {}",
            e, body
        )),
    }
}

impl EdgeClient {
    pub fn with_url(url: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .cookie_store(true)
            .build()
            .unwrap();

        Self {
            client,
            url: url.to_owned(),
        }
    }

    pub fn with_url_and_token(url: &str, token: &str) -> Self {
        use reqwest::header;
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static("edge-api-token"),
            header::HeaderValue::from_str(token).expect("Token cannot be used as a HTTP header"),
        );

        let client = reqwest::blocking::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        Self {
            client,
            url: url.to_owned(),
        }
    }

    pub fn login(&self, username: String, password: String) -> Result<LoginRespUser, EdgeError> {
        let mut login_data = BTreeMap::new();
        login_data.insert("username", username);
        login_data.insert("password", password);

        #[derive(Debug, Deserialize)]
        struct LoginResp {
            user: LoginRespUser,
        }

        let res = self
            .client
            .post(format!("{}/api/login/", self.url))
            .header("content-type", "application/json")
            .json(&login_data)
            .send()?
            .error_if_not_success()?;

        Ok(res.json::<LoginResp>()?.user)
    }

    pub fn list_inputs(&self) -> Result<Vec<Input>, reqwest::Error> {
        #[derive(Debug, Deserialize)]
        struct InputListResp {
            items: Vec<Input>,
            // total: u32,
        }

        let mut all_inputs = Vec::new();
        let pagesize = 100;
        let mut page = 1;

        loop {
            let skip = (page - 1) * pagesize;
            let res = self
                .client
                .get(format!(
                    r#"{}/api/input/?q={{"filter":{{}},"skip":{},"limit":{}}}"#,
                    self.url, skip, pagesize
                ))
                .header("content-type", "application/json")
                .send()?;

            let resp = res.json::<InputListResp>()?;
            let items_count = resp.items.len();
            all_inputs.extend(resp.items);

            if items_count < pagesize as usize {
                break;
            }
            page += 1;
        }

        Ok(all_inputs)
    }

    pub fn get_input(&self, id: &str) -> Result<Input, reqwest::Error> {
        let res = self
            .client
            .get(format!(r#"{}/api/input/{}"#, self.url, id))
            .header("content-type", "application/json")
            .send()?;

        res.json::<Input>()
    }

    pub fn find_inputs(&self, name: &str) -> Result<Vec<Input>, reqwest::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct InputFilter {
            search_name: String,
        }

        #[derive(Debug, Deserialize)]
        struct InputListResp {
            items: Vec<Input>,
            // total: u32,
        }

        let query = EdgeQuery {
            filter: InputFilter {
                search_name: name.to_owned(),
            },
        };
        let query = serde_json::to_string(&query).expect("Failed to serialize filter as JSON");

        let res = self
            .client
            .get(format!(r#"{}/api/input/?q={}"#, self.url, query))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<InputListResp>()?.items)
    }

    pub fn list_inputs_by_ids(&self, ids: Vec<String>) -> Result<Vec<Input>, reqwest::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct InputFilterByIds {
            ids: Vec<String>,
        }

        #[derive(Debug, Deserialize)]
        struct InputListResp {
            items: Vec<Input>,
            // total: u32,
        }

        let query = EdgeQuery {
            filter: InputFilterByIds { ids },
        };
        let query = serde_json::to_string(&query).expect("Failed to serialize filter as JSON");

        let res = self
            .client
            .get(format!(r#"{}/api/input/?q={}"#, self.url, query))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<InputListResp>()?.items)
    }

    pub fn create_input(&self, input: NewInput) -> Result<(), EdgeError> {
        self.client
            .post(format!("{}/api/input/", self.url))
            .header("content-type", "application/json")
            .json(&input)
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn delete_input(&self, id: &str) -> Result<(), EdgeError> {
        self.client
            .delete(format!("{}/api/input/{}", self.url, id))
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn list_outputs(&self) -> Result<Vec<Output>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct OutputListResp {
            items: Vec<Output>,
            // total: u32,
        }

        let mut all_outputs = Vec::new();
        let pagesize = 100;
        let mut page = 1;

        loop {
            let skip = (page - 1) * pagesize;
            let res = self
                .client
                .get(format!(
                    r#"{}/api/output/?q={{"filter":{{}},"skip":{},"limit":{}}}"#,
                    self.url, skip, pagesize
                ))
                .header("content-type", "application/json")
                .send()?
                .error_if_not_success()?;

            let resp = res.json::<OutputListResp>()?;
            let items_count = resp.items.len();
            all_outputs.extend(resp.items);

            if items_count < pagesize as usize {
                break;
            }
            page += 1;
        }

        Ok(all_outputs)
    }

    pub fn find_outputs(&self, name: &str) -> Result<Vec<Output>, reqwest::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct OutputFilter {
            search_name: String,
        }

        #[derive(Debug, Deserialize)]
        struct OutputListResp {
            items: Vec<Output>,
            // total: u32,
        }

        let query = EdgeQuery {
            filter: OutputFilter {
                search_name: name.to_owned(),
            },
        };
        let query = serde_json::to_string(&query).expect("Failed to serialize filter as JSON");

        let res = self
            .client
            .get(format!(r#"{}/api/output/?q={}"#, self.url, query))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<OutputListResp>()?.items)
    }

    pub fn list_outputs_by_ids(&self, ids: Vec<String>) -> Result<Vec<Output>, EdgeError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct OutputFilterByIds {
            ids: Vec<String>,
        }

        #[derive(Debug, Deserialize)]
        struct OutputListResp {
            items: Vec<Output>,
            // total: u32,
        }

        let query = EdgeQuery {
            filter: OutputFilterByIds { ids },
        };
        let query = serde_json::to_string(&query).expect("Failed to serialize filter as JSON");

        let res = self
            .client
            .get(format!(r#"{}/api/output/?q={}"#, self.url, query))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<OutputListResp>()?.items)
    }

    pub fn create_output(&self, output: NewOutput) -> Result<(), EdgeError> {
        self.client
            .post(format!("{}/api/output/", self.url))
            .header("content-type", "application/json")
            .json(&output)
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn delete_output(&self, id: &str) -> Result<(), EdgeError> {
        self.client
            .delete(format!("{}/api/output/{}", self.url, id))
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn find_groups(&self, name: &str) -> Result<Vec<Group>, EdgeError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct GroupFilter {
            search_name: String,
        }

        #[derive(Debug, Deserialize)]
        struct GroupListResp {
            items: Vec<Group>,
            // total: u32,
        }

        let query = EdgeQuery {
            filter: GroupFilter {
                search_name: name.to_owned(),
            },
        };
        let query = serde_json::to_string(&query).expect("Failed to serialize filter as JSON");

        let res = self
            .client
            .get(format!(r#"{}/api/group/?q={}"#, self.url, query))
            .header("content-type", "application/json")
            .send()?
            .error_if_not_success()?;

        Ok(res.json::<GroupListResp>()?.items)
    }

    pub fn list_groups(&self) -> Result<Vec<Group>, reqwest::Error> {
        #[derive(Debug, Deserialize)]
        struct GroupListResp {
            items: Vec<Group>,
            // total: u32,
        }

        let res = self
            .client
            .get(format!(r#"{}/api/group/"#, self.url,))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<GroupListResp>()?.items)
    }

    pub fn get_group(&self, id: &str) -> Result<Group, reqwest::Error> {
        let res = self
            .client
            .get(format!(r#"{}/api/group/{}"#, self.url, id))
            .header("content-type", "application/json")
            .send()?;

        res.json::<Group>()
    }

    pub fn get_group_core_secret(&self, id: &str) -> Result<String, reqwest::Error> {
        let res = self
            .client
            .get(format!(r#"{}/api/group/{}/core-secret"#, self.url, id))
            .header("content-type", "application/json")
            .send()?;

        #[derive(Deserialize)]
        struct SecretResp {
            secret: String,
        }

        Ok(res.json::<SecretResp>()?.secret)
    }

    pub fn create_group(&self, group: NewGroup) -> Result<Group, EdgeError> {
        let res = self
            .client
            .post(format!("{}/api/group/", self.url))
            .header("content-type", "application/json")
            .json(&group)
            .send()?
            .error_if_not_success()?;

        Ok(res.json::<Group>()?)
    }

    pub fn delete_group(&self, id: &str) -> Result<(), EdgeError> {
        self.client
            .delete(format!("{}/api/group/{}", self.url, id))
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn get_port(&self, id: &str) -> Result<Port, reqwest::Error> {
        let res = self
            .client
            .get(format!(r#"{}/api/port/{}"#, self.url, id))
            .header("content-type", "application/json")
            .send()?;

        res.json::<Port>()
    }

    pub fn list_ports_by_ids(&self, ids: Vec<String>) -> Result<Vec<Port>, reqwest::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct PortFilterByIds {
            ids: Vec<String>,
        }

        #[derive(Debug, Deserialize)]
        struct PortListResp {
            items: Vec<Port>,
        }

        let query = EdgeQuery {
            filter: PortFilterByIds { ids },
        };
        let query = serde_json::to_string(&query).expect("Failed to serialize filter as JSON");

        let res = self
            .client
            .get(format!(r#"{}/api/port/?q={}"#, self.url, query))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<PortListResp>()?.items)
    }

    pub fn list_appliances(&self) -> Result<Vec<Appliance>, reqwest::Error> {
        #[derive(Debug, Deserialize)]
        struct ApplianceListResponse {
            items: Vec<Appliance>,
            // total: u32,
        }

        let res = self
            .client
            .get(format!(r#"{}/api/appliance/"#, self.url,))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<ApplianceListResponse>()?.items)
    }

    pub fn find_appliances(&self, name: &str) -> Result<Vec<Appliance>, reqwest::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct ApplianceFilter {
            search_name: String,
        }

        #[derive(Debug, Deserialize)]
        struct ApplianceListResp {
            items: Vec<Appliance>,
            // total: u32,
        }

        let query = EdgeQuery {
            filter: ApplianceFilter {
                search_name: name.to_owned(),
            },
        };
        let query = serde_json::to_string(&query).expect("Failed to serialize filter as JSON");

        let res = self
            .client
            .get(format!(r#"{}/api/appliance/?q={}"#, self.url, query))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<ApplianceListResp>()?.items)
    }

    pub fn delete_appliance(&self, id: &str) -> Result<(), EdgeError> {
        self.client
            .delete(format!("{}/api/appliance/{}", self.url, id))
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn get_appliance_inputs(&self, id: &str) -> Result<Vec<ApplianceInput>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct InputListResp {
            items: Vec<ApplianceInput>,
            // total: u32,
        }

        let res = self
            .client
            .get(format!(r#"{}/api/appliance/{}/inputs"#, self.url, id))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<InputListResp>()?.items)
    }

    pub fn get_appliance_outputs(&self, id: &str) -> Result<Vec<ApplianceOutput>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct OutputListResp {
            items: Vec<ApplianceOutput>,
            // total: u32,
        }

        let res = self
            .client
            .get(format!(r#"{}/api/appliance/{}/outputs"#, self.url, id))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<OutputListResp>()?.items)
    }

    pub fn get_appliance_config(&self, id: &str) -> Result<serde_json::Value, EdgeError> {
        let res = self
            .client
            .get(format!(r#"{}/api/appliance/{}/config"#, self.url, id))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json()?)
    }

    pub fn restart_appliance(&self, id: &str) -> Result<(), EdgeError> {
        self.client
            .post(format!("{}/api/appliance/{}/restart", self.url, id))
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn update_appliance(
        &self,
        id: &str,
        payload: UpdateAppliancePayload,
    ) -> Result<(), EdgeError> {
        self.client
            .post(format!("{}/api/appliance/{}", self.url, id))
            .header("content-type", "application/json")
            .json(&payload)
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn list_regions(&self) -> Result<Vec<Region>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct RegionListResp {
            items: Vec<Region>,
            // total: u32,
        }

        let res = self
            .client
            .get(format!(r#"{}/api/region/"#, self.url))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<RegionListResp>()?.items)
    }

    pub fn find_region(&self, name: &str) -> Result<Vec<Region>, reqwest::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct RegionFilter {
            search_name: String,
        }

        #[derive(Debug, Deserialize)]
        struct RegionListResp {
            items: Vec<Region>,
            // total: u32,
        }

        let query = EdgeQuery {
            filter: RegionFilter {
                search_name: name.to_owned(),
            },
        };
        let query = serde_json::to_string(&query).expect("Failed to serialize filter as JSON");

        let res = self
            .client
            .get(format!(r#"{}/api/region/?q={}"#, self.url, query))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<RegionListResp>()?.items)
    }

    pub fn create_region(&self, region: NewRegion) -> Result<(), EdgeError> {
        self.client
            .post(format!("{}/api/region/", self.url))
            .header("content-type", "application/json")
            .json(&region)
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn delete_region(&self, id: &str) -> Result<(), EdgeError> {
        self.client
            .delete(format!("{}/api/region/{}", self.url, id))
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn list_kubernetes_nodes(&self) -> Result<Vec<KubernetesNode>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct NodeListResp {
            items: Vec<KubernetesNode>,
            // total: u32,
        }

        let res = self
            .client
            .get(format!(r#"{}/api/k8s/node/"#, self.url))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<NodeListResp>()?.items)
    }

    pub fn list_tunnels(&self) -> Result<Vec<Tunnel>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct TunnelListResp {
            items: Vec<Tunnel>,
            // total: u32,
        }

        let res = self
            .client
            .get(format!(r#"{}/api/tunnel/"#, self.url))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<TunnelListResp>()?.items)
    }

    pub fn get_build_info(&self) -> Result<BuildInfo, EdgeError> {
        let res = self
            .client
            .get(format!(r#"{}/api/build-info"#, self.url))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<BuildInfo>()?)
    }

    pub fn global_settings(&self) -> Result<GlobalSettings, EdgeError> {
        let res = self
            .client
            .get(format!(r#"{}/api/globalsettings"#, self.url))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<GlobalSettings>()?)
    }

    pub fn set_global_settings(&self, settings: GlobalSettings) -> Result<(), EdgeError> {
        self.client
            .put(format!("{}/api/globalsettings", self.url))
            .header("content-type", "application/json")
            .json(&settings)
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn list_output_recipient_lists(&self) -> Result<Vec<OutputRecipientList>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct OutputRecipientListResp {
            items: Vec<OutputRecipientList>,
        }

        let res = self
            .client
            .get(format!("{}/api/outputRecipientList/", self.url))
            .header("content-type", "application/json")
            .send()?
            .error_if_not_success()?;

        Ok(res.json::<OutputRecipientListResp>()?.items)
    }

    pub fn create_output_recipient_list(
        &self,
        list: NewOutputRecipientList,
    ) -> Result<OutputRecipientList, EdgeError> {
        let res = self
            .client
            .post(format!("{}/api/outputRecipientList/", self.url))
            .header("content-type", "application/json")
            .json(&list)
            .send()?
            .error_if_not_success()?;

        Ok(res.json::<OutputRecipientList>()?)
    }

    pub fn delete_output_recipient_list(&self, id: &str) -> Result<(), EdgeError> {
        self.client
            .delete(format!("{}/api/outputRecipientList/{}", self.url, id))
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn get_output_list_members(&self, id: &str) -> Result<Vec<Output>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct OutputListMembersResp {
            items: Vec<Output>,
        }

        let res = self
            .client
            .get(format!(
                "{}/api/outputRecipientList/{}/outputs",
                self.url, id
            ))
            .header("content-type", "application/json")
            .send()?
            .error_if_not_success()?;

        Ok(res.json::<OutputListMembersResp>()?.items)
    }

    pub fn add_output_to_list(
        &self,
        list_id: &str,
        list_name: &str,
        outputs: Vec<String>,
    ) -> Result<(), EdgeError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct UpdateOutputsPayload {
            name: String,
            add_outputs: Vec<String>,
            remove_outputs: Vec<String>,
        }

        self.client
            .post(format!("{}/api/outputRecipientList/{}", self.url, list_id))
            .header("content-type", "application/json")
            .json(&UpdateOutputsPayload {
                name: list_name.to_string(),
                add_outputs: outputs,
                remove_outputs: Vec::new(),
            })
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn remove_output_from_list(
        &self,
        list_id: &str,
        list_name: &str,
        outputs: Vec<String>,
    ) -> Result<(), EdgeError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct UpdateOutputsPayload {
            name: String,
            add_outputs: Vec<String>,
            remove_outputs: Vec<String>,
        }

        self.client
            .post(format!("{}/api/outputRecipientList/{}", self.url, list_id))
            .header("content-type", "application/json")
            .json(&UpdateOutputsPayload {
                name: list_name.to_string(),
                add_outputs: Vec::new(),
                remove_outputs: outputs,
            })
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn find_output_recipient_lists(
        &self,
        name: &str,
    ) -> Result<Vec<OutputRecipientList>, EdgeError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct OutputRecipientListFilter {
            search_name: String,
        }

        #[derive(Debug, Deserialize)]
        struct OutputRecipientListResp {
            items: Vec<OutputRecipientList>,
        }

        let query = EdgeQuery {
            filter: OutputRecipientListFilter {
                search_name: name.to_owned(),
            },
        };
        let query = serde_json::to_string(&query).expect("Failed to serialize filter as JSON");

        let res = self
            .client
            .get(format!(
                r#"{}/api/outputRecipientList/?q={}"#,
                self.url, query
            ))
            .header("content-type", "application/json")
            .send()?
            .error_if_not_success()?;

        Ok(res.json::<OutputRecipientListResp>()?.items)
    }

    pub fn list_group_recipient_lists(&self) -> Result<Vec<GroupRecipientList>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct GroupRecipientListResp {
            items: Vec<GroupRecipientList>,
        }

        let res = self
            .client
            .get(format!("{}/api/groupRecipientList/", self.url))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<GroupRecipientListResp>()?.items)
    }

    pub fn create_group_recipient_list(
        &self,
        list: NewGroupRecipientList,
    ) -> Result<GroupRecipientList, EdgeError> {
        let res = self
            .client
            .post(format!("{}/api/groupRecipientList/", self.url))
            .header("content-type", "application/json")
            .json(&list)
            .send()?
            .error_if_not_success()?;

        Ok(res.json::<GroupRecipientList>()?)
    }

    pub fn delete_group_recipient_list(&self, id: &str) -> Result<(), EdgeError> {
        self.client
            .delete(format!("{}/api/groupRecipientList/{}", self.url, id))
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn get_group_list_members(&self, id: &str) -> Result<Vec<Group>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct GroupListMembersResp {
            items: Vec<Group>,
        }

        let res = self
            .client
            .get(format!("{}/api/groupRecipientList/{}/groups", self.url, id))
            .header("content-type", "application/json")
            .send()?;

        Ok(res.json::<GroupListMembersResp>()?.items)
    }

    pub fn add_group_to_list(
        &self,
        list_id: &str,
        list_name: &str,
        group_ids: Vec<String>,
    ) -> Result<(), EdgeError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct UpdateGroupsPayload {
            name: String,
            add_groups: Vec<String>,
            remove_groups: Vec<String>,
        }

        self.client
            .post(format!("{}/api/groupRecipientList/{}", self.url, list_id))
            .header("content-type", "application/json")
            .json(&UpdateGroupsPayload {
                name: list_name.to_string(),
                add_groups: group_ids,
                remove_groups: Vec::new(),
            })
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn remove_group_from_list(
        &self,
        list_id: &str,
        list_name: &str,
        group_ids: Vec<String>,
    ) -> Result<(), EdgeError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct UpdateGroupsPayload {
            name: String,
            add_groups: Vec<String>,
            remove_groups: Vec<String>,
        }

        self.client
            .post(format!("{}/api/groupRecipientList/{}", self.url, list_id))
            .header("content-type", "application/json")
            .json(&UpdateGroupsPayload {
                name: list_name.to_string(),
                add_groups: Vec::new(),
                remove_groups: group_ids,
            })
            .send()?
            .error_if_not_success()
            .map(|_| ())
    }

    pub fn find_group_recipient_lists(
        &self,
        name: &str,
    ) -> Result<Vec<GroupRecipientList>, EdgeError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct GroupRecipientListFilter {
            search_name: String,
        }

        #[derive(Debug, Deserialize)]
        struct GroupRecipientListResp {
            items: Vec<GroupRecipientList>,
        }

        let query = EdgeQuery {
            filter: GroupRecipientListFilter {
                search_name: name.to_owned(),
            },
        };
        let query = serde_json::to_string(&query).expect("Failed to serialize filter as JSON");

        let res = self
            .client
            .get(format!(
                r#"{}/api/groupRecipientList/?q={}"#,
                self.url, query
            ))
            .header("content-type", "application/json")
            .send()?
            .error_if_not_success()?;

        Ok(res.json::<GroupRecipientListResp>()?.items)
    }

    pub fn list_alarms(&self) -> Result<Vec<AlarmWithImpact>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct AlarmListResp {
            items: Vec<AlarmWithImpact>,
        }

        let mut all_alarms = Vec::new();
        let pagesize = 100;
        let mut page = 1;

        loop {
            let skip = (page - 1) * pagesize;
            let res = self
                .client
                .get(format!(
                    r#"{}/api/alarm/?q={{"filter":{{}},"skip":{},"limit":{}}}"#,
                    self.url, skip, pagesize
                ))
                .header("content-type", "application/json")
                .send()?
                .error_if_not_success()?;

            let resp = res.json::<AlarmListResp>()?;
            let items_count = resp.items.len();
            all_alarms.extend(resp.items);

            if items_count < pagesize {
                break;
            }
            page += 1;
        }

        Ok(all_alarms)
    }

    pub fn list_alarm_history(
        &self,
        limit: usize,
        from_date: Option<String>,
        to_date: Option<String>,
    ) -> Result<Vec<Alarm>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct AlarmLogResp {
            items: Vec<Alarm>,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct AlarmLogFilter {
            #[serde(skip_serializing_if = "Option::is_none")]
            from_date: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            to_date: Option<String>,
        }

        #[derive(Serialize)]
        struct AlarmLogQuery {
            filter: AlarmLogFilter,
            skip: usize,
            limit: usize,
        }

        let mut all_alarms = Vec::new();
        let pagesize = 100;
        let mut page = 1;

        loop {
            let skip = (page - 1) * pagesize;
            let remaining = limit.saturating_sub(all_alarms.len());
            if remaining == 0 {
                break;
            }

            let fetch_size = pagesize.min(remaining);

            let query = AlarmLogQuery {
                filter: AlarmLogFilter {
                    from_date: from_date.clone(),
                    to_date: to_date.clone(),
                },
                skip,
                limit: fetch_size,
            };
            let query_str = serde_json::to_string(&query).expect("Failed to serialize query");

            let res = self
                .client
                .get(format!("{}/api/alarm-log/?q={}", self.url, query_str))
                .header("content-type", "application/json")
                .send()?
                .error_if_not_success()?;

            let resp = res.json::<AlarmLogResp>()?;
            let items_count = resp.items.len();
            all_alarms.extend(resp.items);

            if items_count < fetch_size {
                break;
            }
            page += 1;
        }

        Ok(all_alarms)
    }

    pub fn clear_alarm(&self, alarm_id: &str) -> Result<(), EdgeError> {
        #[derive(Serialize)]
        struct ClearAlarmRequest {
            status: String,
        }

        let encoded_id = urlencoding::encode(alarm_id);
        self.client
            .put(format!("{}/api/alarm/{}", self.url, encoded_id))
            .header("content-type", "application/json")
            .json(&ClearAlarmRequest {
                status: "CLEAR".to_string(),
            })
            .send()?
            .error_if_not_success()?;

        Ok(())
    }

    pub fn create_api_token(&self, init: ApiTokenInit) -> Result<ApiToken, EdgeError> {
        let res = self
            .client
            .post(format!("{}/api/api-token/", self.url))
            .header("content-type", "application/json")
            .json(&init)
            .send()
            .map_err(EdgeError::RequestError)?
            .error_if_not_success()?;

        res.json::<ApiToken>().map_err(EdgeError::RequestError)
    }

    pub fn list_api_tokens(&self) -> Result<Vec<ApiToken>, EdgeError> {
        #[derive(Debug, Deserialize)]
        struct ApiTokenListResp {
            items: Vec<ApiToken>,
        }

        let res = self
            .client
            .get(format!("{}/api/api-token/", self.url))
            .header("content-type", "application/json")
            .send()
            .map_err(EdgeError::RequestError)?
            .error_if_not_success()?;

        res.json::<ApiTokenListResp>()
            .map(|resp| resp.items)
            .map_err(EdgeError::RequestError)
    }

    pub fn delete_api_token(&self, name: &str) -> Result<(), EdgeError> {
        self.client
            .delete(format!("{}/api/api-token/{}", self.url, name))
            .header("content-type", "application/json")
            .send()
            .map_err(EdgeError::RequestError)?
            .error_if_not_success()
            .map(|_| ())
    }

    /// Fetch thumbnail image for an input
    /// Returns JPEG bytes on success, or None if thumbnail doesn't exist
    pub fn fetch_thumbnail(&self, path: &str) -> Option<Vec<u8>> {
        let timestamp = chrono::Utc::now().timestamp();
        let url = format!("{}/{}.jpg?{}", self.url, path, timestamp);

        let response = self.client.get(&url).send().ok()?;
        if response.status().is_success() {
            response.bytes().ok().map(|b| b.to_vec())
        } else {
            None
        }
    }
}
